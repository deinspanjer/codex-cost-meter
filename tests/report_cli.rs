use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

#[cfg(unix)]
use std::path::PathBuf;

use rusqlite::{Connection, params};
use serde_json::{Value, json};
use tempfile::TempDir;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

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
            json!({"type": "event_msg", "payload": {"type": "thread_settings_applied", "thread_settings": {"service_tier": "standard"}}}),
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

fn dated_rollout(
    id: &str,
    parent: Option<&str>,
    cwd: &Path,
    model: &str,
    started_at: &str,
    ended_at: &str,
    input: u64,
) -> Vec<Value> {
    let mut metadata = json!({"id": id, "source": "cli", "cwd": cwd});
    if let Some(parent) = parent {
        metadata["parent_thread_id"] = json!(parent);
        metadata["source"] = json!({"subagent": {"other": "worker"}});
    }
    vec![
        json!({"type": "session_meta", "timestamp": started_at, "payload": metadata}),
        json!({"type": "event_msg", "payload": {"type": "thread_settings_applied", "thread_settings": {"service_tier": "standard"}}}),
        json!({"type": "turn_context", "payload": {"turn_id": "turn-1", "model": model, "effort": "high"}}),
        json!({"type": "event_msg", "timestamp": started_at, "payload": {"type": "task_started", "turn_id": "turn-1"}}),
        json!({
            "type": "event_msg",
            "timestamp": started_at,
            "payload": {
                "type": "token_count",
                "info": {"last_token_usage": {"input_tokens": input, "total_tokens": input}},
            },
        }),
        json!({"type": "event_msg", "timestamp": ended_at, "payload": {"type": "task_complete", "turn_id": "turn-1"}}),
    ]
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

#[test]
fn corpus_and_project_reports_share_date_grouping_and_empty_bucket_rules() {
    let home = TempDir::new().unwrap();
    let workspace = home.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    write_jsonl(
        &home.path().join("sessions/root.jsonl"),
        &dated_rollout(
            "root",
            None,
            &workspace,
            "gpt-5.6-terra",
            "2026-08-01T23:59:00Z",
            "2026-08-02T00:01:00Z",
            100,
        ),
    );
    write_jsonl(
        &home.path().join("sessions/child.jsonl"),
        &dated_rollout(
            "child",
            Some("root"),
            &workspace,
            "gpt-5.6-sol",
            "2026-08-03T12:00:00Z",
            "2026-08-03T12:00:30Z",
            50,
        ),
    );

    let corpus = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args([
            "report",
            "--all",
            "--since",
            "2026-08-01",
            "--through",
            "2026-08-02",
            "--group-by",
            "day,rollout-type,model",
            "--include-empty",
            "--json",
            "--codex-home",
        ])
        .arg(home.path())
        .env("TZ", "UTC")
        .output()
        .unwrap();
    assert!(corpus.status.success(), "{}", stderr(&corpus));
    let corpus: Value = serde_json::from_slice(&corpus.stdout).unwrap();
    assert_eq!(corpus["tree"]["rollout_count"], 2);
    assert_eq!(corpus["tree"]["input_tokens"], 100);
    assert_eq!(corpus["tree"]["turns"], 1);
    assert_eq!(corpus["tree"]["total_turn_duration_seconds"], 120.0);
    assert_eq!(corpus["by_rollout_type"]["root"]["input_tokens"], 100);
    assert_eq!(
        corpus["by_rollout_type"]["subagent:worker"]["input_tokens"],
        0
    );
    let groups = corpus["groups"].as_array().unwrap();
    assert!(groups.iter().any(|group| {
        group["period"] == "2026-08-01"
            && group["rollout_type"] == "root"
            && group["model"] == "gpt-5.6-terra"
            && group["stats"]["input_tokens"] == 100
            && group["stats"]["total_turn_duration_seconds"] == 120.0
    }));
    assert!(!groups.iter().any(|group| {
        group["period"] == "2026-08-01"
            && group.get("rollout_type").is_none()
            && group.get("model").is_none()
    }));
    assert!(groups.iter().any(|group| {
        group["period"] == "2026-08-02"
            && group.get("rollout_type").is_none()
            && group.get("model").is_none()
            && group["stats"]["rollout_count"] == 0
    }));
    assert_eq!(
        groups
            .iter()
            .map(|group| group["stats"]["input_tokens"].as_u64().unwrap())
            .sum::<u64>(),
        corpus["tree"]["input_tokens"].as_u64().unwrap()
    );

    let human = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args([
            "report",
            "--all",
            "--since",
            "2026-08-01",
            "--through",
            "2026-08-02",
            "--group-by",
            "day",
            "--include-empty",
            "--codex-home",
        ])
        .arg(home.path())
        .env("TZ", "UTC")
        .output()
        .unwrap();
    assert!(human.status.success(), "{}", stderr(&human));
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("Codex corpus report"));
    assert!(human.contains("Selected range (2026-08-01 through 2026-08-02)"));
    assert!(human.contains("Groups\n"));
    assert!(human.contains("2026-08-02"));

    let project = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "--project"])
        .arg(&workspace)
        .args([
            "--since",
            "2026-08-01",
            "--through",
            "2026-08-02",
            "--json",
            "--codex-home",
        ])
        .arg(home.path())
        .env("TZ", "UTC")
        .output()
        .unwrap();
    assert!(project.status.success(), "{}", stderr(&project));
    let project: Value = serde_json::from_slice(&project.stdout).unwrap();
    assert_eq!(project["selection"]["workspace_fallbacks"], 1);
    assert_eq!(project["tree"]["rollout_count"], 2);
    assert_eq!(project["tree"]["input_tokens"], 100);
}

#[test]
fn corpus_dates_prune_only_metadata_backed_out_of_range_rollouts() {
    let home = TempDir::new().unwrap();
    let workspace = home.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let fixtures = [
        ("old", 1),
        ("future", 2),
        ("since-boundary", 4),
        ("through-boundary", 8),
        ("metadata-gap", 16),
        ("unindexed", 32),
    ];
    for (id, input) in fixtures {
        write_jsonl(
            &home.path().join(format!("sessions/{id}.jsonl")),
            &dated_rollout(
                id,
                None,
                &workspace,
                "gpt-5.6-terra",
                "2026-08-10T12:00:00Z",
                "2026-08-10T12:00:01Z",
                input,
            ),
        );
    }
    let timestamp = |value: &str| {
        OffsetDateTime::parse(value, &Rfc3339)
            .unwrap()
            .unix_timestamp()
    };
    let database = Connection::open(home.path().join("state_5.sqlite")).unwrap();
    database
        .execute_batch(
            "CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                created_at INTEGER,
                updated_at INTEGER
             );",
        )
        .unwrap();
    for (id, created_at, updated_at) in [
        ("old", "2026-07-01T00:00:00Z", "2026-07-31T23:59:59Z"),
        ("future", "2026-09-01T00:00:00Z", "2026-09-02T00:00:00Z"),
        (
            "since-boundary",
            "2026-07-01T00:00:00Z",
            "2026-08-01T00:00:00Z",
        ),
        (
            "through-boundary",
            "2026-08-31T23:59:59Z",
            "2026-09-01T00:00:00Z",
        ),
    ] {
        database
            .execute(
                "INSERT INTO threads VALUES (?1, ?2, ?3, ?4)",
                params![
                    id,
                    home.path()
                        .join(format!("sessions/{id}.jsonl"))
                        .to_string_lossy(),
                    timestamp(created_at),
                    timestamp(updated_at),
                ],
            )
            .unwrap();
    }
    database
        .execute(
            "INSERT INTO threads VALUES (?1, ?2, NULL, NULL)",
            params![
                "metadata-gap",
                home.path()
                    .join("sessions/metadata-gap.jsonl")
                    .to_string_lossy()
            ],
        )
        .unwrap();
    drop(database);

    let run = |bounds: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
            .args(["report", "--all", "--json", "--codex-home"])
            .arg(home.path())
            .args(bounds)
            .env("TZ", "UTC")
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", stderr(&output));
        output
    };
    let since = run(&["--since", "2026-08-01"]);
    let since_report: Value = serde_json::from_slice(&since.stdout).unwrap();
    assert_eq!(since_report["tree"]["rollout_count"], 5);
    assert_eq!(since_report["tree"]["input_tokens"], 62);

    let through = run(&["--through", "2026-08-31"]);
    let through_report: Value = serde_json::from_slice(&through.stdout).unwrap();
    assert_eq!(through_report["tree"]["rollout_count"], 5);
    assert_eq!(through_report["tree"]["input_tokens"], 61);

    fs::remove_file(home.path().join("sessions/old.jsonl")).unwrap();
    fs::remove_file(home.path().join("sessions/future.jsonl")).unwrap();
    let bounded = run(&["--since", "2026-08-01", "--through", "2026-08-31"]);
    let bounded_report: Value = serde_json::from_slice(&bounded.stdout).unwrap();
    assert_eq!(bounded_report["tree"]["rollout_count"], 4);
    assert_eq!(bounded_report["tree"]["input_tokens"], 60);
    assert!(!stderr(&bounded).contains("rollout scan could not read"));
}

#[test]
fn date_options_reject_exact_threads_and_unbounded_empty_rows() {
    let home = fixture_home();
    let exact = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "root", "--since", "2026-08-01", "--codex-home"])
        .arg(home.path())
        .output()
        .unwrap();
    assert!(!exact.status.success());
    assert!(stderr(&exact).contains("only available for project or --all reports"));

    let empty = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args([
            "report",
            "--all",
            "--since",
            "2026-08-01",
            "--group-by",
            "day",
            "--include-empty",
            "--codex-home",
        ])
        .arg(home.path())
        .output()
        .unwrap();
    assert!(!empty.status.success());
    assert!(stderr(&empty).contains("requires both --since and --through"));

    let mixed = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "--all", "--project", "--codex-home"])
        .arg(home.path())
        .output()
        .unwrap();
    assert!(!mixed.status.success());
    assert!(stderr(&mixed).contains("cannot be used with"));
}

#[test]
fn filtered_reports_exclude_untimestamped_data_without_changing_lifetime_totals() {
    let home = TempDir::new().unwrap();
    write_jsonl(
        &home.path().join("sessions/root.jsonl"),
        &[
            json!({"type": "session_meta", "payload": {"id": "root", "source": "cli", "cwd": "/tmp/project"}}),
            json!({"type": "event_msg", "payload": {"type": "thread_settings_applied", "thread_settings": {"service_tier": "standard"}}}),
            json!({"type": "turn_context", "payload": {"model": "gpt-5.6-terra", "effort": "high"}}),
            json!({"type": "event_msg", "payload": {"type": "token_count", "info": {"last_token_usage": {"input_tokens": 42, "total_tokens": 42}}}}),
        ],
    );

    let lifetime = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "--all", "--json", "--codex-home"])
        .arg(home.path())
        .output()
        .unwrap();
    assert!(lifetime.status.success(), "{}", stderr(&lifetime));
    let lifetime: Value = serde_json::from_slice(&lifetime.stdout).unwrap();
    assert_eq!(lifetime["tree"]["input_tokens"], 42);

    let filtered = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args([
            "report",
            "--all",
            "--since",
            "2026-08-01",
            "--through",
            "2026-08-01",
            "--json",
            "--codex-home",
        ])
        .arg(home.path())
        .output()
        .unwrap();
    assert!(filtered.status.success(), "{}", stderr(&filtered));
    let filtered: Value = serde_json::from_slice(&filtered.stdout).unwrap();
    assert_eq!(filtered["tree"]["input_tokens"], 0);
    assert_eq!(filtered["tree"]["incomplete_input"], true);
    assert!(
        filtered["incomplete_input_warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("usable timestamp"))
    );
}

#[test]
fn forced_progress_keeps_json_stdout_machine_readable() {
    let home = fixture_home();
    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "root", "--json", "--progress", "--codex-home"])
        .arg(home.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(serde_json::from_slice::<Value>(&output.stdout).is_ok());
    let stderr = stderr(&output);
    assert!(stderr.contains("Indexing rollout metadata"));
    assert!(stderr.contains("Analyzing 1/1 rollouts"));
    assert_eq!(stderr.matches("Analyzing 1/1 rollouts").count(), 1);
    assert!(!stderr.contains('\r'));
}

#[test]
fn project_path_report_keeps_assigned_and_cli_roots_separate_from_exclusions() {
    let home = TempDir::new().unwrap();
    let workspace = home.path().join("workspace");
    fs::create_dir_all(workspace.join("cli")).unwrap();
    write_jsonl(
        &home.path().join("sessions/assigned.jsonl"),
        &[json!({
            "type": "session_meta",
            "payload": {"id": "assigned", "source": "cli", "cwd": "/worktree"},
        })],
    );
    write_jsonl(
        &home.path().join("sessions/cli.jsonl"),
        &[json!({
            "type": "session_meta",
            "payload": {"id": "cli", "source": "cli", "cwd": workspace.join("cli")},
        })],
    );
    write_jsonl(
        &home.path().join("sessions/projectless.jsonl"),
        &[json!({
            "type": "session_meta",
            "payload": {"id": "projectless", "source": "cli", "cwd": workspace},
        })],
    );
    write_jsonl(
        &home.path().join("sessions/other.jsonl"),
        &[json!({
            "type": "session_meta",
            "payload": {"id": "other", "source": "cli", "cwd": workspace},
        })],
    );
    fs::write(
        home.path().join(".codex-global-state.json"),
        json!({
            "local-projects": {
                "project": {"id": "project", "name": "Project", "rootPaths": [workspace]},
                "other": {"id": "other", "name": "Other", "rootPaths": ["/other"]},
            },
            "thread-project-assignments": {
                "assigned": {"projectId": "project", "projectKind": "local"},
                "other": {"projectId": "other", "projectKind": "local"},
            },
            "projectless-thread-ids": ["projectless"],
        })
        .to_string(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "--project"])
        .arg(&workspace)
        .args(["--json", "--codex-home"])
        .arg(home.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["selection"]["resolver"], "source_root_path");
    assert_eq!(report["selection"]["direct_assignments"], 1);
    assert_eq!(report["selection"]["workspace_fallbacks"], 1);
    assert_eq!(report["selection"]["projectless_exclusions"], 1);
    assert_eq!(report["selection"]["other_project_exclusions"], 1);
    assert_eq!(report["tree"]["rollout_count"], 2);
}

#[test]
fn project_flag_without_a_ref_derives_an_assigned_threads_desktop_project() {
    let home = TempDir::new().unwrap();
    let workspace = home.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    for (id, cwd) in [
        ("assigned", "/worktree"),
        ("cli", workspace.to_str().unwrap()),
    ] {
        write_jsonl(
            &home.path().join(format!("sessions/{id}.jsonl")),
            &[json!({
                "type": "session_meta",
                "payload": {"id": id, "source": "cli", "cwd": cwd},
            })],
        );
    }
    fs::write(
        home.path().join(".codex-global-state.json"),
        json!({
            "local-projects": {
                "project": {"id": "project", "name": "Project", "rootPaths": [workspace]},
            },
            "thread-project-assignments": {
                "assigned": {"projectId": "project", "projectKind": "local"},
            },
            "projectless-thread-ids": [],
        })
        .to_string(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "assigned", "--project", "--json", "--codex-home"])
        .arg(home.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["selection"]["resolver"], "thread_assignment");
    assert_eq!(report["selection"]["direct_assignments"], 1);
    assert_eq!(report["selection"]["workspace_fallbacks"], 1);
    assert_eq!(report["tree"]["rollout_count"], 2);
}

#[test]
fn project_flag_without_a_ref_rolls_up_a_projectless_threads_exact_cwd() {
    let home = TempDir::new().unwrap();
    let workspace = home.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    for id in ["projectless", "cli", "assigned"] {
        write_jsonl(
            &home.path().join(format!("sessions/{id}.jsonl")),
            &[json!({
                "type": "session_meta",
                "payload": {"id": id, "source": "cli", "cwd": workspace},
            })],
        );
    }
    fs::write(
        home.path().join(".codex-global-state.json"),
        json!({
            "local-projects": {
                "other": {"id": "other", "name": "Other", "rootPaths": ["/other"]},
            },
            "thread-project-assignments": {
                "assigned": {"projectId": "other", "projectKind": "local"},
            },
            "projectless-thread-ids": ["projectless"],
        })
        .to_string(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args([
            "report",
            "projectless",
            "--project",
            "--json",
            "--codex-home",
        ])
        .arg(home.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["selection"]["resolver"], "thread_cwd");
    assert_eq!(report["selection"]["projectless_threads"], 1);
    assert_eq!(report["selection"]["workspace_fallbacks"], 1);
    assert_eq!(report["selection"]["other_project_exclusions"], 1);
    assert_eq!(report["tree"]["rollout_count"], 2);
}

#[test]
fn project_name_selects_all_of_its_source_roots() {
    let home = TempDir::new().unwrap();
    let first_root = home.path().join("first");
    let second_root = home.path().join("second");
    fs::create_dir_all(&first_root).unwrap();
    fs::create_dir_all(&second_root).unwrap();
    for (id, cwd) in [("first", &first_root), ("second", &second_root)] {
        write_jsonl(
            &home.path().join(format!("sessions/{id}.jsonl")),
            &[json!({
                "type": "session_meta",
                "payload": {"id": id, "source": "cli", "cwd": cwd},
            })],
        );
    }
    fs::write(
        home.path().join(".codex-global-state.json"),
        json!({
            "local-projects": {
                "project": {"id": "project", "name": "Project", "rootPaths": [first_root, second_root]},
            },
            "thread-project-assignments": {},
            "projectless-thread-ids": [],
        })
        .to_string(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args([
            "report",
            "--project",
            "Project",
            "--json",
            "--progress",
            "--codex-home",
        ])
        .arg(home.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["selection"]["resolver"], "project_name");
    assert_eq!(report["selection"]["workspace_fallbacks"], 2);
    assert_eq!(report["tree"]["rollout_count"], 2);
    assert!(stderr(&output).contains("Analyzing 2/2 rollouts"));
}

#[test]
fn named_project_keeps_direct_assignments_when_a_source_root_is_missing() {
    let home = TempDir::new().unwrap();
    let missing_root = home.path().join("missing");
    write_jsonl(
        &home.path().join("sessions/worktree.jsonl"),
        &[json!({
            "type": "session_meta",
            "payload": {"id": "worktree", "source": "cli", "cwd": "/outside"},
        })],
    );
    fs::write(
        home.path().join(".codex-global-state.json"),
        json!({
            "local-projects": {
                "project": {"id": "project", "name": "Project", "rootPaths": [missing_root]},
            },
            "thread-project-assignments": {
                "worktree": {"projectId": "project", "projectKind": "local"},
            },
            "projectless-thread-ids": [],
        })
        .to_string(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "--project", "Project", "--json", "--codex-home"])
        .arg(home.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["selection"]["missing_source_roots"], 1);
    assert_eq!(report["selection"]["direct_assignments"], 1);
    assert!(
        report["incomplete_input_warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("source root"))
    );
}

#[test]
fn ambiguous_source_root_lists_the_matching_projects() {
    let home = TempDir::new().unwrap();
    let workspace = home.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        home.path().join(".codex-global-state.json"),
        json!({
            "local-projects": {
                "first": {"id": "first", "name": "First", "rootPaths": [workspace]},
                "second": {"id": "second", "name": "Second", "rootPaths": [workspace]},
            },
            "thread-project-assignments": {},
            "projectless-thread-ids": [],
        })
        .to_string(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "--project"])
        .arg(&workspace)
        .args(["--codex-home"])
        .arg(home.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("matches: First, Second"));
}

#[test]
fn explicit_project_rejects_a_thread_owned_by_another_project() {
    let home = TempDir::new().unwrap();
    let first_root = home.path().join("first");
    let second_root = home.path().join("second");
    fs::create_dir_all(&first_root).unwrap();
    fs::create_dir_all(&second_root).unwrap();
    write_jsonl(
        &home.path().join("sessions/other.jsonl"),
        &[json!({
            "type": "session_meta",
            "payload": {"id": "other", "source": "cli", "cwd": second_root},
        })],
    );
    fs::write(
        home.path().join(".codex-global-state.json"),
        json!({
            "local-projects": {
                "first": {"id": "first", "name": "First", "rootPaths": [first_root]},
                "second": {"id": "second", "name": "Second", "rootPaths": [second_root]},
            },
            "thread-project-assignments": {
                "other": {"projectId": "second", "projectKind": "local"},
            },
            "projectless-thread-ids": [],
        })
        .to_string(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "other", "--project", "First", "--codex-home"])
        .arg(home.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("does not belong"));
}

#[test]
fn bare_report_defaults_to_the_current_working_directory() {
    let home = TempDir::new().unwrap();
    let workspace = home.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    write_jsonl(
        &home.path().join("sessions/root.jsonl"),
        &[json!({
            "type": "session_meta",
            "payload": {"id": "root", "source": "cli", "cwd": workspace},
        })],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "--json", "--codex-home"])
        .arg(home.path())
        .current_dir(&workspace)
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["selection"]["resolver"], "current_directory");
    assert_eq!(report["selection"]["workspace_fallbacks"], 1);
    assert_eq!(report["tree"]["rollout_count"], 1);
}

#[test]
fn unique_fuzzy_project_name_resolves_before_historical_cwds() {
    let home = TempDir::new().unwrap();
    let workspace = home.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    write_jsonl(
        &home.path().join("sessions/root.jsonl"),
        &[json!({
            "type": "session_meta",
            "payload": {"id": "root", "source": "cli", "cwd": workspace},
        })],
    );
    fs::write(
        home.path().join(".codex-global-state.json"),
        json!({
            "local-projects": {
                "project": {"id": "project", "name": "Project", "rootPaths": [workspace]},
            },
            "thread-project-assignments": {},
            "projectless-thread-ids": [],
        })
        .to_string(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "--project", "Projec", "--json", "--codex-home"])
        .arg(home.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["selection"]["resolver"], "fuzzy_project_name");
    assert_eq!(report["tree"]["rollout_count"], 1);
}

#[test]
fn unique_historical_cwd_match_is_used_when_no_project_name_or_path_matches() {
    let home = TempDir::new().unwrap();
    write_jsonl(
        &home.path().join("sessions/root.jsonl"),
        &[json!({
            "type": "session_meta",
            "payload": {"id": "root", "source": "cli", "cwd": "/gone/workspace"},
        })],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args([
            "report",
            "--project",
            "/gone/workspace",
            "--json",
            "--codex-home",
        ])
        .arg(home.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["selection"]["resolver"], "fuzzy_historical_cwd");
    assert_eq!(report["tree"]["rollout_count"], 1);
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn assert_cache_created(output: &Output, home: &Path) {
    assert_eq!(
        stderr(output),
        format!(
            "created rollout cache at {}\n",
            home.join("codex-cost-meter.sqlite").display()
        )
    );
}

#[test]
fn help_and_version_describe_the_public_cli_contract() {
    let binary = env!("CARGO_BIN_EXE_codex-cost-meter");
    let root_help = Command::new(binary).arg("--help").output().unwrap();
    let report_help = Command::new(binary)
        .args(["report", "--help"])
        .output()
        .unwrap();
    let update_help = Command::new(binary)
        .args(["update", "--help"])
        .output()
        .unwrap();
    let version = Command::new(binary).arg("--version").output().unwrap();

    assert!(root_help.status.success());
    assert!(root_help.stderr.is_empty());
    let root_help = String::from_utf8(root_help.stdout).unwrap();
    assert!(root_help.contains("report"));
    assert!(root_help.contains("update"));

    assert!(update_help.status.success());
    assert!(update_help.stderr.is_empty());
    let update_help = String::from_utf8(update_help.stdout).unwrap();
    assert!(update_help.contains("Select a task by ID"));
    assert!(update_help.contains("Apply the proposed title updates"));
    assert!(update_help.contains("--refresh"));

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let schedule_help = Command::new(binary)
            .args(["schedule", "--help"])
            .output()
            .unwrap();
        assert!(schedule_help.status.success());
        assert!(schedule_help.stderr.is_empty());
        let schedule_help = String::from_utf8(schedule_help.stdout).unwrap();
        assert!(schedule_help.contains("Manage scheduled idle task-title updates"));
        assert!(
            schedule_help
                .lines()
                .all(|line| !line.trim_start().starts_with("run"))
        );

        let install_help = Command::new(binary)
            .args(["schedule", "install", "--help"])
            .output()
            .unwrap();
        assert!(install_help.status.success());
        assert!(install_help.stderr.is_empty());
        let install_help = String::from_utf8(install_help.stdout).unwrap();
        assert!(install_help.contains("Minimum idle time"));
        assert!(install_help.contains("Use this Codex storage directory"));
    }

    assert!(report_help.status.success());
    assert!(report_help.stderr.is_empty());
    let report_help = String::from_utf8(report_help.stdout).unwrap();
    assert!(report_help.contains("Codex session ID"));
    assert!(report_help.contains("--project"));
    assert!(report_help.contains("--progress"));
    assert!(report_help.contains("--refresh"));
    assert!(report_help.contains("--json"));

    assert!(version.status.success());
    assert!(version.stderr.is_empty());
    assert!(
        String::from_utf8(version.stdout)
            .unwrap()
            .contains(env!("CARGO_PKG_VERSION"))
    );
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
    assert_cache_created(&output, home.path());
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
    assert_cache_created(&output, home.path());
}

#[cfg(not(windows))]
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
    assert_cache_created(&output, &root.path().join(".codex"));
}

#[cfg(windows)]
#[test]
fn report_uses_userprofile_as_a_codex_home_fallback() {
    let root = TempDir::new().unwrap();
    fixture_home_at(&root.path().join(".codex"));
    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "root", "--json"])
        .env_remove("CODEX_HOME")
        .env_remove("HOME")
        .env("USERPROFILE", root.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["rollout"]["rollout_id"],
        "root"
    );
    assert_cache_created(&output, &root.path().join(".codex"));
}

#[cfg(windows)]
#[test]
fn report_uses_complete_home_drive_and_path_as_a_codex_home_fallback() {
    let root = TempDir::new().unwrap();
    let cache_home = root.path().join(".codex");
    fixture_home_at(&cache_home);
    let root = root.path().to_str().unwrap();
    let Some((drive, path)) = root
        .get(..2)
        .zip(root.get(2..))
        .filter(|(drive, path)| drive.ends_with(':') && path.starts_with('\\'))
    else {
        return;
    };

    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "root", "--json"])
        .env_remove("CODEX_HOME")
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .env("HOMEDRIVE", drive)
        .env("HOMEPATH", path)
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["rollout"]["rollout_id"],
        "root"
    );
    assert_cache_created(&output, &cache_home);
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
fn parser_errors_sanitize_terminal_control_thread_ids() {
    let home = fixture_home();
    let output = report_with_home(home.path(), "unknown\u{1b}[31m\r\nforged", true);
    let message = stderr(&output);

    assert_eq!(output.status.code(), Some(2));
    assert!(message.contains("control character"));
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
    assert_cache_created(&output, home.path());
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
    assert_cache_created(&output, home.path());
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

    assert!(output.status.success());
    assert_cache_created(&output, home.path());
    let text = String::from_utf8(output.stdout).unwrap();
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
    assert_cache_created(&output, home.path());
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
