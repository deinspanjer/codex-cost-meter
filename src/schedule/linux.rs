use std::{
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

#[cfg(target_os = "linux")]
use std::process::Command;

use thiserror::Error;
use time::format_description::well_known::Rfc3339;

use super::{InstallOptions, Status, StatusError, read_status, resume_status, write_status};

const SERVICE: &str = "io.github.deinspanjer.codex-cost-meter.service";
const TIMER: &str = "io.github.deinspanjer.codex-cost-meter.timer";
const SYSTEMCTL: &str = "/usr/bin/systemctl";

pub(crate) struct Paths {
    service: PathBuf,
    timer: PathBuf,
    status: PathBuf,
}

impl Paths {
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub(crate) fn new(home: &Path) -> Self {
        Self::from_homes(
            home,
            env::var_os("XDG_CONFIG_HOME")
                .filter(|path| !path.is_empty())
                .map(PathBuf::from),
            env::var_os("XDG_STATE_HOME")
                .filter(|path| !path.is_empty())
                .map(PathBuf::from),
        )
    }

    fn from_homes(home: &Path, config_home: Option<PathBuf>, state_home: Option<PathBuf>) -> Self {
        let config_home = config_home.unwrap_or_else(|| home.join(".config"));
        let state_home = state_home.unwrap_or_else(|| home.join(".local/state"));
        let units = config_home.join("systemd/user");
        Self {
            service: units.join(SERVICE),
            timer: units.join(TIMER),
            status: state_home.join("codex-cost-meter/status.json"),
        }
    }

    pub(crate) fn service(&self) -> &Path {
        &self.service
    }

    pub(crate) fn timer(&self) -> &Path {
        &self.timer
    }

    pub(crate) fn status(&self) -> &Path {
        &self.status
    }
}

pub(crate) struct Inspection {
    pub(crate) installed: bool,
    pub(crate) active: bool,
    pub(crate) status: Option<Status>,
}

#[derive(Debug, Error)]
pub(crate) enum LifecycleError {
    #[error("could not run systemctl")]
    Tool {
        #[source]
        source: io::Error,
    },
    #[error("could not create the systemd user unit directory")]
    CreateUnitDirectory {
        #[source]
        source: io::Error,
    },
    #[error("could not create a temporary systemd user unit")]
    CreateTemporary {
        #[source]
        source: io::Error,
    },
    #[error("could not write a systemd user unit")]
    WriteUnit {
        #[source]
        source: io::Error,
    },
    #[error("could not flush a systemd user unit")]
    FlushUnit {
        #[source]
        source: io::Error,
    },
    #[error("could not synchronize a systemd user unit")]
    SyncUnit {
        #[source]
        source: io::Error,
    },
    #[error("could not replace a systemd user unit")]
    ReplaceUnit {
        #[source]
        source: io::Error,
    },
    #[error("could not reload systemd user units")]
    Reload,
    #[error("could not enable and start the systemd user timer")]
    Enable,
    #[error("could not disable and stop the systemd user timer")]
    Disable,
    #[error("schedule is not installed")]
    MissingTimer,
    #[error("could not remove the systemd service unit")]
    RemoveService {
        #[source]
        source: io::Error,
    },
    #[error("could not remove the systemd timer unit")]
    RemoveTimer {
        #[source]
        source: io::Error,
    },
    #[error("could not remove schedule status")]
    RemoveStatus {
        #[source]
        source: io::Error,
    },
    #[error("could not read or write schedule status")]
    Status {
        #[source]
        source: StatusError,
    },
    #[error("could not find the current executable")]
    CurrentExecutable {
        #[source]
        source: io::Error,
    },
    #[error("schedule state was removed, but the current executable could not be deleted")]
    ExecutableNotDeleted {
        #[source]
        source: io::Error,
    },
}

#[derive(Clone, Debug)]
struct CommandOutput {
    success: bool,
}

trait CommandRunner {
    fn run(&mut self, program: &Path, arguments: &[OsString]) -> io::Result<CommandOutput>;
}

#[cfg(target_os = "linux")]
struct SystemRunner;

#[cfg(target_os = "linux")]
impl CommandRunner for SystemRunner {
    fn run(&mut self, program: &Path, arguments: &[OsString]) -> io::Result<CommandOutput> {
        let output = Command::new(program).args(arguments).output()?;
        Ok(CommandOutput {
            success: output.status.success(),
        })
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn install(paths: &Paths, options: &InstallOptions) -> Result<(), LifecycleError> {
    let mut runner = SystemRunner;
    install_with_runner(paths, options, &mut runner)
}

#[cfg(target_os = "linux")]
pub(crate) fn inspect(paths: &Paths) -> Result<Inspection, LifecycleError> {
    let mut runner = SystemRunner;
    inspect_with_runner(paths, &mut runner)
}

#[cfg(target_os = "linux")]
pub(crate) fn remove(paths: &Paths) -> Result<(), LifecycleError> {
    let mut runner = SystemRunner;
    remove_with_runner(paths, &mut runner)
}

pub(crate) fn resume(paths: &Paths) -> Result<(), LifecycleError> {
    resume_impl(paths)
}

#[cfg(target_os = "linux")]
pub(crate) fn uninstall(paths: &Paths) -> Result<(), LifecycleError> {
    let current_exe =
        env::current_exe().map_err(|source| LifecycleError::CurrentExecutable { source })?;
    let mut runner = SystemRunner;
    uninstall_with_current_exe_and_runner(paths, &current_exe, &mut runner)
}

fn install_with_runner(
    paths: &Paths,
    options: &InstallOptions,
    runner: &mut impl CommandRunner,
) -> Result<(), LifecycleError> {
    let executable = fs::canonicalize(&options.executable)
        .map_err(|source| LifecycleError::CurrentExecutable { source })?;
    write_unit(paths.service(), &service_unit(options, &executable))?;
    write_unit(paths.timer(), &timer_unit())?;
    if !systemctl(runner, ["--user".into(), "daemon-reload".into()])?.success {
        return Err(LifecycleError::Reload);
    }
    if !systemctl(
        runner,
        [
            "--user".into(),
            "enable".into(),
            "--now".into(),
            TIMER.into(),
        ],
    )?
    .success
    {
        return Err(LifecycleError::Enable);
    }
    Ok(())
}

fn inspect_with_runner(
    paths: &Paths,
    runner: &mut impl CommandRunner,
) -> Result<Inspection, LifecycleError> {
    let active = systemctl(
        runner,
        [
            "--user".into(),
            "is-active".into(),
            "--quiet".into(),
            TIMER.into(),
        ],
    )?
    .success;
    Ok(Inspection {
        installed: paths.service().is_file() && paths.timer().is_file(),
        active,
        status: read_status(paths.status()).map_err(|source| LifecycleError::Status { source })?,
    })
}

fn remove_with_runner(
    paths: &Paths,
    runner: &mut impl CommandRunner,
) -> Result<(), LifecycleError> {
    if paths.timer().is_file()
        && !systemctl(
            runner,
            [
                "--user".into(),
                "disable".into(),
                "--now".into(),
                TIMER.into(),
            ],
        )?
        .success
    {
        return Err(LifecycleError::Disable);
    }
    remove_file_if_present(paths.service())
        .map_err(|source| LifecycleError::RemoveService { source })?;
    remove_file_if_present(paths.timer())
        .map_err(|source| LifecycleError::RemoveTimer { source })?;
    remove_file_if_present(paths.status())
        .map_err(|source| LifecycleError::RemoveStatus { source })?;
    if !systemctl(runner, ["--user".into(), "daemon-reload".into()])?.success {
        return Err(LifecycleError::Reload);
    }
    Ok(())
}

fn resume_impl(paths: &Paths) -> Result<(), LifecycleError> {
    if !paths.timer().is_file() {
        return Err(LifecycleError::MissingTimer);
    }
    let previous =
        read_status(paths.status()).map_err(|source| LifecycleError::Status { source })?;
    write_status(paths.status(), &resume_status(previous))
        .map_err(|source| LifecycleError::Status { source })
}

fn uninstall_with_current_exe_and_runner(
    paths: &Paths,
    current_exe: &Path,
    runner: &mut impl CommandRunner,
) -> Result<(), LifecycleError> {
    remove_with_runner(paths, runner)?;
    let executable = fs::canonicalize(current_exe)
        .map_err(|source| LifecycleError::ExecutableNotDeleted { source })?;
    fs::remove_file(executable).map_err(|source| LifecycleError::ExecutableNotDeleted { source })
}

fn systemctl(
    runner: &mut impl CommandRunner,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<CommandOutput, LifecycleError> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    runner
        .run(Path::new(SYSTEMCTL), &arguments)
        .map_err(|source| LifecycleError::Tool { source })
}

fn service_unit(options: &InstallOptions, executable: &Path) -> String {
    let mut arguments = vec![
        systemd_quote(&executable.as_os_str().to_string_lossy()),
        "schedule".into(),
        "run".into(),
        "--codex-home".into(),
        systemd_quote(&options.codex_home.as_os_str().to_string_lossy()),
        "--idle-minutes".into(),
        options.idle_minutes.to_string(),
        "--limit".into(),
        options.limit.to_string(),
        "--max-runtime".into(),
        runtime_argument(options.max_runtime),
        "--max-width".into(),
        options.max_width.to_string(),
        "--title-metrics".into(),
        systemd_quote(&options.title_metrics),
    ];
    if let Some(reprice_before) = options.reprice_before {
        arguments.extend([
            "--reprice-before".into(),
            reprice_before
                .format(&Rfc3339)
                .expect("OffsetDateTime always formats as RFC 3339"),
        ]);
    }
    arguments.push("--apply".into());
    format!(
        "[Unit]\nDescription=Codex Cost Meter scheduled update\n\n[Service]\nType=oneshot\nExecStart={}\nStandardOutput=null\nStandardError=null\n",
        arguments.join(" ")
    )
}

fn timer_unit() -> String {
    format!(
        "[Unit]\nDescription=Run Codex Cost Meter every five minutes\n\n[Timer]\nOnActiveSec=0\nOnUnitActiveSec=5min\nUnit={SERVICE}\n\n[Install]\nWantedBy=timers.target\n"
    )
}

fn systemd_quote(value: &str) -> String {
    let mut quoted = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '\"' => quoted.push_str("\\\""),
            '$' => quoted.push_str("$$"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            character if character.is_control() => {
                for byte in character.to_string().bytes() {
                    quoted.push_str(&format!("\\x{byte:02x}"));
                }
            }
            character => quoted.push(character),
        }
    }
    quoted.push('\"');
    quoted
}

fn runtime_argument(runtime: Duration) -> String {
    let seconds = runtime.as_secs();
    if seconds.is_multiple_of(60) {
        format!("{}m", seconds / 60)
    } else {
        seconds.to_string()
    }
}

fn write_unit(path: &Path, unit: &str) -> Result<(), LifecycleError> {
    let parent = path.parent().expect("systemd unit path has a parent");
    fs::create_dir_all(parent).map_err(|source| LifecycleError::CreateUnitDirectory { source })?;
    let temporary = parent.join(format!(
        ".{}-{}.tmp",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|source| LifecycleError::CreateTemporary { source })?;
    file.write_all(unit.as_bytes())
        .map_err(|source| LifecycleError::WriteUnit { source })?;
    file.flush()
        .map_err(|source| LifecycleError::FlushUnit { source })?;
    file.sync_all()
        .map_err(|source| LifecycleError::SyncUnit { source })?;
    drop(file);
    fs::rename(temporary, path).map_err(|source| LifecycleError::ReplaceUnit { source })
}

fn remove_file_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(source),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        fs,
        path::{Path, PathBuf},
        time::Duration,
    };

    use tempfile::TempDir;
    use time::OffsetDateTime;

    use super::{
        CommandOutput, CommandRunner, InstallOptions, Paths, inspect_with_runner,
        install_with_runner, remove_with_runner, resume, uninstall_with_current_exe_and_runner,
    };
    use crate::schedule::{after_failure, read_status, write_status};
    use crate::update::FailureClass;

    const SYSTEMCTL: &str = "/usr/bin/systemctl";
    const SERVICE: &str = "io.github.deinspanjer.codex-cost-meter.service";
    const TIMER: &str = "io.github.deinspanjer.codex-cost-meter.timer";

    #[derive(Default)]
    struct FakeRunner {
        calls: Vec<(PathBuf, Vec<OsString>)>,
        outputs: Vec<CommandOutput>,
    }

    impl FakeRunner {
        fn with_outputs(outputs: impl IntoIterator<Item = CommandOutput>) -> Self {
            Self {
                calls: Vec::new(),
                outputs: outputs.into_iter().collect(),
            }
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(
            &mut self,
            program: &Path,
            arguments: &[OsString],
        ) -> std::io::Result<CommandOutput> {
            self.calls.push((program.to_owned(), arguments.to_vec()));
            Ok(self.outputs.remove(0))
        }
    }

    fn output(success: bool) -> CommandOutput {
        CommandOutput { success }
    }

    fn paths(directory: &TempDir) -> Paths {
        Paths::from_homes(
            directory.path(),
            Some(directory.path().join("config")),
            Some(directory.path().join("state")),
        )
    }

    fn options(directory: &TempDir) -> InstallOptions {
        InstallOptions {
            executable: directory.path().join("codex cost meter"),
            codex_home: directory.path().join("Codex Home"),
            idle_minutes: 15,
            limit: 500,
            max_runtime: Duration::from_secs(240),
            max_width: 65,
            title_metrics: "tokens,cost".into(),
            reprice_before: Some(OffsetDateTime::UNIX_EPOCH),
        }
    }

    #[test]
    fn paths_fall_back_to_home_or_use_xdg_roots() {
        let directory = TempDir::new().unwrap();
        let fallback = Paths::from_homes(directory.path(), None, None);
        let configured = paths(&directory);

        assert_eq!(
            fallback.service(),
            directory.path().join(".config/systemd/user").join(SERVICE)
        );
        assert_eq!(
            fallback.status(),
            directory
                .path()
                .join(".local/state/codex-cost-meter/status.json")
        );
        assert_eq!(
            configured.timer(),
            directory.path().join("config/systemd/user").join(TIMER)
        );
        assert_eq!(
            configured.status(),
            directory.path().join("state/codex-cost-meter/status.json")
        );
    }

    #[test]
    fn install_writes_quoted_units_and_uses_the_fixed_systemctl_plan() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        let options = options(&directory);
        fs::write(&options.executable, "binary").unwrap();
        let mut runner = FakeRunner::with_outputs([output(true), output(true)]);

        install_with_runner(&paths, &options, &mut runner).unwrap();

        assert_eq!(
            fs::read_to_string(paths.service()).unwrap(),
            format!(
                "[Unit]\nDescription=Codex Cost Meter scheduled update\n\n[Service]\nType=oneshot\nExecStart=\"{}\" schedule run --codex-home \"{}\" --idle-minutes 15 --limit 500 --max-runtime 4m --max-width 65 --title-metrics \"tokens,cost\" --reprice-before 1970-01-01T00:00:00Z --apply\nStandardOutput=null\nStandardError=null\n",
                fs::canonicalize(&options.executable).unwrap().display(),
                options.codex_home.display(),
            )
        );
        assert_eq!(
            fs::read_to_string(paths.timer()).unwrap(),
            "[Unit]\nDescription=Run Codex Cost Meter every five minutes\n\n[Timer]\nOnActiveSec=0\nOnUnitActiveSec=5min\nUnit=io.github.deinspanjer.codex-cost-meter.service\n\n[Install]\nWantedBy=timers.target\n"
        );
        assert_eq!(
            runner.calls,
            [
                (
                    PathBuf::from(SYSTEMCTL),
                    vec!["--user".into(), "daemon-reload".into()],
                ),
                (
                    PathBuf::from(SYSTEMCTL),
                    vec![
                        "--user".into(),
                        "enable".into(),
                        "--now".into(),
                        TIMER.into()
                    ],
                ),
            ]
        );
    }

    #[test]
    fn remove_is_idempotent_and_leaves_unrelated_files_untouched() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        fs::create_dir_all(paths.timer().parent().unwrap()).unwrap();
        fs::write(paths.service(), "service").unwrap();
        fs::write(paths.timer(), "timer").unwrap();
        fs::create_dir_all(paths.status().parent().unwrap()).unwrap();
        fs::write(paths.status(), "status").unwrap();
        let unrelated = paths.timer().parent().unwrap().join("unrelated.timer");
        fs::write(&unrelated, "unrelated").unwrap();
        let mut runner = FakeRunner::with_outputs([output(true), output(true), output(true)]);

        remove_with_runner(&paths, &mut runner).unwrap();
        remove_with_runner(&paths, &mut runner).unwrap();

        assert!(!paths.service().exists());
        assert!(!paths.timer().exists());
        assert!(!paths.status().exists());
        assert!(unrelated.exists());
        assert_eq!(
            runner.calls,
            [
                (
                    PathBuf::from(SYSTEMCTL),
                    vec![
                        "--user".into(),
                        "disable".into(),
                        "--now".into(),
                        TIMER.into()
                    ],
                ),
                (
                    PathBuf::from(SYSTEMCTL),
                    vec!["--user".into(), "daemon-reload".into()],
                ),
                (
                    PathBuf::from(SYSTEMCTL),
                    vec!["--user".into(), "daemon-reload".into()],
                ),
            ]
        );
    }

    #[test]
    fn inspect_reports_unit_presence_timer_activity_and_status() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        fs::create_dir_all(paths.timer().parent().unwrap()).unwrap();
        fs::write(paths.service(), "service").unwrap();
        fs::write(paths.timer(), "timer").unwrap();
        let status = after_failure(None, FailureClass::Ordinary, OffsetDateTime::UNIX_EPOCH);
        write_status(paths.status(), &status).unwrap();
        let mut runner = FakeRunner::with_outputs([output(true)]);

        let inspection = inspect_with_runner(&paths, &mut runner).unwrap();

        assert!(inspection.installed);
        assert!(inspection.active);
        assert_eq!(inspection.status, Some(status));
        assert_eq!(
            runner.calls,
            [(
                PathBuf::from(SYSTEMCTL),
                vec![
                    "--user".into(),
                    "is-active".into(),
                    "--quiet".into(),
                    TIMER.into()
                ],
            )]
        );
    }

    #[test]
    fn resume_requires_timer_and_clears_only_the_circuit_breaker() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        assert!(resume(&paths).is_err());

        fs::create_dir_all(paths.timer().parent().unwrap()).unwrap();
        fs::write(paths.timer(), "timer").unwrap();
        let previous = after_failure(None, FailureClass::Ordinary, OffsetDateTime::UNIX_EPOCH);
        write_status(paths.status(), &previous).unwrap();

        resume(&paths).unwrap();

        let status = read_status(paths.status()).unwrap().unwrap();
        assert_eq!(status.last_run_at, previous.last_run_at);
        assert!(!status.paused);
        assert_eq!(status.consecutive_failures, 0);
    }

    #[test]
    fn uninstall_removes_only_current_executable_after_scheduler_state() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        fs::create_dir_all(paths.timer().parent().unwrap()).unwrap();
        fs::write(paths.service(), "service").unwrap();
        fs::write(paths.timer(), "timer").unwrap();
        let executable = directory.path().join("codex-cost-meter");
        let unrelated = directory.path().join("other-executable");
        fs::write(&executable, "binary").unwrap();
        fs::write(&unrelated, "binary").unwrap();
        let mut runner = FakeRunner::with_outputs([output(true), output(true)]);

        uninstall_with_current_exe_and_runner(&paths, &executable, &mut runner).unwrap();

        assert!(!executable.exists());
        assert!(unrelated.exists());
    }
}
