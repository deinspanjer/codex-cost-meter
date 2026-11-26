use std::{
    fs,
    path::Path,
    process::{Command, Output},
    time::{Duration, UNIX_EPOCH},
};

use rusqlite::{Connection, params};
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

fn set_modified(path: &Path, seconds: u64) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(seconds))
        .unwrap();
}

/// One rollout with a single 2026-08-10 request of `input` tokens.
fn rollout(id: &str, parent: Option<&str>, cwd: &str, input: u64) -> Vec<Value> {
    let mut metadata = json!({"id": id, "source": "cli", "cwd": cwd});
    if let Some(parent) = parent {
        metadata["parent_thread_id"] = json!(parent);
        metadata["source"] = json!({"subagent": {"other": "worker"}});
    }
    vec![
        json!({"type": "session_meta", "timestamp": "2026-08-10T12:00:00Z", "payload": metadata}),
        json!({"type": "event_msg", "payload": {"type": "thread_settings_applied", "thread_settings": {"service_tier": "standard"}}}),
        json!({"type": "turn_context", "payload": {"turn_id": "turn-1", "model": "gpt-5.6-terra", "effort": "high"}}),
        json!({"type": "event_msg", "timestamp": "2026-08-10T12:00:00Z", "payload": {"type": "task_started", "turn_id": "turn-1"}}),
        json!({
            "type": "event_msg",
            "timestamp": "2026-08-10T12:00:00Z",
            "payload": {
                "type": "token_count",
                "info": {
                    "last_token_usage": {"input_tokens": input, "total_tokens": input},
                    "total_token_usage": {"input_tokens": input, "total_tokens": input},
                },
            },
        }),
    ]
}

fn write_rollout(home: &Path, id: &str, parent: Option<&str>, cwd: &str, input: u64) {
    write_jsonl(
        &home.join(format!("sessions/{id}.jsonl")),
        &rollout(id, parent, cwd, input),
    );
}

fn run(homes: &[&Path], args: &[&str]) -> Output {
    let list = homes
        .iter()
        .map(|home| home.to_str().unwrap())
        .collect::<Vec<_>>()
        .join(",");
    Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .arg("report")
        .args(args)
        .args(["--codex-homes", &list])
        .env("HOME", homes[0])
        .env("USERPROFILE", homes[0])
        .env_remove("CODEX_HOME")
        .env("TZ", "UTC")
        .output()
        .unwrap()
}

fn json_report(homes: &[&Path], args: &[&str]) -> Value {
    let mut args = args.to_vec();
    args.push("--json");
    let output = run(homes, &args);
    assert!(output.status.success(), "{}", stderr(&output));
    serde_json::from_slice(&output.stdout).unwrap()
}

fn export(homes: &[&Path], args: &[&str]) -> Vec<Value> {
    let mut args = args.to_vec();
    args.push("--jsonl");
    let output = run(homes, &args);
    assert!(output.status.success(), "{}", stderr(&output));
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn desktop_state(home: &Path, state: Value) {
    fs::write(home.join(".codex-global-state.json"), state.to_string()).unwrap();
}

#[test]
fn a_copied_rollout_counts_once_in_any_home_order() {
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    for home in [&first, &second] {
        write_rollout(home.path(), "shared", None, "/project", 100);
    }
    write_rollout(second.path(), "only-second", None, "/project", 7);

    let forward = json_report(&[first.path(), second.path()], &["--all"]);
    let reverse = json_report(&[second.path(), first.path()], &["--all"]);

    assert_eq!(forward["tree"]["rollout_count"], 2);
    assert_eq!(forward["tree"]["input_tokens"], 107);
    assert_eq!(forward, reverse);
    let usage = export(&[first.path(), second.path()], &["--all"])
        .into_iter()
        .filter(|row| row["type"] == "usage")
        .count();
    assert_eq!(usage, 2);
}

#[test]
fn an_exact_report_includes_a_child_from_another_home() {
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    write_rollout(first.path(), "root", None, "/project", 100);
    write_rollout(second.path(), "child", Some("root"), "/project", 20);

    let report = json_report(&[first.path(), second.path()], &["root"]);

    assert_eq!(report["tree"]["rollout_count"], 2);
    assert_eq!(report["tree"]["input_tokens"], 120);
}

/// A legacy fork child whose first record copies its parent's only request,
/// followed by one genuine request of 25 input tokens.
fn write_legacy_fork(parent_home: &Path, child_home: &Path) {
    let parent = rollout("parent", None, "/project", 100);
    write_jsonl(&parent_home.join("sessions/parent.jsonl"), &parent);
    let mut child = vec![json!({
        "type": "session_meta",
        "timestamp": "2026-08-10T12:02:00Z",
        "payload": {"id": "child", "forked_from_id": "parent", "source": "cli", "cwd": "/project"},
    })];
    child.extend(parent);
    child.push(json!({
        "type": "event_msg",
        "timestamp": "2026-08-10T12:03:00Z",
        "payload": {
            "type": "token_count",
            "info": {
                "last_token_usage": {"input_tokens": 25, "total_tokens": 25},
                "total_token_usage": {"input_tokens": 125, "total_tokens": 125},
            },
        },
    }));
    write_jsonl(&child_home.join("sessions/child.jsonl"), &child);
}

fn index_threads(home: &Path, ids: &[(&str, &Path)]) {
    let database = Connection::open(home.join("state_5.sqlite")).unwrap();
    database
        .execute_batch(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT, cwd TEXT);
             CREATE TABLE thread_spawn_edges (
                 parent_thread_id TEXT, child_thread_id TEXT PRIMARY KEY
             );",
        )
        .unwrap();
    for (id, path) in ids {
        database
            .execute(
                "INSERT INTO threads VALUES (?1, ?2, '/project')",
                params![id, path.to_string_lossy()],
            )
            .unwrap();
    }
}

#[test]
fn a_fork_child_report_excludes_its_copied_prefix_with_one_or_two_homes() {
    let single = TempDir::new().unwrap();
    write_legacy_fork(single.path(), single.path());
    index_threads(
        single.path(),
        &[
            ("parent", &single.path().join("sessions/parent.jsonl")),
            ("child", &single.path().join("sessions/child.jsonl")),
        ],
    );
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    write_legacy_fork(first.path(), second.path());

    for homes in [vec![single.path()], vec![first.path(), second.path()]] {
        // The second run uses the warm cache and the SQLite-targeted lookup.
        for _ in 0..2 {
            let report = json_report(&homes, &["child"]);
            assert_eq!(report["tree"]["rollout_count"], 1);
            assert_eq!(report["tree"]["input_tokens"], 25);
            let summary = export(&homes, &["child"]).pop().unwrap();
            assert_eq!(summary["stats"]["input_tokens"], 25);
        }
    }
}

#[test]
fn date_pruning_uses_each_homes_own_metadata() {
    let indexed = TempDir::new().unwrap();
    let unindexed = TempDir::new().unwrap();
    // Recorded events fall inside the range; only metadata can exclude them.
    write_rollout(indexed.path(), "stale", None, "/project", 1);
    write_rollout(unindexed.path(), "kept", None, "/project", 2);
    let database = Connection::open(indexed.path().join("state_5.sqlite")).unwrap();
    database
        .execute_batch(
            "CREATE TABLE threads (
                id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL,
                created_at INTEGER, updated_at INTEGER
             );",
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO threads VALUES ('stale', ?1, 0, 0)",
            params![
                indexed
                    .path()
                    .join("sessions/stale.jsonl")
                    .to_string_lossy()
            ],
        )
        .unwrap();
    drop(database);

    let report = json_report(
        &[indexed.path(), unindexed.path()],
        &["--all", "--since", "2026-08-01"],
    );

    assert_eq!(report["tree"]["rollout_count"], 1);
    assert_eq!(report["tree"]["input_tokens"], 2);
}

#[test]
fn a_mistyped_home_fails_before_any_report() {
    let home = TempDir::new().unwrap();
    write_rollout(home.path(), "root", None, "/project", 100);
    let missing = home.path().join("missing");

    let output = run(&[home.path(), &missing], &["--all", "--json"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains(missing.to_str().unwrap()));
}

#[test]
fn a_missing_rollout_error_names_every_home() {
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();

    let output = run(&[first.path(), second.path()], &["absent"]);

    assert!(!output.status.success());
    for home in [&first, &second] {
        assert!(stderr(&output).contains(home.path().to_str().unwrap()));
    }
}

fn name(home: &Path, id: &str, title: &str) {
    write_jsonl(
        &home.join("session_index.jsonl"),
        &[json!({"id": id, "thread_name": title, "updated_at": "2026-08-13T12:00:00Z"})],
    );
}

#[test]
fn title_matching_counts_a_copied_root_once() {
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    for home in [&first, &second] {
        write_rollout(home.path(), "root", None, "/project", 100);
        name(home.path(), "root", "Billing fixes");
    }

    let report = json_report(
        &[first.path(), second.path()],
        &["--match-title", "billing"],
    );

    assert_eq!(report["rollout"]["rollout_id"], "root");
}

#[test]
fn a_fallback_title_does_not_depend_on_home_order() {
    let homes = [
        TempDir::new().unwrap(),
        TempDir::new().unwrap(),
        TempDir::new().unwrap(),
    ];
    for (index, home) in homes.iter().enumerate() {
        write_rollout(home.path(), "root", None, "/project", 100);
        set_modified(
            &home.path().join("sessions/root.jsonl"),
            1_000 + index as u64,
        );
    }
    // The newest copy has no name; the other holders disagree.
    name(homes[0].path(), "root", "First title");
    name(homes[1].path(), "root", "Second title");
    let paths = homes.iter().map(TempDir::path).collect::<Vec<_>>();

    let names = [[0, 1, 2], [1, 0, 2], [2, 1, 0], [2, 0, 1]].map(|order| {
        json_report(&order.map(|index| paths[index]), &["root"])["rollout"]["thread_name"].clone()
    });

    assert!(names[0].is_string());
    assert!(names.iter().all(|name| *name == names[0]), "{names:?}");
}

#[cfg(unix)]
#[test]
fn one_unreadable_home_keeps_other_homes_and_marks_the_report_incomplete() {
    use std::os::unix::fs::PermissionsExt;

    let readable = TempDir::new().unwrap();
    let denied = TempDir::new().unwrap();
    write_rollout(readable.path(), "root", None, "/project", 100);
    let sessions = denied.path().join("sessions");
    fs::create_dir_all(&sessions).unwrap();
    fs::set_permissions(&sessions, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(&sessions).is_ok() {
        fs::set_permissions(&sessions, fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let report = json_report(&[readable.path(), denied.path()], &["--all"]);
    fs::set_permissions(&sessions, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(report["tree"]["input_tokens"], 100);
    assert_eq!(report["tree"]["incomplete_input"], true);
    let warnings = report["incomplete_input_warnings"].to_string();
    assert!(warnings.contains(sessions.to_str().unwrap()), "{warnings}");
}

#[cfg(unix)]
#[test]
fn each_home_keeps_its_own_cache() {
    use std::os::unix::fs::PermissionsExt;

    let writable = TempDir::new().unwrap();
    let locked = TempDir::new().unwrap();
    write_rollout(writable.path(), "first", None, "/project", 1);
    write_rollout(locked.path(), "second", None, "/project", 2);
    fs::set_permissions(locked.path(), fs::Permissions::from_mode(0o555)).unwrap();
    let probe = locked.path().join("probe");
    if fs::write(&probe, "").is_ok() {
        fs::remove_file(probe).unwrap();
        fs::set_permissions(locked.path(), fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let homes = [writable.path(), locked.path()];
    let output = run(&homes, &["--all", "--json"]);
    let rerun_output = run(&homes, &["--all", "--json"]);
    let rerun: Value = serde_json::from_slice(&rerun_output.stdout).unwrap();
    fs::set_permissions(locked.path(), fs::Permissions::from_mode(0o755)).unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let notices = stderr(&output);
    assert!(
        notices.contains(&format!(
            "could not create rollout cache at {}",
            locked.path().display()
        )),
        "{notices}"
    );
    assert!(writable.path().join("codex-cost-meter.sqlite").is_file());
    // The writable home reuses its cache instead of creating it again.
    assert!(!stderr(&rerun_output).contains(&format!(
        "created rollout cache at {}",
        writable.path().display()
    )));
    assert_eq!(rerun["tree"]["input_tokens"], 3);
}

#[test]
fn same_named_projects_in_two_homes_are_one_project() {
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    write_rollout(first.path(), "from-first", None, "/elsewhere", 1);
    write_rollout(second.path(), "from-second", None, "/elsewhere", 2);
    for (home, project, root) in [
        (&first, "first-id", "from-first"),
        (&second, "second-id", "from-second"),
    ] {
        desktop_state(
            home.path(),
            json!({
                "local-projects": {project: {"id": project, "name": "Billing", "rootPaths": ["/missing-root"]}},
                "thread-project-assignments": {root: {"projectId": project}},
                "projectless-thread-ids": [],
            }),
        );
    }

    let report = json_report(&[first.path(), second.path()], &["--project", "billing"]);

    assert_eq!(report["selection"]["resolver"], "project_name");
    assert_eq!(report["selection"]["direct_assignments"], 2);
    assert_eq!(report["tree"]["input_tokens"], 3);
}

#[test]
fn a_copied_root_is_classified_once_by_its_strongest_reason() {
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    let workspace = first.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let workspace = workspace.canonicalize().unwrap();
    for home in [&first, &second] {
        write_rollout(home.path(), "copied", None, workspace.to_str().unwrap(), 10);
    }
    write_rollout(second.path(), "unrelated", None, "/somewhere-else", 5);
    desktop_state(
        first.path(),
        json!({
            "local-projects": {"p": {"id": "p", "name": "Billing", "rootPaths": [workspace]}},
            "thread-project-assignments": {"copied": {"projectId": "p"}},
            "projectless-thread-ids": [],
        }),
    );
    desktop_state(
        second.path(),
        json!({
            "local-projects": {},
            "thread-project-assignments": {},
            "projectless-thread-ids": ["copied"],
        }),
    );

    let report = json_report(&[first.path(), second.path()], &["--project", "Billing"]);
    let selection = &report["selection"];

    assert_eq!(selection["direct_assignments"], 1);
    for counter in [
        "workspace_fallbacks",
        "projectless_threads",
        "projectless_exclusions",
        "other_project_exclusions",
    ] {
        assert_eq!(selection[counter], 0, "{counter}");
    }
    assert_eq!(report["tree"]["input_tokens"], 10);
}

#[test]
fn a_project_in_one_home_includes_unassigned_roots_from_another() {
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    let workspace = first.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let workspace = workspace.canonicalize().unwrap();
    write_rollout(
        second.path(),
        "beneath",
        None,
        workspace.to_str().unwrap(),
        4,
    );
    desktop_state(
        first.path(),
        json!({
            "local-projects": {"p": {"id": "p", "name": "Billing", "rootPaths": [workspace]}},
            "thread-project-assignments": {},
            "projectless-thread-ids": [],
        }),
    );

    let report = json_report(&[first.path(), second.path()], &["--project", "Billing"]);

    assert_eq!(report["selection"]["workspace_fallbacks"], 1);
    assert_eq!(report["tree"]["input_tokens"], 4);
}

#[test]
fn a_task_derived_project_spans_homes_by_name() {
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    write_rollout(first.path(), "task", None, "/elsewhere", 1);
    write_rollout(second.path(), "peer", None, "/elsewhere", 2);
    for (home, project, root) in [(&first, "a", "task"), (&second, "b", "peer")] {
        desktop_state(
            home.path(),
            json!({
                "local-projects": {project: {"id": project, "name": "Billing", "rootPaths": ["/missing-root"]}},
                "thread-project-assignments": {root: {"projectId": project}},
                "projectless-thread-ids": [],
            }),
        );
    }

    let report = json_report(&[first.path(), second.path()], &["task", "--project"]);

    assert_eq!(report["selection"]["resolver"], "thread_assignment");
    assert_eq!(report["tree"]["input_tokens"], 3);
}

#[test]
fn a_task_derived_project_needs_metadata_in_the_tasks_home() {
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    write_rollout(second.path(), "task", None, "/elsewhere", 1);
    desktop_state(
        first.path(),
        json!({"local-projects": {}, "thread-project-assignments": {}, "projectless-thread-ids": []}),
    );

    let output = run(&[first.path(), second.path()], &["task", "--project"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("Desktop Project metadata is unavailable"));
}

#[test]
fn two_same_named_projects_in_one_home_stay_ambiguous() {
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    desktop_state(
        first.path(),
        json!({
            "local-projects": {
                "a": {"id": "a", "name": "Billing", "rootPaths": []},
                "b": {"id": "b", "name": "billing", "rootPaths": []},
            },
            "thread-project-assignments": {},
            "projectless-thread-ids": [],
        }),
    );

    let output = run(&[first.path(), second.path()], &["--project", "Billing"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("ambiguous"), "{}", stderr(&output));
}

#[test]
fn a_subdirectory_of_a_source_root_does_not_select_that_project() {
    let home = TempDir::new().unwrap();
    let root = home.path().join("repo");
    let subdirectory = root.join("service");
    fs::create_dir_all(&subdirectory).unwrap();
    desktop_state(
        home.path(),
        json!({
            "local-projects": {"p": {"id": "p", "name": "Repo", "rootPaths": [root]}},
            "thread-project-assignments": {},
            "projectless-thread-ids": [],
        }),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["report", "--project", ".", "--json", "--codex-home"])
        .arg(home.path())
        .current_dir(&subdirectory)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["selection"]["resolver"], "path");
}
