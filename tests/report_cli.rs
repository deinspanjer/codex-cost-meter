use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::{Value, json};
use tempfile::TempDir;

fn write_jsonl(path: &Path, rows: &[Value]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        rows.iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>(),
    )
    .unwrap();
}

fn fixture_home() -> TempDir {
    let home = TempDir::new().unwrap();
    fixture_home_at(home.path());
    home
}

fn fixture_home_at(home: &Path) {
    write_jsonl(
        &home.join("sessions/root.jsonl"),
        &[
            json!({
                "type": "session_meta",
                "timestamp": "2026-08-13T12:00:00Z",
                "payload": {"id": "root", "source": "cli", "cwd": "/tmp/project"},
            }),
            json!({"type": "turn_context", "payload": {"model": "gpt-5.6-terra", "effort": "high"}}),
            json!({
                "type": "event_msg",
                "timestamp": "2026-08-13T12:00:00Z",
                "payload": {
                    "type": "token_count",
                    "info": {"last_token_usage": {"input_tokens": 100, "total_tokens": 100}},
                },
            }),
        ],
    );
}

fn report_with_home(home: &Path, thread_id: &str, json: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"));
    command
        .args(["report", thread_id, "--codex-home"])
        .arg(home)
        .env_remove("CODEX_HOME");
    if json {
        command.arg("--json");
    }
    command.output().unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn report_json_uses_explicit_codex_home() {
    let home = fixture_home();
    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "root", "--json", "--codex-home"])
        .arg(home.path())
        .env_remove("CODEX_HOME")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["rollout"]["rollout_id"],
        "root"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn report_uses_codex_home_environment_when_no_flag_is_present() {
    let home = fixture_home();
    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "root", "--json"])
        .env("CODEX_HOME", home.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["rollout"]["rollout_id"],
        "root"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn report_uses_home_codex_directory_as_the_final_default() {
    let root = TempDir::new().unwrap();
    fixture_home_at(&root.path().join(".codex"));
    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "root", "--json"])
        .env("HOME", root.path())
        .env_remove("CODEX_HOME")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["rollout"]["rollout_id"],
        "root"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn report_does_not_use_userprofile_as_a_codex_home_fallback() {
    let root = TempDir::new().unwrap();
    fixture_home_at(&root.path().join(".codex"));
    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "root", "--json"])
        .env_remove("CODEX_HOME")
        .env_remove("HOME")
        .env("USERPROFILE", root.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("HOME is not set"));
}

#[test]
fn missing_thread_id_is_a_clap_usage_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .arg("report")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("Usage:"));
}

#[test]
fn unknown_thread_is_a_single_line_runtime_error() {
    let home = fixture_home();
    let output = report_with_home(home.path(), "unknown", true);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("rollout not found"));
    assert_eq!(stderr(&output).lines().count(), 1);
}

#[test]
fn runtime_errors_sanitize_terminal_control_thread_ids() {
    let home = fixture_home();
    let output = report_with_home(home.path(), "unknown\u{1b}[31m\r\nforged", true);
    let message = stderr(&output);

    assert_eq!(output.status.code(), Some(1));
    assert!(!message.contains('\u{1b}'));
    assert!(!message.contains('\r'));
    assert_eq!(message.lines().count(), 1);
}

#[test]
fn malformed_unknown_and_wrong_type_records_do_not_abort_a_report() {
    let home = fixture_home();
    let path = home.path().join("sessions/noise.jsonl");
    fs::write(
        &path,
        b"not json\n{\"type\":\"unknown\",\"payload\":{}}\n{\"type\":\"session_meta\",\"payload\":{\"id\":17}}\n",
    )
    .unwrap();

    let output = report_with_home(home.path(), "root", true);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(
        report["incomplete_input_warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("malformed JSONL"))
    );
}

#[test]
fn oversized_records_leave_a_visible_incomplete_input_warning() {
    let home = fixture_home();
    fs::write(
        home.path().join("sessions/oversized.jsonl"),
        vec![b'x'; 16 * 1024 * 1024 + 1],
    )
    .unwrap();

    let output = report_with_home(home.path(), "root", true);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(
        report["incomplete_input_warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("oversized JSONL"))
    );
}

#[cfg(unix)]
#[test]
fn symlinked_directories_are_not_scanned() {
    use std::os::unix::fs::symlink;

    let home = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fixture_home_at(outside.path());
    let linked = home.path().join("sessions/link");
    fs::create_dir_all(linked.parent().unwrap()).unwrap();
    symlink(outside.path().join("sessions"), &linked).unwrap();

    let output = report_with_home(home.path(), "root", true);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("rollout not found"));
}

#[test]
fn human_output_sanitizes_terminal_control_text_from_codex_files() {
    let home = fixture_home();
    write_jsonl(
        &home.path().join("session_index.jsonl"),
        &[json!({
            "id": "root",
            "thread_name": "named\u{1b}[31m\r\nforged",
            "updated_at": "2026-08-13T12:00:00Z",
        })],
    );

    let output = report_with_home(home.path(), "root", false);
    let text = String::from_utf8(output.stdout).unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(!text.contains('\u{1b}'));
    assert!(!text.contains('\r'));
    assert!(!text.contains("\nforged"));
}

#[cfg(unix)]
struct PermissionsGuard {
    path: PathBuf,
    original: fs::Permissions,
}

#[cfg(unix)]
impl PermissionsGuard {
    fn remove(path: &Path) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let original = fs::metadata(path).unwrap().permissions();
        let mut denied = original.clone();
        denied.set_mode(0o000);
        fs::set_permissions(path, denied).unwrap();
        Self {
            path: path.to_path_buf(),
            original,
        }
    }
}

#[cfg(unix)]
impl Drop for PermissionsGuard {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.path, self.original.clone());
    }
}

#[cfg(unix)]
#[test]
fn unreadable_scan_directory_is_reported_as_partial_input_when_permissions_apply() {
    let home = fixture_home();
    let denied = home.path().join("sessions/denied");
    fs::create_dir_all(&denied).unwrap();
    fs::write(denied.join("hidden.jsonl"), b"{}").unwrap();
    let _guard = PermissionsGuard::remove(&denied);

    if fs::read_dir(&denied).is_ok() {
        return;
    }
    let output = report_with_home(home.path(), "root", true);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(
        report["incomplete_input_warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning
                .as_str()
                .unwrap()
                .contains("rollout scan could not read"))
    );
}
