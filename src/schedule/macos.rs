use std::{
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use thiserror::Error;
use time::format_description::well_known::Rfc3339;

use super::{InstallOptions, Status, StatusError, read_status, resume_status, write_status};

const LABEL: &str = "io.github.deinspanjer.codex-cost-meter";
const LAUNCHCTL: &str = "/bin/launchctl";
const ID: &str = "/usr/bin/id";
const MISSING_BOOTOUT_EXIT_CODE: i32 = 3;
const MISSING_PRINT_EXIT_CODES: [i32; 2] = [3, 113];

pub(crate) struct Paths {
    plist: PathBuf,
    status: PathBuf,
}

impl Paths {
    pub(crate) fn new(home: &Path) -> Self {
        Self {
            plist: home
                .join("Library")
                .join("LaunchAgents")
                .join(format!("{LABEL}.plist")),
            status: home
                .join("Library")
                .join("Application Support")
                .join("codex-cost-meter")
                .join("status.json"),
        }
    }

    pub(crate) fn plist(&self) -> &Path {
        &self.plist
    }

    pub(crate) fn status(&self) -> &Path {
        &self.status
    }
}

pub(crate) struct Inspection {
    pub(crate) installed: bool,
    pub(crate) loaded: bool,
    pub(crate) status: Option<Status>,
}

#[derive(Debug, Error)]
pub(crate) enum LifecycleError {
    #[error("could not run a required macOS tool")]
    Tool {
        #[source]
        source: io::Error,
    },
    #[error("could not determine the current user identifier")]
    InvalidUser,
    #[error("could not create the LaunchAgents directory")]
    CreateLaunchAgents {
        #[source]
        source: io::Error,
    },
    #[error("could not create a temporary LaunchAgent property list")]
    CreateTemporary {
        #[source]
        source: io::Error,
    },
    #[error("could not write the LaunchAgent property list")]
    WritePlist {
        #[source]
        source: io::Error,
    },
    #[error("could not flush the LaunchAgent property list")]
    FlushPlist {
        #[source]
        source: io::Error,
    },
    #[error("could not synchronize the LaunchAgent property list")]
    SyncPlist {
        #[source]
        source: io::Error,
    },
    #[error("could not replace the LaunchAgent property list")]
    ReplacePlist {
        #[source]
        source: io::Error,
    },
    #[error("could not stop the existing LaunchAgent")]
    Bootout,
    #[error("could not start the LaunchAgent")]
    Bootstrap,
    #[error("could not inspect the LaunchAgent")]
    Inspect,
    #[error("schedule is not installed")]
    MissingPlist,
    #[error("could not remove the LaunchAgent property list")]
    RemovePlist {
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
    exit_code: Option<i32>,
    stdout: Vec<u8>,
}

trait CommandRunner {
    fn run(&mut self, program: &Path, arguments: &[OsString]) -> io::Result<CommandOutput>;
}

struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&mut self, program: &Path, arguments: &[OsString]) -> io::Result<CommandOutput> {
        let output = Command::new(program).args(arguments).output()?;
        Ok(CommandOutput {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: output.stdout,
        })
    }
}

pub(crate) fn install(paths: &Paths, options: &InstallOptions) -> Result<(), LifecycleError> {
    let mut runner = SystemRunner;
    install_with_runner(paths, options, &mut runner)
}

pub(crate) fn inspect(paths: &Paths) -> Result<Inspection, LifecycleError> {
    let mut runner = SystemRunner;
    inspect_with_runner(paths, &mut runner)
}

pub(crate) fn remove(paths: &Paths) -> Result<(), LifecycleError> {
    let mut runner = SystemRunner;
    remove_with_runner(paths, &mut runner)
}

pub(crate) fn uninstall(paths: &Paths) -> Result<(), LifecycleError> {
    let current_exe =
        env::current_exe().map_err(|source| LifecycleError::CurrentExecutable { source })?;
    let mut runner = SystemRunner;
    uninstall_with_current_exe_and_runner(paths, &current_exe, &mut runner)
}

pub(crate) fn resume(paths: &Paths) -> Result<(), LifecycleError> {
    if !paths.plist.is_file() {
        return Err(LifecycleError::MissingPlist);
    }
    let previous =
        read_status(&paths.status).map_err(|source| LifecycleError::Status { source })?;
    write_status(&paths.status, &resume_status(previous))
        .map_err(|source| LifecycleError::Status { source })
}

fn install_with_runner(
    paths: &Paths,
    options: &InstallOptions,
    runner: &mut impl CommandRunner,
) -> Result<(), LifecycleError> {
    let canonical_executable = fs::canonicalize(&options.executable)
        .map_err(|source| LifecycleError::CurrentExecutable { source })?;
    write_plist(&paths.plist, &property_list(options, &canonical_executable))?;
    let uid = current_uid(runner)?;
    bootout(runner, &uid)?;
    bootstrap(runner, &uid, &paths.plist)
}

fn inspect_with_runner(
    paths: &Paths,
    runner: &mut impl CommandRunner,
) -> Result<Inspection, LifecycleError> {
    let uid = current_uid(runner)?;
    let output = run(runner, LAUNCHCTL, ["print".into(), job_target(&uid).into()])?;
    if !output.success && !MISSING_PRINT_EXIT_CODES.contains(&output.exit_code.unwrap_or_default())
    {
        return Err(LifecycleError::Inspect);
    }
    Ok(Inspection {
        installed: paths.plist.is_file(),
        loaded: output.success,
        status: read_status(&paths.status).map_err(|source| LifecycleError::Status { source })?,
    })
}

fn remove_with_runner(
    paths: &Paths,
    runner: &mut impl CommandRunner,
) -> Result<(), LifecycleError> {
    let uid = current_uid(runner)?;
    bootout(runner, &uid)?;
    remove_file_if_present(&paths.plist)
        .map_err(|source| LifecycleError::RemovePlist { source })?;
    remove_file_if_present(&paths.status).map_err(|source| LifecycleError::RemoveStatus { source })
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

fn property_list(options: &InstallOptions, executable: &Path) -> String {
    let mut arguments = vec![
        executable.as_os_str().to_string_lossy().into_owned(),
        "schedule".into(),
        "run".into(),
        "--codex-home".into(),
        options
            .codex_home
            .as_os_str()
            .to_string_lossy()
            .into_owned(),
        "--idle-minutes".into(),
        options.idle_minutes.to_string(),
        "--limit".into(),
        options.limit.to_string(),
        "--max-runtime".into(),
        runtime_argument(options.max_runtime),
        "--max-width".into(),
        options.max_width.to_string(),
        "--title-metrics".into(),
        options.title_metrics.clone(),
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
    let arguments = arguments
        .into_iter()
        .map(|argument| format!("    <string>{}</string>", xml_text(&argument)))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>{LABEL}</string>\n  <key>ProgramArguments</key>\n  <array>\n{arguments}\n  </array>\n  <key>RunAtLoad</key>\n  <true/>\n  <key>StartInterval</key>\n  <integer>300</integer>\n  <key>ProcessType</key>\n  <string>Background</string>\n</dict>\n</plist>\n"
    )
}

fn xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn runtime_argument(runtime: Duration) -> String {
    let seconds = runtime.as_secs();
    if seconds.is_multiple_of(60) {
        format!("{}m", seconds / 60)
    } else {
        seconds.to_string()
    }
}

fn write_plist(path: &Path, plist: &str) -> Result<(), LifecycleError> {
    let parent = path.parent().expect("LaunchAgent path has a parent");
    fs::create_dir_all(parent).map_err(|source| LifecycleError::CreateLaunchAgents { source })?;
    let temporary = parent.join(format!(".{LABEL}-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|source| LifecycleError::CreateTemporary { source })?;
    file.write_all(plist.as_bytes())
        .map_err(|source| LifecycleError::WritePlist { source })?;
    file.flush()
        .map_err(|source| LifecycleError::FlushPlist { source })?;
    file.sync_all()
        .map_err(|source| LifecycleError::SyncPlist { source })?;
    drop(file);
    fs::rename(temporary, path).map_err(|source| LifecycleError::ReplacePlist { source })
}

fn current_uid(runner: &mut impl CommandRunner) -> Result<String, LifecycleError> {
    let output = run(runner, ID, ["-u".into()])?;
    if !output.success {
        return Err(LifecycleError::InvalidUser);
    }
    let uid = std::str::from_utf8(&output.stdout)
        .ok()
        .map(str::trim)
        .filter(|uid| !uid.is_empty() && uid.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or(LifecycleError::InvalidUser)?;
    Ok(uid.into())
}

fn bootout(runner: &mut impl CommandRunner, uid: &str) -> Result<(), LifecycleError> {
    let output = run(
        runner,
        LAUNCHCTL,
        ["bootout".into(), job_target(uid).into()],
    )?;
    if output.success || output.exit_code == Some(MISSING_BOOTOUT_EXIT_CODE) {
        Ok(())
    } else {
        Err(LifecycleError::Bootout)
    }
}

fn bootstrap(
    runner: &mut impl CommandRunner,
    uid: &str,
    plist: &Path,
) -> Result<(), LifecycleError> {
    let output = run(
        runner,
        LAUNCHCTL,
        [
            "bootstrap".into(),
            format!("gui/{uid}").into(),
            plist.as_os_str().to_owned(),
        ],
    )?;
    if output.success {
        Ok(())
    } else {
        Err(LifecycleError::Bootstrap)
    }
}

fn run(
    runner: &mut impl CommandRunner,
    program: impl AsRef<Path>,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<CommandOutput, LifecycleError> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    runner
        .run(program.as_ref(), &arguments)
        .map_err(|source| LifecycleError::Tool { source })
}

fn job_target(uid: &str) -> String {
    format!("gui/{uid}/{LABEL}")
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

    use super::{
        CommandOutput, CommandRunner, InstallOptions, LifecycleError, Paths, inspect_with_runner,
        install_with_runner, remove_with_runner, resume, uninstall_with_current_exe_and_runner,
    };
    use crate::schedule::{ResultCode, Status, after_failure, read_status, write_status};
    use crate::update::FailureClass;
    use time::OffsetDateTime;

    const LAUNCHCTL: &str = "/bin/launchctl";
    const ID: &str = "/usr/bin/id";

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

    fn output(success: bool, stdout: &str) -> CommandOutput {
        CommandOutput {
            success,
            exit_code: success.then_some(0).or(Some(3)),
            stdout: stdout.as_bytes().to_vec(),
        }
    }

    fn failed_output(exit_code: i32) -> CommandOutput {
        CommandOutput {
            success: false,
            exit_code: Some(exit_code),
            stdout: Vec::new(),
        }
    }

    fn paths(directory: &TempDir) -> Paths {
        Paths::new(directory.path())
    }

    #[test]
    fn remove_attempts_exact_label_bootout_when_schedule_files_are_absent() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        let mut runner = FakeRunner::with_outputs([output(true, "501\n"), failed_output(3)]);

        remove_with_runner(&paths, &mut runner).unwrap();

        assert_eq!(
            runner.calls,
            [
                (PathBuf::from(ID), vec![OsString::from("-u")]),
                (
                    PathBuf::from(LAUNCHCTL),
                    vec![
                        OsString::from("bootout"),
                        OsString::from("gui/501/io.github.deinspanjer.codex-cost-meter"),
                    ],
                ),
            ]
        );
        assert_eq!(
            paths.status(),
            directory
                .path()
                .join("Library/Application Support/codex-cost-meter/status.json")
        );
    }

    fn options(directory: &TempDir) -> InstallOptions {
        InstallOptions {
            executable: directory.path().join("codex-cost-meter"),
            codex_home: directory.path().join(".codex"),
            idle_minutes: 15,
            limit: 500,
            max_runtime: Duration::from_secs(240),
            max_width: 65,
            title_metrics: "cost,total-tokens".into(),
            reprice_before: None,
        }
    }

    fn successful_runner() -> FakeRunner {
        FakeRunner::with_outputs([output(true, "501\n"), output(false, ""), output(true, "")])
    }

    #[test]
    fn plist_escapes_text_and_preserves_scheduled_argument_order() {
        let directory = TempDir::new().unwrap();
        let mut options = options(&directory);
        options.executable = PathBuf::from("/tmp/codex&<meter>");
        options.codex_home = PathBuf::from("/tmp/home&<codex>");
        options.title_metrics = "cost&<total>".into();

        let plist = super::property_list(&options, &options.executable);

        for escaped in [
            "/tmp/codex&amp;&lt;meter&gt;",
            "/tmp/home&amp;&lt;codex&gt;",
            "cost&amp;&lt;total&gt;",
        ] {
            assert!(plist.contains(escaped), "missing {escaped:?}");
        }
        assert!(plist.contains(
            "<key>Label</key>\n  <string>io.github.deinspanjer.codex-cost-meter</string>"
        ));
        assert!(plist.contains("<key>RunAtLoad</key>\n  <true/>"));
        assert!(plist.contains("<key>StartInterval</key>\n  <integer>300</integer>"));
        assert!(plist.contains("<key>ProcessType</key>\n  <string>Background</string>"));
        assert!(!plist.contains("StandardOutPath"));
        assert!(!plist.contains("StandardErrorPath"));
        assert_eq!(
            string_elements(&plist),
            [
                "/tmp/codex&<meter>",
                "schedule",
                "run",
                "--codex-home",
                "/tmp/home&<codex>",
                "--idle-minutes",
                "15",
                "--limit",
                "500",
                "--max-runtime",
                "4m",
                "--max-width",
                "65",
                "--title-metrics",
                "cost&<total>",
                "--apply",
            ]
        );
    }

    #[test]
    fn install_replaces_only_the_plist_and_uses_the_fixed_command_plan() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        fs::create_dir_all(paths.plist.parent().unwrap()).unwrap();
        fs::write(&paths.plist, "old plist").unwrap();
        fs::write(directory.path().join("codex-cost-meter"), "binary").unwrap();
        let mut runner = successful_runner();

        install_with_runner(&paths, &options(&directory), &mut runner).unwrap();

        assert!(
            fs::read_to_string(&paths.plist)
                .unwrap()
                .contains("<key>ProgramArguments</key>")
        );
        assert!(
            fs::read_dir(paths.plist.parent().unwrap())
                .unwrap()
                .all(|entry| entry.unwrap().path() == paths.plist)
        );
        assert_eq!(
            runner.calls,
            [
                (PathBuf::from(ID), vec![OsString::from("-u")]),
                (
                    PathBuf::from(LAUNCHCTL),
                    vec![
                        OsString::from("bootout"),
                        OsString::from("gui/501/io.github.deinspanjer.codex-cost-meter"),
                    ],
                ),
                (
                    PathBuf::from(LAUNCHCTL),
                    vec![
                        OsString::from("bootstrap"),
                        OsString::from("gui/501"),
                        paths.plist.as_os_str().to_owned(),
                    ],
                ),
            ]
        );
    }

    #[test]
    fn install_does_not_treat_an_unrelated_bootout_failure_as_missing() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        fs::write(directory.path().join("codex-cost-meter"), "binary").unwrap();
        let mut runner = FakeRunner::with_outputs([output(true, "501\n"), failed_output(1)]);

        let error = install_with_runner(&paths, &options(&directory), &mut runner).unwrap_err();

        assert!(matches!(error, LifecycleError::Bootout));
        assert_eq!(runner.calls.len(), 2);
    }

    #[test]
    fn inspect_reports_only_existence_loaded_state_and_bounded_status() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        fs::create_dir_all(paths.plist.parent().unwrap()).unwrap();
        fs::write(&paths.plist, "plist").unwrap();
        let status = after_failure(None, FailureClass::Ordinary, OffsetDateTime::UNIX_EPOCH);
        write_status(&paths.status, &status).unwrap();
        let mut runner = FakeRunner::with_outputs([output(true, "501\n"), output(true, "private")]);

        let inspection = inspect_with_runner(&paths, &mut runner).unwrap();

        assert!(inspection.installed);
        assert!(inspection.loaded);
        assert_eq!(inspection.status, Some(status));
        assert_eq!(
            runner.calls,
            [
                (PathBuf::from(ID), vec![OsString::from("-u")]),
                (
                    PathBuf::from(LAUNCHCTL),
                    vec![
                        OsString::from("print"),
                        OsString::from("gui/501/io.github.deinspanjer.codex-cost-meter"),
                    ],
                ),
            ]
        );
    }

    #[test]
    fn inspect_treats_launchctl_print_exit_113_as_missing_without_reading_output() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        let mut runner = FakeRunner::with_outputs([
            output(true, "501\n"),
            CommandOutput {
                success: false,
                exit_code: Some(113),
                stdout: b"private scheduler output".to_vec(),
            },
        ]);

        let inspection = inspect_with_runner(&paths, &mut runner).unwrap();

        assert!(!inspection.installed);
        assert!(!inspection.loaded);
        assert_eq!(inspection.status, None);
        assert_eq!(
            runner.calls,
            [
                (PathBuf::from(ID), vec![OsString::from("-u")]),
                (
                    PathBuf::from(LAUNCHCTL),
                    vec![
                        OsString::from("print"),
                        OsString::from("gui/501/io.github.deinspanjer.codex-cost-meter"),
                    ],
                ),
            ]
        );
    }

    #[test]
    fn remove_is_idempotent_and_deletes_only_schedule_files() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        fs::create_dir_all(paths.plist.parent().unwrap()).unwrap();
        fs::write(&paths.plist, "plist").unwrap();
        write_status(
            &paths.status,
            &after_failure(None, FailureClass::Ordinary, OffsetDateTime::UNIX_EPOCH),
        )
        .unwrap();
        let neighboring = directory.path().join("keep-me");
        fs::write(&neighboring, "keep").unwrap();
        let mut runner = FakeRunner::with_outputs([output(true, "501\n"), output(false, "")]);

        remove_with_runner(&paths, &mut runner).unwrap();

        assert!(!paths.plist.exists());
        assert!(!paths.status.exists());
        assert!(neighboring.exists());
        assert_eq!(runner.calls.len(), 2);
        let mut second_runner =
            FakeRunner::with_outputs([output(true, "501\n"), output(false, "")]);
        remove_with_runner(&paths, &mut second_runner).unwrap();
        assert_eq!(second_runner.calls.len(), 2);
    }

    #[test]
    fn resume_requires_an_installed_plist_and_never_re_registers_it() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);

        assert!(matches!(resume(&paths), Err(LifecycleError::MissingPlist)));

        fs::create_dir_all(paths.plist.parent().unwrap()).unwrap();
        fs::write(&paths.plist, "plist").unwrap();
        let paused = after_failure(None, FailureClass::DiskFull, OffsetDateTime::UNIX_EPOCH);
        write_status(&paths.status, &paused).unwrap();

        resume(&paths).unwrap();

        assert_eq!(
            read_status(&paths.status).unwrap(),
            Some(Status {
                last_run_at: Some(OffsetDateTime::UNIX_EPOCH),
                result: ResultCode::Success,
                consecutive_failures: 0,
                paused: false,
                remediation: "No action required.".into(),
            })
        );
    }

    #[test]
    fn uninstall_removes_only_the_current_executable_after_schedule_removal() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        fs::create_dir_all(paths.plist.parent().unwrap()).unwrap();
        fs::write(&paths.plist, "plist").unwrap();
        let executable = directory.path().join("codex-cost-meter");
        let neighboring = directory.path().join("neighboring-file");
        fs::write(&executable, "binary").unwrap();
        fs::write(&neighboring, "keep").unwrap();
        let mut runner = FakeRunner::with_outputs([output(true, "501\n"), output(false, "")]);

        uninstall_with_current_exe_and_runner(&paths, &executable, &mut runner).unwrap();

        assert!(!paths.plist.exists());
        assert!(!executable.exists());
        assert!(neighboring.exists());
        assert!(directory.path().exists());
    }

    #[test]
    fn uninstall_reports_partial_outcome_when_executable_cannot_be_deleted() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        fs::create_dir_all(paths.plist.parent().unwrap()).unwrap();
        fs::write(&paths.plist, "plist").unwrap();
        let executable_directory = directory.path().join("codex-cost-meter");
        fs::create_dir(&executable_directory).unwrap();
        let mut runner = FakeRunner::with_outputs([output(true, "501\n"), output(false, "")]);

        let error =
            uninstall_with_current_exe_and_runner(&paths, &executable_directory, &mut runner)
                .unwrap_err();

        assert!(matches!(error, LifecycleError::ExecutableNotDeleted { .. }));
        assert!(!paths.plist.exists());
        assert!(executable_directory.exists());
    }

    fn string_elements(plist: &str) -> Vec<String> {
        plist
            .split_once("<key>ProgramArguments</key>")
            .unwrap()
            .1
            .split_once("</array>")
            .unwrap()
            .0
            .split("<string>")
            .skip(1)
            .map(|entry| entry.split_once("</string>").unwrap().0)
            .map(|value| {
                value
                    .replace("&gt;", ">")
                    .replace("&lt;", "<")
                    .replace("&amp;", "&")
            })
            .collect()
    }
}
