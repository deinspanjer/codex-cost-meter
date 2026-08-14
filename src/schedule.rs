use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

use crate::update::FailureClass;

const MAX_STATUS_BYTES: u64 = 4096;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResultCode {
    Success,
    OrdinaryFailure,
    DiskFull,
    IncompatibleSchema,
    PermissionDenied,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Status {
    #[serde(with = "time::serde::rfc3339::option")]
    pub(crate) last_run_at: Option<OffsetDateTime>,
    pub(crate) result: ResultCode,
    pub(crate) consecutive_failures: u8,
    pub(crate) paused: bool,
    pub(crate) remediation: String,
}

#[derive(Debug, Error)]
pub(crate) enum StatusError {
    #[error("schedule status is too large")]
    TooLarge,
    #[error("schedule status is malformed")]
    Malformed,
    #[error("schedule status has an invalid remediation")]
    InvalidRemediation,
    #[error("schedule status has too many consecutive failures")]
    TooManyFailures,
    #[error("could not read schedule status")]
    Read {
        #[source]
        source: io::Error,
    },
    #[error("could not create schedule status directory")]
    CreateParent {
        #[source]
        source: io::Error,
    },
    #[error("could not create temporary schedule status")]
    CreateTemporary {
        #[source]
        source: io::Error,
    },
    #[error("could not set schedule status permissions")]
    Permissions {
        #[source]
        source: io::Error,
    },
    #[error("could not write schedule status")]
    Write {
        #[source]
        source: io::Error,
    },
    #[error("could not flush schedule status")]
    Flush {
        #[source]
        source: io::Error,
    },
    #[error("could not synchronize schedule status")]
    Sync {
        #[source]
        source: io::Error,
    },
    #[error("could not replace schedule status")]
    Rename {
        #[source]
        source: io::Error,
    },
    #[error("could not serialize schedule status")]
    Serialize,
}

pub(crate) fn after_failure(
    previous: Option<Status>,
    failure: FailureClass,
    now: OffsetDateTime,
) -> Status {
    match failure {
        FailureClass::Ordinary => {
            let consecutive_failures = previous
                .map_or(0, |status| status.consecutive_failures)
                .saturating_add(1)
                .min(3);
            status(
                Some(now),
                ResultCode::OrdinaryFailure,
                consecutive_failures,
                consecutive_failures == 3,
            )
        }
        FailureClass::DiskFull => status(Some(now), ResultCode::DiskFull, 1, true),
        FailureClass::IncompatibleSchema => {
            status(Some(now), ResultCode::IncompatibleSchema, 1, true)
        }
        FailureClass::PermissionDenied => status(Some(now), ResultCode::PermissionDenied, 1, true),
    }
}

pub(crate) fn after_success(_: Option<Status>, now: OffsetDateTime) -> Status {
    status(Some(now), ResultCode::Success, 0, false)
}

pub(crate) fn resume_status(previous: Option<Status>) -> Status {
    status(
        previous.and_then(|status| status.last_run_at),
        ResultCode::Success,
        0,
        false,
    )
}

pub(crate) fn read_status(path: &Path) -> Result<Option<Status>, StatusError> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(StatusError::Read { source }),
    };
    if file
        .metadata()
        .map_err(|source| StatusError::Read { source })?
        .len()
        > MAX_STATUS_BYTES
    {
        return Err(StatusError::TooLarge);
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_STATUS_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|source| StatusError::Read { source })?;
    let status = serde_json::from_slice(&bytes).map_err(|_| StatusError::Malformed)?;
    validate(&status)?;
    Ok(Some(status))
}

pub(crate) fn write_status(path: &Path, status: &Status) -> Result<(), StatusError> {
    validate(status)?;
    let bytes = serde_json::to_vec(status).map_err(|_| StatusError::Serialize)?;
    let parent = path.parent().filter(|path| !path.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent).map_err(|source| StatusError::CreateParent { source })?;
    }
    let parent = parent.unwrap_or_else(|| Path::new("."));
    let temporary = parent.join(format!(".schedule-status-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|source| StatusError::CreateTemporary { source })?;
    set_private_permissions(&file).map_err(|source| StatusError::Permissions { source })?;
    file.write_all(&bytes)
        .map_err(|source| StatusError::Write { source })?;
    file.flush()
        .map_err(|source| StatusError::Flush { source })?;
    file.sync_all()
        .map_err(|source| StatusError::Sync { source })?;
    drop(file);
    fs::rename(temporary, path).map_err(|source| StatusError::Rename { source })
}

fn status(
    last_run_at: Option<OffsetDateTime>,
    result: ResultCode,
    consecutive_failures: u8,
    paused: bool,
) -> Status {
    Status {
        last_run_at,
        result,
        consecutive_failures,
        paused,
        remediation: remediation(result).into(),
    }
}

fn validate(status: &Status) -> Result<(), StatusError> {
    if status.consecutive_failures > 3 {
        return Err(StatusError::TooManyFailures);
    }
    if status.remediation != remediation(status.result) {
        return Err(StatusError::InvalidRemediation);
    }
    Ok(())
}

fn remediation(result: ResultCode) -> &'static str {
    match result {
        ResultCode::Success => "No action required.",
        ResultCode::OrdinaryFailure => "Retry the scheduled update.",
        ResultCode::DiskFull => "Free disk space, then resume the schedule.",
        ResultCode::IncompatibleSchema => "Update Codex Cost Meter, then resume the schedule.",
        ResultCode::PermissionDenied => {
            "Restore access to Codex storage, then resume the schedule."
        }
    }
}

#[cfg(unix)]
fn set_private_permissions(file: &File) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    file.set_permissions(fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_permissions(_: &File) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;
    use time::{Duration, OffsetDateTime};

    use super::{
        ResultCode, Status, StatusError, after_failure, after_success, read_status, resume_status,
        write_status,
    };
    use crate::update::FailureClass;

    #[test]
    fn failure_transitions_pause_after_three_ordinary_failures_or_one_severe_failure() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let two_failures = Status {
            last_run_at: Some(now - Duration::minutes(1)),
            result: ResultCode::OrdinaryFailure,
            consecutive_failures: 2,
            paused: false,
            remediation: String::new(),
        };

        for (previous, class, result, consecutive_failures, paused) in [
            (
                None,
                FailureClass::Ordinary,
                ResultCode::OrdinaryFailure,
                1,
                false,
            ),
            (
                Some(two_failures.clone()),
                FailureClass::Ordinary,
                ResultCode::OrdinaryFailure,
                3,
                true,
            ),
            (None, FailureClass::DiskFull, ResultCode::DiskFull, 1, true),
            (
                None,
                FailureClass::IncompatibleSchema,
                ResultCode::IncompatibleSchema,
                1,
                true,
            ),
            (
                None,
                FailureClass::PermissionDenied,
                ResultCode::PermissionDenied,
                1,
                true,
            ),
        ] {
            let status = after_failure(previous, class, now);
            assert_eq!(status.last_run_at, Some(now));
            assert_eq!(status.result, result);
            assert_eq!(status.consecutive_failures, consecutive_failures);
            assert_eq!(status.paused, paused);
        }
    }

    #[test]
    fn ordinary_failures_saturate_at_three() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let previous = Status {
            last_run_at: None,
            result: ResultCode::OrdinaryFailure,
            consecutive_failures: 3,
            paused: true,
            remediation: String::new(),
        };

        assert_eq!(
            after_failure(Some(previous), FailureClass::Ordinary, now).consecutive_failures,
            3
        );
    }

    #[test]
    fn success_and_resume_clear_the_circuit_breaker_without_changing_resume_time() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let paused = Status {
            last_run_at: Some(now - Duration::minutes(1)),
            result: ResultCode::PermissionDenied,
            consecutive_failures: 1,
            paused: true,
            remediation: String::new(),
        };

        let success = after_success(Some(paused.clone()), now);
        assert_eq!(success.last_run_at, Some(now));
        assert_eq!(success.result, ResultCode::Success);
        assert_eq!(success.consecutive_failures, 0);
        assert!(!success.paused);

        let resumed = resume_status(Some(paused));
        assert_eq!(resumed.last_run_at, Some(now - Duration::minutes(1)));
        assert_eq!(resumed.result, ResultCode::Success);
        assert_eq!(resumed.consecutive_failures, 0);
        assert!(!resumed.paused);
    }

    #[test]
    fn status_storage_round_trips_atomically_with_private_permissions() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("status.json");
        let status = after_failure(None, FailureClass::Ordinary, OffsetDateTime::UNIX_EPOCH);

        write_status(&path, &status).unwrap();

        assert_eq!(read_status(&path).unwrap(), Some(status));
        assert!(serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).is_ok());
        assert!(
            fs::read_dir(directory.path())
                .unwrap()
                .all(|entry| entry.unwrap().path() == path)
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn malformed_or_unallowlisted_status_is_rejected() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("status.json");

        fs::write(&path, b"{").unwrap();
        assert!(matches!(read_status(&path), Err(StatusError::Malformed)));

        fs::write(
            &path,
            br#"{"last_run_at":null,"result":"success","consecutive_failures":0,"paused":false,"remediation":"sensitive updater detail"}"#,
        )
        .unwrap();
        assert!(matches!(
            read_status(&path),
            Err(StatusError::InvalidRemediation)
        ));
    }

    #[test]
    fn oversized_or_over_limit_status_is_rejected() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("status.json");

        fs::write(&path, vec![b' '; 4097]).unwrap();
        assert!(matches!(read_status(&path), Err(StatusError::TooLarge)));

        fs::write(
            &path,
            br#"{"last_run_at":null,"result":"ordinary_failure","consecutive_failures":4,"paused":true,"remediation":"Retry the scheduled update."}"#,
        )
        .unwrap();
        assert!(matches!(
            read_status(&path),
            Err(StatusError::TooManyFailures)
        ));
    }

    #[test]
    fn writing_an_unallowlisted_remediation_never_persists_it() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("status.json");
        let status = Status {
            last_run_at: None,
            result: ResultCode::Success,
            consecutive_failures: 0,
            paused: false,
            remediation: "private task metadata".into(),
        };

        assert!(matches!(
            write_status(&path, &status),
            Err(StatusError::InvalidRemediation)
        ));
        assert!(!path.exists());
    }
}
