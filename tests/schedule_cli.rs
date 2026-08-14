use std::{
    fs::{self, OpenOptions},
    path::Path,
    process::{Command, Output},
};

use rusqlite::Connection;
use tempfile::TempDir;

fn run(home: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(arguments)
        .env("HOME", home)
        .env_remove("CODEX_HOME")
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn status_path(home: &Path) -> std::path::PathBuf {
    home.join("Library/Application Support/codex-cost-meter/status.json")
}

#[test]
fn schedule_parser_errors_and_public_help_do_not_expose_internal_workflow() {
    let home = TempDir::new().unwrap();
    let control = run(
        home.path(),
        &["schedule", "install", "--title-metrics", "cost,\nforged"],
    );
    assert_eq!(control.status.code(), Some(2));
    assert!(control.stdout.is_empty());
    assert_eq!(stderr(&control).lines().count(), 1);
    assert!(stderr(&control).contains("control character"));
    assert!(!stderr(&control).contains("forged"));

    let help = run(home.path(), &["schedule", "--help"]);
    assert!(help.status.success());
    assert!(!String::from_utf8(help.stdout).unwrap().contains("run"));
    assert!(help.stderr.is_empty());
}

#[test]
fn scheduled_failures_record_only_stable_circuit_breaker_results() {
    let home = TempDir::new().unwrap();
    let arguments = ["schedule", "run", "--idle-minutes", "15", "--apply"];
    for count in 1..=3 {
        let output = run(home.path(), &arguments);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            stderr(&output),
            "ordinary_failure: Retry the scheduled update.\n"
        );
        let status = fs::read_to_string(status_path(home.path())).unwrap();
        assert!(status.contains("\"result\":\"ordinary_failure\""));
        assert!(status.contains(&format!("\"consecutive_failures\":{count}")));
    }
    assert!(
        fs::read_to_string(status_path(home.path()))
            .unwrap()
            .contains("\"paused\":true")
    );
    let paused = run(home.path(), &arguments);
    assert!(paused.status.success());
    assert!(paused.stdout.is_empty());
    assert!(paused.stderr.is_empty());

    let severe = TempDir::new().unwrap();
    fs::create_dir_all(severe.path().join(".codex")).unwrap();
    Connection::open(severe.path().join(".codex/state_5.sqlite"))
        .unwrap()
        .execute_batch("CREATE TABLE threads (id TEXT PRIMARY KEY)")
        .unwrap();
    let output = run(severe.path(), &arguments);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "incompatible_schema: Update Codex Cost Meter, then resume the schedule.\n"
    );
    let status = fs::read_to_string(status_path(severe.path())).unwrap();
    assert!(status.contains("\"result\":\"incompatible_schema\""));
    assert!(status.contains("\"consecutive_failures\":1"));
    assert!(status.contains("\"paused\":true"));
    assert!(!status.contains(severe.path().to_str().unwrap()));
}

#[test]
fn paused_and_locked_scheduled_runs_are_silent_without_status_writes() {
    let paused = TempDir::new().unwrap();
    let status = status_path(paused.path());
    fs::create_dir_all(status.parent().unwrap()).unwrap();
    fs::write(
        &status,
        br#"{"last_run_at":null,"result":"ordinary_failure","consecutive_failures":3,"paused":true,"remediation":"Retry the scheduled update."}"#,
    )
    .unwrap();
    let arguments = ["schedule", "run", "--idle-minutes", "15", "--apply"];
    let output = run(paused.path(), &arguments);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(!paused.path().join(".codex").exists());

    let locked = TempDir::new().unwrap();
    let codex_home = locked.path().join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    let lock = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(codex_home.join("thread-cost-title-updater.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let output = run(locked.path(), &arguments);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(!status_path(locked.path()).exists());
}

#[test]
fn status_write_failure_does_not_mask_a_successful_scheduled_update() {
    let home = TempDir::new().unwrap();
    let codex_home = home.path().join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    Connection::open(codex_home.join("state_5.sqlite"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, title TEXT, name TEXT, history_mode TEXT, updated_at INTEGER, first_user_message TEXT)",
        )
        .unwrap();
    let status = status_path(home.path());
    let status_parent = status.parent().unwrap();
    fs::create_dir_all(status_parent).unwrap();
    fs::write(
        &status,
        br#"{"last_run_at":null,"result":"success","consecutive_failures":0,"paused":false,"remediation":"No action required."}"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(status_parent).unwrap().permissions();
        permissions.set_mode(0o500);
        fs::set_permissions(status_parent, permissions).unwrap();
    }

    let output = run(
        home.path(),
        &["schedule", "run", "--idle-minutes", "15", "--apply"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        output.stdout,
        b"update completed; schedule status unavailable\n"
    );
    assert!(output.stderr.is_empty());
}
