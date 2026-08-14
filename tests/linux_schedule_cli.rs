#![cfg(target_os = "linux")]

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use tempfile::TempDir;

fn run(home: &Path, config_home: &Path, state_home: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(arguments)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", config_home)
        .env("XDG_STATE_HOME", state_home)
        .env("CODEX_HOME", home.join("codex-storage"))
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn linux_schedule_help_is_public_and_status_uses_xdg_scheduler_state() {
    let home = TempDir::new().unwrap();
    let config_home = TempDir::new().unwrap();
    let state_home = TempDir::new().unwrap();

    let help = run(
        home.path(),
        config_home.path(),
        state_home.path(),
        &["schedule", "--help"],
    );
    assert!(help.status.success(), "{}", stderr(&help));
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("Manage scheduled idle task-title updates"));
    assert!(
        !help
            .lines()
            .any(|line| line.trim_start().starts_with("run"))
    );

    let units = config_home.path().join("systemd/user");
    fs::create_dir_all(&units).unwrap();
    fs::write(
        units.join("io.github.deinspanjer.codex-cost-meter.service"),
        "[Service]\n",
    )
    .unwrap();
    fs::write(
        units.join("io.github.deinspanjer.codex-cost-meter.timer"),
        "[Timer]\n",
    )
    .unwrap();
    let status = state_home.path().join("codex-cost-meter/status.json");
    fs::create_dir_all(status.parent().unwrap()).unwrap();
    fs::write(
        status,
        br#"{"last_run_at":null,"result":"success","consecutive_failures":0,"paused":false,"remediation":"No action required."}"#,
    )
    .unwrap();

    let output = run(
        home.path(),
        config_home.path(),
        state_home.path(),
        &["schedule", "status"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("installed: yes\n"));
    assert!(
        stdout
            .lines()
            .any(|line| line == "active: yes" || line == "active: no")
    );
    assert!(stdout.contains("last run: never\nresult: success\n"));
    assert!(!stdout.contains("systemctl"));
    assert!(!stdout.contains("ExecStart"));
}
