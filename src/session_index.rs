use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::rollout::discovery::read_jsonl;

#[derive(Debug, Error)]
pub(crate) enum AppendError {
    #[error("session index storage is full while attempting to {operation}: {source}")]
    DiskFull {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("could not open session index {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not seek session index {path}: {source}")]
    Seek {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not read session index {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not write session index {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not flush session index {path}: {source}")]
    Flush {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not sync session index {path}: {source}")]
    Sync {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Serialize)]
struct IndexRecord<'a> {
    id: &'a str,
    thread_name: &'a str,
    updated_at: &'a str,
}

pub(crate) fn append(
    path: &Path,
    updates: &[(String, String)],
    updated_at: OffsetDateTime,
) -> Result<(), AppendError> {
    if updates.is_empty() {
        return Ok(());
    }
    let timestamp = updated_at
        .format(&Rfc3339)
        .expect("the RFC3339 format description supports all OffsetDateTime values");
    let mut index = open_index(path)?;
    let end = index
        .seek(SeekFrom::End(0))
        .map_err(|source| append_error("seek", path, source))?;
    let mut bytes = Vec::new();
    if end > 0 {
        index
            .seek(SeekFrom::Start(end - 1))
            .map_err(|source| append_error("seek", path, source))?;
        let mut final_byte = [0];
        index
            .read_exact(&mut final_byte)
            .map_err(|source| append_error("read", path, source))?;
        if final_byte != *b"\n" {
            bytes.push(b'\n');
        }
    }
    for (id, name) in updates {
        serde_json::to_writer(
            &mut bytes,
            &IndexRecord {
                id,
                thread_name: name,
                updated_at: &timestamp,
            },
        )
        .expect("serializing string-only session-index records cannot fail");
        bytes.push(b'\n');
    }
    index
        .write_all(&bytes)
        .map_err(|source| append_error("write", path, source))?;
    index
        .flush()
        .map_err(|source| append_error("flush", path, source))?;
    index
        .sync_all()
        .map_err(|source| append_error("sync", path, source))
}

fn open_index(path: &Path) -> Result<File, AppendError> {
    OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|source| append_error("open", path, source))
}

fn append_error(operation: &'static str, path: &Path, source: io::Error) -> AppendError {
    if matches!(source.raw_os_error(), Some(28 | 112)) {
        return AppendError::DiskFull { operation, source };
    }
    let path = path.to_path_buf();
    match operation {
        "open" => AppendError::Open { path, source },
        "seek" => AppendError::Seek { path, source },
        "read" => AppendError::Read { path, source },
        "write" => AppendError::Write { path, source },
        "flush" => AppendError::Flush { path, source },
        "sync" => AppendError::Sync { path, source },
        _ => unreachable!("all append operations are classified"),
    }
}

pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) updated_at: OffsetDateTime,
}

pub(crate) struct Snapshot {
    entries: HashMap<String, Entry>,
    malformed_records: usize,
    oversized_records: usize,
    read_error: Option<io::ErrorKind>,
}

impl Snapshot {
    pub(crate) fn load(home: &Path) -> Self {
        let mut snapshot = Self {
            entries: HashMap::new(),
            malformed_records: 0,
            oversized_records: 0,
            read_error: None,
        };
        let result = read_jsonl(&home.join("session_index.jsonl"), |line| {
            let Ok(value) = serde_json::from_slice::<Value>(line) else {
                snapshot.malformed_records += 1;
                return;
            };
            let Some(object) = value.as_object() else {
                snapshot.malformed_records += 1;
                return;
            };
            let Some(id) = object.get("id").and_then(Value::as_str) else {
                return;
            };
            let Some(name) = object.get("thread_name").and_then(Value::as_str) else {
                return;
            };
            if name.is_empty() {
                return;
            }
            let Some(updated_at) = object.get("updated_at").and_then(Value::as_str) else {
                snapshot.malformed_records += 1;
                return;
            };
            let Ok(updated_at) = OffsetDateTime::parse(updated_at, &Rfc3339) else {
                snapshot.malformed_records += 1;
                return;
            };
            if snapshot
                .entries
                .get(id)
                .is_none_or(|entry| updated_at >= entry.updated_at)
            {
                snapshot.entries.insert(
                    id.into(),
                    Entry {
                        name: name.into(),
                        updated_at,
                    },
                );
            }
        });
        match result {
            Ok(summary) => snapshot.oversized_records = summary.oversized_lines_skipped,
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                snapshot.read_error = Some(error.kind());
            }
            Err(_) => {}
        }
        snapshot
    }

    pub(crate) fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.get(id)
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.malformed_records == 0 && self.oversized_records == 0 && self.read_error.is_none()
    }

    pub(crate) fn malformed_records(&self) -> usize {
        self.malformed_records
    }

    pub(crate) fn oversized_records(&self) -> usize {
        self.oversized_records
    }

    pub(crate) fn read_error(&self) -> Option<io::ErrorKind> {
        self.read_error
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{self, Seek, SeekFrom, Write},
        path::Path,
    };

    use serde_json::json;
    use tempfile::TempDir;
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    use super::{AppendError, Snapshot, append, append_error, open_index};

    #[test]
    fn retains_the_latest_valid_entry_and_counts_malformed_records() {
        let home = TempDir::new().unwrap();
        assert!(Snapshot::load(home.path()).is_complete());

        let rows = [
            json!({
                "id": "root",
                "thread_name": "oldest",
                "updated_at": "2026-08-13T12:00:00Z",
            })
            .to_string(),
            "not json".into(),
            json!("wrong type").to_string(),
            json!({
                "id": "root",
                "thread_name": "ignored",
                "updated_at": "2026-08-13T12:00:30Z",
                "unknown_field": "ignored",
            })
            .to_string(),
            json!({
                "id": "root",
                "thread_name": "newest",
                "updated_at": "2026-08-13T12:01:00Z",
            })
            .to_string(),
        ];
        fs::write(home.path().join("session_index.jsonl"), rows.join("\n")).unwrap();

        let snapshot = Snapshot::load(home.path());

        assert_eq!(snapshot.entry("root").unwrap().name, "newest");
        assert_eq!(snapshot.malformed_records(), 2);
        assert!(!snapshot.is_complete());
    }

    #[test]
    fn append_repairs_a_partial_final_record_and_uses_one_timestamp_for_the_batch() {
        let home = TempDir::new().unwrap();
        let path = home.path().join("session_index.jsonl");
        fs::write(&path, br#"{"id":"root","thread_name":"truncated"#).unwrap();
        let updated_at = OffsetDateTime::parse("2026-08-14T12:34:56Z", &Rfc3339).unwrap();

        append(
            &path,
            &[
                ("root".into(), "fresh".into()),
                ("next".into(), "new".into()),
            ],
            updated_at,
        )
        .unwrap();

        let bytes = fs::read(&path).unwrap();
        let rendered_timestamp = updated_at.format(&Rfc3339).unwrap();
        assert!(bytes.starts_with(b"{\"id\":\"root\",\"thread_name\":\"truncated\n"));
        assert!(bytes.ends_with(b"\n"));
        assert_eq!(
            bytes
                .windows(rendered_timestamp.len())
                .filter(|window| *window == rendered_timestamp.as_bytes())
                .count(),
            2
        );
        let snapshot = Snapshot::load(home.path());
        assert_eq!(snapshot.entry("root").unwrap().name, "fresh");
        assert_eq!(snapshot.entry("next").unwrap().name, "new");
    }

    #[test]
    fn classifies_enospc_as_an_actionable_disk_full_error() {
        let error = append_error(
            "write",
            Path::new("session_index.jsonl"),
            io::Error::from_raw_os_error(28),
        );

        assert!(matches!(error, AppendError::DiskFull { .. }));
    }

    #[test]
    fn opens_the_index_with_os_append_semantics() {
        let home = TempDir::new().unwrap();
        let path = home.path().join("session_index.jsonl");
        fs::write(&path, b"existing").unwrap();
        let mut index = open_index(&path).unwrap();

        index.seek(SeekFrom::Start(0)).unwrap();
        index.write_all(b"-appended").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"existing-appended");
    }
}
