use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[cfg(target_os = "windows")]
use std::{env, process::Command};

use thiserror::Error;
use time::format_description::well_known::Rfc3339;

use super::{InstallOptions, Status, StatusError, read_status, resume_status, write_status};

const TASK_NAME: &str = "Codex Cost Meter";
const SCHTASKS: &str = "schtasks.exe";
const WHOAMI: &str = "whoami.exe";
const POWERSHELL: &str = "powershell.exe";
const MISSING_TASK_HRESULT: i32 = 0x8007_0002u32 as i32;
const MAX_SID_BYTES: usize = 184;
const MAX_CREATE_DIAGNOSTIC_CHARS: usize = 256;
const CLEANUP_SCRIPT: &str = r#"param([int]$ParentProcessId, [string]$ExecutablePath)
Wait-Process -Id $ParentProcessId -ErrorAction SilentlyContinue
$deleted = $false
for ($attempt = 0; $attempt -lt 150; $attempt++) {
  try {
    Remove-Item -LiteralPath $ExecutablePath -Force -ErrorAction Stop
    $deleted = $true
    break
  } catch {
    Start-Sleep -Milliseconds 200
  }
}
if ($deleted) {
  Remove-Item -LiteralPath $PSCommandPath -Force -ErrorAction SilentlyContinue
  exit 0
}
exit 1
"#;

pub(crate) struct Paths {
    status: PathBuf,
    definition: PathBuf,
    cleanup: PathBuf,
}

impl Paths {
    pub(crate) fn new(local_app_data: &Path) -> Self {
        let directory = local_app_data.join("codex-cost-meter");
        Self {
            status: directory.join("status.json"),
            definition: directory.join("schedule-definition.xml"),
            cleanup: directory.join("schedule-cleanup.ps1"),
        }
    }

    pub(crate) fn status(&self) -> &Path {
        &self.status
    }

    fn definition(&self) -> &Path {
        &self.definition
    }

    fn cleanup(&self) -> &Path {
        &self.cleanup
    }
}

pub(crate) struct Inspection {
    pub(crate) registered: bool,
    pub(crate) status: Option<Status>,
}

#[derive(Debug, Error)]
pub(crate) enum LifecycleError {
    #[cfg(target_os = "windows")]
    #[error("could not locate the Windows system tools")]
    SystemRoot,
    #[error("could not run a required Windows tool")]
    Tool {
        #[source]
        source: io::Error,
    },
    #[error("could not determine the current user identifier")]
    InvalidUser,
    #[error("could not find the current executable")]
    CurrentExecutable {
        #[source]
        source: io::Error,
    },
    #[error("could not create the scheduler state directory")]
    CreateStateDirectory {
        #[source]
        source: io::Error,
    },
    #[error("could not create a temporary Task Scheduler definition")]
    CreateTemporary {
        #[source]
        source: io::Error,
    },
    #[error("could not write the temporary Task Scheduler definition")]
    WriteDefinition {
        #[source]
        source: io::Error,
    },
    #[error("could not flush the temporary Task Scheduler definition")]
    FlushDefinition {
        #[source]
        source: io::Error,
    },
    #[error("could not synchronize the temporary Task Scheduler definition")]
    SyncDefinition {
        #[source]
        source: io::Error,
    },
    #[error("could not register the Task Scheduler task (exit code {exit_code:?}: {diagnostic})")]
    CreateTask {
        exit_code: Option<i32>,
        diagnostic: String,
    },
    #[error("could not inspect the Task Scheduler task")]
    InspectTask,
    #[error("schedule is not registered")]
    MissingTask,
    #[error("could not remove the Task Scheduler task")]
    DeleteTask,
    #[error("could not remove the temporary Task Scheduler definition")]
    RemoveTemporary {
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
    #[error(
        "schedule state was removed, but cleanup script could not be written; delete the current executable manually"
    )]
    CleanupWrite {
        #[source]
        source: io::Error,
    },
    #[error(
        "schedule state was removed, but cleanup could not be started; delete the current executable manually"
    )]
    CleanupSpawn {
        #[source]
        source: io::Error,
    },
}

#[derive(Clone, Debug)]
struct CommandOutput {
    success: bool,
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

trait CommandRunner {
    fn run(&mut self, tool: &str, arguments: &[OsString]) -> io::Result<CommandOutput>;
    fn spawn(&mut self, tool: &str, arguments: &[OsString]) -> io::Result<()>;
}

#[cfg(target_os = "windows")]
struct SystemRunner {
    system32: PathBuf,
}

#[cfg(target_os = "windows")]
impl SystemRunner {
    fn new() -> Result<Self, LifecycleError> {
        let root = env::var_os("SystemRoot").filter(|value| !value.is_empty());
        Ok(Self {
            system32: PathBuf::from(root.ok_or(LifecycleError::SystemRoot)?).join("System32"),
        })
    }
}

#[cfg(target_os = "windows")]
impl CommandRunner for SystemRunner {
    fn run(&mut self, tool: &str, arguments: &[OsString]) -> io::Result<CommandOutput> {
        let program = self.program(tool);
        let output = Command::new(program).args(arguments).output()?;
        Ok(CommandOutput {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    fn spawn(&mut self, tool: &str, arguments: &[OsString]) -> io::Result<()> {
        Command::new(self.program(tool)).args(arguments).spawn()?;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl SystemRunner {
    fn program(&self, tool: &str) -> PathBuf {
        match tool {
            SCHTASKS => self.system32.join(SCHTASKS),
            WHOAMI => self.system32.join(WHOAMI),
            POWERSHELL => self
                .system32
                .join("WindowsPowerShell")
                .join("v1.0")
                .join(POWERSHELL),
            _ => unreachable!("Windows scheduler only runs fixed system tools"),
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn install(paths: &Paths, options: &InstallOptions) -> Result<(), LifecycleError> {
    let mut runner = SystemRunner::new()?;
    install_with_runner(paths, options, &mut runner)
}

#[cfg(target_os = "windows")]
pub(crate) fn inspect(paths: &Paths) -> Result<Inspection, LifecycleError> {
    let mut runner = SystemRunner::new()?;
    inspect_with_runner(paths, &mut runner)
}

#[cfg(target_os = "windows")]
pub(crate) fn remove(paths: &Paths) -> Result<(), LifecycleError> {
    let mut runner = SystemRunner::new()?;
    remove_with_runner(paths, &mut runner)
}

#[cfg(target_os = "windows")]
pub(crate) fn resume(paths: &Paths) -> Result<(), LifecycleError> {
    let mut runner = SystemRunner::new()?;
    resume_with_runner(paths, &mut runner)
}

#[cfg(target_os = "windows")]
pub(crate) fn uninstall(paths: &Paths) -> Result<(), LifecycleError> {
    let current_exe =
        env::current_exe().map_err(|source| LifecycleError::CurrentExecutable { source })?;
    let mut runner = SystemRunner::new()?;
    uninstall_with_current_exe_and_runner(paths, &current_exe, &mut runner)
}

fn install_with_runner(
    paths: &Paths,
    options: &InstallOptions,
    runner: &mut impl CommandRunner,
) -> Result<(), LifecycleError> {
    remove_definition(paths)?;
    let executable = fs::canonicalize(&options.executable)
        .map_err(|source| LifecycleError::CurrentExecutable { source })?;
    let sid = current_sid(runner)?;
    let result = (|| {
        write_definition(paths, &task_xml(options, &executable, &sid))?;
        let output = run(
            runner,
            SCHTASKS,
            [
                "/Create".into(),
                "/TN".into(),
                TASK_NAME.into(),
                "/XML".into(),
                paths.definition().as_os_str().to_owned(),
                "/F".into(),
            ],
        )?;
        if output.success {
            Ok(())
        } else {
            Err(create_task_error(&output, paths, &sid))
        }
    })();
    let cleanup = remove_definition(paths);
    cleanup?;
    result
}

fn inspect_with_runner(
    paths: &Paths,
    runner: &mut impl CommandRunner,
) -> Result<Inspection, LifecycleError> {
    Ok(Inspection {
        registered: query_registered(runner)?,
        status: read_status(paths.status()).map_err(|source| LifecycleError::Status { source })?,
    })
}

fn remove_with_runner(
    paths: &Paths,
    runner: &mut impl CommandRunner,
) -> Result<(), LifecycleError> {
    if query_registered(runner)? {
        let output = run(
            runner,
            SCHTASKS,
            [
                "/Delete".into(),
                "/TN".into(),
                TASK_NAME.into(),
                "/F".into(),
            ],
        )?;
        if !output.success {
            return Err(LifecycleError::DeleteTask);
        }
    }
    remove_definition(paths)?;
    remove_file_if_present(paths.status()).map_err(|source| LifecycleError::RemoveStatus { source })
}

fn resume_with_runner(
    paths: &Paths,
    runner: &mut impl CommandRunner,
) -> Result<(), LifecycleError> {
    if !query_registered(runner)? {
        return Err(LifecycleError::MissingTask);
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
        .map_err(|source| LifecycleError::CurrentExecutable { source })?;
    write_cleanup(paths).map_err(|source| LifecycleError::CleanupWrite { source })?;
    runner
        .spawn(
            POWERSHELL,
            &[
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-File".into(),
                paths.cleanup().as_os_str().to_owned(),
                std::process::id().to_string().into(),
                executable.into_os_string(),
            ],
        )
        .map_err(|source| LifecycleError::CleanupSpawn { source })
}

fn current_sid(runner: &mut impl CommandRunner) -> Result<String, LifecycleError> {
    let output = run(
        runner,
        WHOAMI,
        ["/user".into(), "/fo".into(), "csv".into(), "/nh".into()],
    )?;
    if !output.success {
        return Err(LifecycleError::InvalidUser);
    }
    extract_sid(&output.stdout).ok_or(LifecycleError::InvalidUser)
}

fn query_registered(runner: &mut impl CommandRunner) -> Result<bool, LifecycleError> {
    let output = run(
        runner,
        SCHTASKS,
        [
            "/Query".into(),
            "/TN".into(),
            TASK_NAME.into(),
            "/HRESULT".into(),
        ],
    )?;
    if output.success {
        Ok(true)
    } else if output.exit_code == Some(MISSING_TASK_HRESULT) {
        Ok(false)
    } else {
        Err(LifecycleError::InspectTask)
    }
}

fn run(
    runner: &mut impl CommandRunner,
    tool: &str,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<CommandOutput, LifecycleError> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    runner
        .run(tool, &arguments)
        .map_err(|source| LifecycleError::Tool { source })
}

fn write_definition(paths: &Paths, xml: &str) -> Result<(), LifecycleError> {
    let parent = paths
        .definition()
        .parent()
        .expect("definition path has a parent");
    fs::create_dir_all(parent).map_err(|source| LifecycleError::CreateStateDirectory { source })?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(paths.definition())
        .map_err(|source| LifecycleError::CreateTemporary { source })?;
    file.write_all(xml.as_bytes())
        .map_err(|source| LifecycleError::WriteDefinition { source })?;
    file.flush()
        .map_err(|source| LifecycleError::FlushDefinition { source })?;
    file.sync_all()
        .map_err(|source| LifecycleError::SyncDefinition { source })
}

fn remove_definition(paths: &Paths) -> Result<(), LifecycleError> {
    remove_file_if_present(paths.definition())
        .map_err(|source| LifecycleError::RemoveTemporary { source })
}

fn write_cleanup(paths: &Paths) -> io::Result<()> {
    let parent = paths.cleanup().parent().expect("cleanup path has a parent");
    fs::create_dir_all(parent)?;
    remove_file_if_present(paths.cleanup())?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(paths.cleanup())?;
    file.write_all(CLEANUP_SCRIPT.as_bytes())?;
    file.flush()?;
    file.sync_all()
}

fn remove_file_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(source),
    }
}

fn create_task_error(output: &CommandOutput, paths: &Paths, sid: &str) -> LifecycleError {
    let diagnostic = String::from_utf8_lossy(&output.stderr)
        .replace(
            paths.definition().to_string_lossy().as_ref(),
            "<temporary-definition>",
        )
        .replace(sid, "<current-user>")
        .split(|character: char| character.is_control() || character.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    LifecycleError::CreateTask {
        exit_code: output.exit_code,
        diagnostic: diagnostic
            .chars()
            .take(MAX_CREATE_DIAGNOSTIC_CHARS)
            .collect(),
    }
}

fn extract_sid(output: &[u8]) -> Option<String> {
    let prefix = b",\"S-1-";
    let start = output
        .windows(prefix.len())
        .position(|window| window == prefix)?
        + 2;
    let remainder = &output[start..];
    let end = remainder.iter().position(|byte| *byte == b'\"')?;
    let sid = remainder.get(..end)?;
    if !remainder[end + 1..]
        .iter()
        .all(|byte| matches!(*byte, b'\r' | b'\n'))
        || sid.len() > MAX_SID_BYTES
        || !sid.starts_with(b"S-1-")
        || sid
            .split(|byte| *byte == b'-')
            .skip(1)
            .any(|part| part.is_empty() || !part.iter().all(u8::is_ascii_digit))
    {
        return None;
    }
    std::str::from_utf8(sid).ok().map(str::to_owned)
}

fn quote_argument(argument: &str) -> String {
    let mut quoted = String::from("\"");
    let mut backslashes = 0;
    for character in argument.chars() {
        if character == '\\' {
            backslashes += 1;
        } else if character == '\"' {
            quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
            quoted.push('\"');
            backslashes = 0;
        } else {
            quoted.push_str(&"\\".repeat(backslashes));
            quoted.push(character);
            backslashes = 0;
        }
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('\"');
    quoted
}

fn task_xml(options: &InstallOptions, executable: &Path, sid: &str) -> String {
    let arguments = scheduled_arguments(options);
    format!(
        "<?xml version=\"1.0\" ?>\n<Task version=\"1.4\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n  <Triggers><RegistrationTrigger><Repetition><Interval>PT5M</Interval></Repetition></RegistrationTrigger></Triggers>\n  <Principals><Principal id=\"Author\"><UserId>{}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>\n  <Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries></Settings>\n  <Actions Context=\"Author\"><Exec><Command>{}</Command><Arguments>{}</Arguments></Exec></Actions>\n</Task>\n",
        xml_text(sid),
        xml_text(&executable.as_os_str().to_string_lossy()),
        xml_text(&arguments),
    )
}

fn scheduled_arguments(options: &InstallOptions) -> String {
    let mut arguments = vec![
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
    arguments
        .iter()
        .map(|argument| quote_argument(argument))
        .collect::<Vec<_>>()
        .join(" ")
}

fn runtime_argument(runtime: std::time::Duration) -> String {
    let seconds = runtime.as_secs();
    if seconds.is_multiple_of(60) {
        format!("{}m", seconds / 60)
    } else {
        seconds.to_string()
    }
}

fn xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, fs, path::PathBuf, time::Duration};

    use tempfile::TempDir;
    use time::OffsetDateTime;

    use super::{
        CommandOutput, CommandRunner, InstallOptions, LifecycleError, Paths, extract_sid,
        inspect_with_runner, install_with_runner, quote_argument, remove_with_runner,
        resume_with_runner, uninstall_with_current_exe_and_runner,
    };
    use crate::schedule::{after_failure, write_status};
    use crate::update::FailureClass;

    const SCHTASKS: &str = "schtasks.exe";
    const WHOAMI: &str = "whoami.exe";

    #[derive(Default)]
    struct FakeRunner {
        calls: Vec<(String, Vec<OsString>)>,
        outputs: Vec<CommandOutput>,
        definition: Option<String>,
        fail_cleanup: bool,
        fail_spawn: bool,
    }

    impl FakeRunner {
        fn with_outputs(outputs: impl IntoIterator<Item = CommandOutput>) -> Self {
            Self {
                calls: Vec::new(),
                outputs: outputs.into_iter().collect(),
                definition: None,
                fail_cleanup: false,
                fail_spawn: false,
            }
        }

        fn fail_cleanup_after_registration(mut self) -> Self {
            self.fail_cleanup = true;
            self
        }

        fn fail_cleanup_spawn(mut self) -> Self {
            self.fail_spawn = true;
            self
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&mut self, tool: &str, arguments: &[OsString]) -> std::io::Result<CommandOutput> {
            if tool == SCHTASKS
                && arguments
                    .first()
                    .is_some_and(|argument| argument == "/Create")
            {
                let index = arguments
                    .iter()
                    .position(|argument| argument == "/XML")
                    .unwrap()
                    + 1;
                let definition = PathBuf::from(&arguments[index]);
                self.definition = Some(fs::read_to_string(&definition).unwrap());
                if self.fail_cleanup {
                    fs::remove_file(&definition).unwrap();
                    fs::create_dir(&definition).unwrap();
                }
            }
            self.calls.push((tool.into(), arguments.to_vec()));
            Ok(self.outputs.remove(0))
        }

        fn spawn(&mut self, tool: &str, arguments: &[OsString]) -> std::io::Result<()> {
            self.calls.push((tool.into(), arguments.to_vec()));
            if self.fail_spawn {
                return Err(std::io::Error::other("powershell failed"));
            }
            Ok(())
        }
    }

    fn output(success: bool, exit_code: i32, stdout: &[u8]) -> CommandOutput {
        CommandOutput {
            success,
            exit_code: Some(exit_code),
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
        }
    }

    fn output_with_stderr(success: bool, exit_code: i32, stderr: &[u8]) -> CommandOutput {
        CommandOutput {
            success,
            exit_code: Some(exit_code),
            stdout: Vec::new(),
            stderr: stderr.to_vec(),
        }
    }

    fn paths(directory: &TempDir) -> Paths {
        Paths::new(directory.path())
    }

    fn options(directory: &TempDir) -> InstallOptions {
        let executable = directory.path().join("codex-cost-meter.exe");
        fs::write(&executable, "binary").unwrap();
        InstallOptions {
            executable,
            codex_home: PathBuf::from(r"C:\Codex Home\&quotable"),
            idle_minutes: 15,
            limit: 500,
            max_runtime: Duration::from_secs(240),
            max_width: 65,
            title_metrics: "tokens,cost".into(),
            reprice_before: Some(OffsetDateTime::UNIX_EPOCH),
        }
    }

    #[test]
    fn quote_argument_follows_the_windows_c_runtime_rules() {
        for (argument, expected) in [
            ("", "\"\""),
            ("two words", "\"two words\""),
            ("a\"b", "\"a\\\"b\""),
            ("trailing\\", "\"trailing\\\\\""),
        ] {
            assert_eq!(quote_argument(argument), expected);
        }
    }

    #[test]
    fn extracts_only_a_bounded_ascii_sid_csv_field() {
        assert_eq!(
            extract_sid(b"\"\x81\x82\x83\",\"S-1-5-21-123-456-789-1001\"\r\n"),
            Some("S-1-5-21-123-456-789-1001".into())
        );
        assert_eq!(extract_sid(b"\"user\",\"S-1-5-x\"\r\n"), None);
        assert_eq!(extract_sid(b"\"user\",\"S-1-5-21-1\" extra"), None);
        assert_eq!(extract_sid(&[b'x'; 512]), None);
    }

    #[test]
    fn query_accepts_only_success_or_the_signed_missing_task_hresult() {
        let directory = TempDir::new().unwrap();
        for (exit_code, expected) in [(0, Ok(false)), (-2_147_024_894, Ok(true)), (5, Err(()))] {
            let mut runner =
                FakeRunner::with_outputs([output(exit_code == 0, exit_code, b"localized")]);
            let result = inspect_with_runner(&paths(&directory), &mut runner)
                .map(|inspection| !inspection.registered)
                .map_err(|_| ());
            assert_eq!(result, expected);
            assert_eq!(
                runner.calls,
                [(
                    SCHTASKS.into(),
                    vec![
                        "/Query".into(),
                        "/TN".into(),
                        "Codex Cost Meter".into(),
                        "/HRESULT".into()
                    ]
                )]
            );
        }
    }

    #[test]
    fn install_replaces_the_fixed_task_with_a_synced_temporary_xml() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        let mut runner = FakeRunner::with_outputs([
            output(true, 0, b"\"user\",\"S-1-5-21-123-456-789-1001\"\r\n"),
            output(true, 0, b""),
        ]);

        let options = options(&directory);
        let executable = fs::canonicalize(&options.executable).unwrap();
        install_with_runner(&paths, &options, &mut runner).unwrap();

        assert!(!paths.definition().exists());
        assert_eq!(
            runner.calls[0],
            (
                WHOAMI.into(),
                vec!["/user".into(), "/fo".into(), "csv".into(), "/nh".into()]
            )
        );
        assert_eq!(runner.calls[1].0, SCHTASKS);
        assert_eq!(
            runner.calls[1].1,
            vec![
                "/Create".into(),
                "/TN".into(),
                "Codex Cost Meter".into(),
                "/XML".into(),
                paths.definition().as_os_str().to_owned(),
                "/F".into()
            ]
        );
        let xml = runner.definition.unwrap();
        assert!(xml.starts_with("<?xml version=\"1.0\" ?>\n"));
        assert!(xml.contains("<RegistrationTrigger><Repetition><Interval>PT5M</Interval></Repetition></RegistrationTrigger>"));
        assert!(xml.contains(
            "<LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel>"
        ));
        assert!(xml.contains("<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>"));
        assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>"));
        assert!(xml.contains(&format!("<Command>{}</Command>", executable.display())));
        assert!(xml.contains("<Arguments>\"schedule\" \"run\" \"--codex-home\" \"C:\\Codex Home\\&amp;quotable\" \"--idle-minutes\" \"15\" \"--limit\" \"500\" \"--max-runtime\" \"4m\" \"--max-width\" \"65\" \"--title-metrics\" \"tokens,cost\" \"--reprice-before\" \"1970-01-01T00:00:00Z\" \"--apply\"</Arguments>"));
        assert!(!xml.contains("StandardOutPath"));
        assert!(!xml.contains("StandardErrorPath"));
    }

    #[test]
    fn install_cleans_up_the_definition_after_a_failed_registration() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        let mut runner = FakeRunner::with_outputs([
            output(true, 0, b"\"user\",\"S-1-5-21-123-456-789-1001\"\r\n"),
            output(false, 1, b"localized failure"),
        ]);

        assert!(matches!(
            install_with_runner(&paths, &options(&directory), &mut runner),
            Err(LifecycleError::CreateTask { .. })
        ));
        assert!(!paths.definition().exists());
    }

    #[test]
    fn failed_registration_reports_a_bounded_redacted_diagnostic_after_cleanup() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        let sid = "S-1-5-21-123-456-789-1001";
        let stderr = format!(
            "Task definition {} for {sid} failed\u{1b}[31m\n{}",
            paths.definition().display(),
            "x".repeat(300),
        );
        let mut runner = FakeRunner::with_outputs([
            output(true, 0, format!("\"user\",\"{sid}\"\r\n").as_bytes()),
            output_with_stderr(false, 42, stderr.as_bytes()),
        ]);

        let error = install_with_runner(&paths, &options(&directory), &mut runner).unwrap_err();

        let LifecycleError::CreateTask {
            exit_code,
            diagnostic,
        } = error
        else {
            panic!("expected registration failure");
        };
        assert_eq!(exit_code, Some(42));
        assert!(
            diagnostic
                .contains("Task definition <temporary-definition> for <current-user> failed [31m")
        );
        assert!(!diagnostic.contains(&paths.definition().display().to_string()));
        assert!(!diagnostic.contains(sid));
        assert!(!diagnostic.chars().any(char::is_control));
        assert!(diagnostic.chars().count() <= 256);
        assert!(!paths.definition().exists());
    }

    #[test]
    fn install_surfaces_cleanup_failure_after_a_failed_registration() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        let mut runner = FakeRunner::with_outputs([
            output(true, 0, b"\"user\",\"S-1-5-21-123-456-789-1001\"\r\n"),
            output(false, 1, b"localized failure"),
        ])
        .fail_cleanup_after_registration();

        assert!(matches!(
            install_with_runner(&paths, &options(&directory), &mut runner),
            Err(LifecycleError::RemoveTemporary { .. })
        ));
        assert!(paths.definition().is_dir());
    }

    #[test]
    fn remove_queries_then_deletes_only_the_fixed_task_and_its_state() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        write_status(
            paths.status(),
            &after_failure(None, FailureClass::Ordinary, OffsetDateTime::UNIX_EPOCH),
        )
        .unwrap();
        fs::write(paths.definition(), "abandoned").unwrap();
        let mut runner = FakeRunner::with_outputs([output(true, 0, b""), output(true, 0, b"")]);

        remove_with_runner(&paths, &mut runner).unwrap();

        assert!(!paths.status().exists());
        assert!(!paths.definition().exists());
        assert_eq!(
            runner.calls[0],
            (
                SCHTASKS.into(),
                vec![
                    "/Query".into(),
                    "/TN".into(),
                    "Codex Cost Meter".into(),
                    "/HRESULT".into()
                ]
            )
        );
        assert_eq!(
            runner.calls[1],
            (
                SCHTASKS.into(),
                vec![
                    "/Delete".into(),
                    "/TN".into(),
                    "Codex Cost Meter".into(),
                    "/F".into()
                ]
            )
        );
    }

    #[test]
    fn uninstall_schedules_fixed_cleanup_without_interpolating_the_executable() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        let executable = directory.path().join("copied executable with spaces.exe");
        fs::write(&executable, "binary").unwrap();
        let mut runner = FakeRunner::with_outputs([output(true, 0, b""), output(true, 0, b"")]);

        uninstall_with_current_exe_and_runner(&paths, &executable, &mut runner).unwrap();

        assert_eq!(runner.calls[2].0, "powershell.exe");
        assert_eq!(
            runner.calls[2].1[..4],
            [
                "-NoProfile",
                "-NonInteractive",
                "-File",
                paths.cleanup().to_str().unwrap()
            ]
        );
        assert_eq!(
            runner.calls[2].1[5],
            fs::canonicalize(&executable).unwrap().as_os_str()
        );
        let script = fs::read_to_string(paths.cleanup()).unwrap();
        assert!(!script.contains(executable.to_str().unwrap()));
        assert!(script.contains("Remove-Item -LiteralPath $ExecutablePath"));
        assert!(script.contains("Remove-Item -LiteralPath $PSCommandPath"));
    }

    #[test]
    fn uninstall_reports_a_distinct_spawn_failure_after_removing_the_task() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        let executable = directory.path().join("copied.exe");
        fs::write(&executable, "binary").unwrap();
        let mut runner = FakeRunner::with_outputs([output(true, 0, b""), output(true, 0, b"")])
            .fail_cleanup_spawn();

        assert!(matches!(
            uninstall_with_current_exe_and_runner(&paths, &executable, &mut runner),
            Err(LifecycleError::CleanupSpawn { .. })
        ));
        assert_eq!(runner.calls[2].0, "powershell.exe");
    }

    #[test]
    fn inspection_reads_bounded_status_and_resume_requires_a_registered_task() {
        let directory = TempDir::new().unwrap();
        let paths = paths(&directory);
        let status = after_failure(None, FailureClass::DiskFull, OffsetDateTime::UNIX_EPOCH);
        write_status(paths.status(), &status).unwrap();
        let mut inspect_runner = FakeRunner::with_outputs([output(true, 0, b"")]);
        assert_eq!(
            inspect_with_runner(&paths, &mut inspect_runner)
                .unwrap()
                .status,
            Some(status)
        );

        let mut absent_runner = FakeRunner::with_outputs([output(false, -2_147_024_894, b"")]);
        assert!(matches!(
            resume_with_runner(&paths, &mut absent_runner),
            Err(LifecycleError::MissingTask)
        ));
    }
}
