#![cfg(target_os = "windows")]

use std::{
    fs::{self, OpenOptions},
    path::Path,
    process::{Command, Output},
};

use tempfile::TempDir;

fn run(local_app_data: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(arguments)
        .env("LOCALAPPDATA", local_app_data)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn windows_schedule_help_hides_run_and_state_never_uses_codex_home() {
    let help = run(TempDir::new().unwrap().path(), &["schedule", "--help"]);
    assert!(help.status.success());
    assert!(!String::from_utf8(help.stdout).unwrap().contains("run"));

    let codex_home = TempDir::new().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["schedule", "status"])
        .env_remove("LOCALAPPDATA")
        .env("CODEX_HOME", codex_home.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        stderr(&output),
        "could not resolve scheduler state: LOCALAPPDATA is not set or empty\n"
    );
}

#[test]
fn windows_status_uses_registered_not_macos_lifecycle_words() {
    let local_app_data = TempDir::new().unwrap();
    let output = run(local_app_data.path(), &["schedule", "status"]);

    assert!(output.status.success(), "{}", stderr(&output));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout
            .lines()
            .any(|line| line == "registered: yes" || line == "registered: no")
    );
    assert!(!stdout.contains("installed:"));
    assert!(!stdout.contains("loaded:"));
}

#[test]
fn paused_and_locked_windows_scheduled_runs_are_silent() {
    let paused = TempDir::new().unwrap();
    let status = paused.path().join("codex-cost-meter/status.json");
    fs::create_dir_all(status.parent().unwrap()).unwrap();
    fs::write(
        &status,
        br#"{"last_run_at":null,"result":"ordinary_failure","consecutive_failures":3,"paused":true,"remediation":"Retry the scheduled update."}"#,
    )
    .unwrap();
    let codex_home = TempDir::new().unwrap();
    let arguments = [
        "schedule",
        "run",
        "--idle-minutes",
        "7",
        "--limit",
        "3",
        "--max-runtime",
        "90",
        "--max-width",
        "80",
        "--title-metrics",
        "cost,output-tokens",
        "--codex-home",
        codex_home.path().to_str().unwrap(),
        "--apply",
    ];
    let output = run(paused.path(), &arguments);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    let locked = TempDir::new().unwrap();
    let lock = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(codex_home.path().join("thread-cost-title-updater.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let output = run(locked.path(), &arguments);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(!locked.path().join("codex-cost-meter/status.json").exists());
}
