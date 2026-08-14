use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use rusqlite::{Connection, params};
use serde_json::json;
use tempfile::TempDir;

struct Fixture {
    home: TempDir,
    database: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let home = TempDir::new().unwrap();
        let database = home.path().join("state_5.sqlite");
        let connection = Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE threads (
            id TEXT PRIMARY KEY, title TEXT, name TEXT, history_mode TEXT,
            updated_at INTEGER, first_user_message TEXT
        )",
            )
            .unwrap();
        for (id, title, name) in [
            ("root", "Stored root", "Root task"),
            ("child", "Stored child", "Child task"),
        ] {
            connection
                .execute(
                    "INSERT INTO threads VALUES (?1, ?2, ?3, 'paginated', 0, 'Prompt')",
                    params![id, title, name],
                )
                .unwrap();
        }
        rollout(home.path(), "root", None);
        rollout(home.path(), "child", Some("root"));
        fs::write(home.path().join("session_index.jsonl"), format!("{}\n{}\n",
            json!({"id":"root", "thread_name":"Root task", "updated_at":"2026-08-13T12:00:00Z"}),
            json!({"id":"child", "thread_name":"Child task", "updated_at":"2026-08-13T12:00:00Z"}),
        )).unwrap();
        Self { home, database }
    }

    fn command(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
            .arg("update")
            .args(arguments)
            .arg("--codex-home")
            .arg(self.home.path())
            .env_remove("CODEX_HOME")
            .output()
            .unwrap()
    }

    fn index(&self) -> PathBuf {
        self.home.path().join("session_index.jsonl")
    }

    fn names(&self, id: &str) -> (String, String) {
        Connection::open(&self.database)
            .unwrap()
            .query_row(
                "SELECT title, name FROM threads WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    }
}

fn rollout(home: &Path, id: &str, parent: Option<&str>) {
    let path = home.join("sessions").join(format!("{id}.jsonl"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut payload = json!({"id":id, "source":"cli", "cwd":"/project"});
    if let Some(parent) = parent {
        payload["parent_thread_id"] = json!(parent);
        payload["source"] = json!({"subagent":{"thread_spawn":{"parent_thread_id":parent}}});
    }
    fs::write(path, format!("{}\n{}\n{}\n",
        json!({"type":"session_meta", "timestamp":"2026-08-13T12:00:00Z", "payload":payload}),
        json!({"type":"turn_context", "payload":{"model":"gpt-5.6-terra", "effort":"high"}}),
        json!({"type":"event_msg", "timestamp":"2026-08-13T12:00:00Z", "payload":{"type":"token_count", "info":{"last_token_usage":{"input_tokens":100, "total_tokens":100}}}}),
    )).unwrap();
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn assert_concise_failure(output: &Output) {
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(stderr(output).lines().count(), 1);
}

#[test]
fn dry_run_prints_one_safe_proposal_without_mutation() {
    let fixture = Fixture::new();
    let database_before = fs::read(&fixture.database).unwrap();
    let index_before = fs::read(fixture.index()).unwrap();
    let output = fixture.command(&["--thread-id", "root"]);
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
    assert_eq!(stdout.lines().count(), 2);
    assert!(stdout.starts_with("root: Root task -> "));
    assert_eq!(
        stdout.lines().last(),
        Some("dry run; pass --apply to write")
    );
    assert_eq!(fs::read(&fixture.database).unwrap(), database_before);
    assert_eq!(fs::read(fixture.index()).unwrap(), index_before);
    let proposal = stdout.lines().next().unwrap();
    assert!(
        proposal.find(" · $").unwrap() < proposal.find(" · ⇄").unwrap(),
        "default metrics must retain cost,total-tokens order: {proposal}"
    );
}

#[test]
fn apply_updates_only_the_root_in_both_stores() {
    let fixture = Fixture::new();
    let output = fixture.command(&["--thread-id", "root", "--apply"]);
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    let root = fixture.names("root");

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout.lines().last(), Some("updated 1 task(s)"));
    assert_eq!(root.0, root.1);
    assert!(root.0.contains(" · $"));
    assert_eq!(
        fixture.names("child"),
        ("Stored child".into(), "Child task".into())
    );
    assert!(
        fs::read_to_string(fixture.index())
            .unwrap()
            .lines()
            .last()
            .unwrap()
            .contains("\"id\":\"root\"")
    );
}

#[test]
fn child_schema_and_lock_fail_without_writes() {
    let fixture = Fixture::new();
    let database_before = fs::read(&fixture.database).unwrap();
    let index_before = fs::read(fixture.index()).unwrap();
    let child = fixture.command(&["--thread-id", "child", "--apply"]);
    assert_concise_failure(&child);
    assert!(stderr(&child).contains("root thread id not found"));
    assert_eq!(fs::read(&fixture.database).unwrap(), database_before);
    assert_eq!(fs::read(fixture.index()).unwrap(), index_before);

    Connection::open(&fixture.database)
        .unwrap()
        .execute_batch("DROP TABLE threads; CREATE TABLE threads (id TEXT PRIMARY KEY)")
        .unwrap();
    let missing = fixture.command(&["--thread-id", "root"]);
    assert_concise_failure(&missing);
    assert!(stderr(&missing).contains("threads schema is missing required column"));
}

#[test]
fn locked_database_and_unwritable_index_fail_concisely() {
    let fixture = Fixture::new();
    let connection = Connection::open(&fixture.database).unwrap();
    connection.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let locked = fixture.command(&["--thread-id", "root"]);
    assert_concise_failure(&locked);
    assert!(stderr(&locked).contains("could not open or read state database"));
    connection.execute_batch("ROLLBACK").unwrap();

    let index = fixture.index();
    fs::remove_file(&index).unwrap();
    fs::create_dir(&index).unwrap();
    let unwritable = fixture.command(&["--thread-id", "root", "--apply"]);
    assert_concise_failure(&unwritable);
}

#[test]
fn accepts_extra_schema_and_sanitizes_metadata() {
    let fixture = Fixture::new();
    Connection::open(&fixture.database)
        .unwrap()
        .execute_batch(
            "ALTER TABLE threads ADD COLUMN extra TEXT; CREATE TABLE unrelated (value TEXT)",
        )
        .unwrap();
    let accepted = fixture.command(&["--thread-id", "root"]);
    assert!(accepted.status.success(), "{}", stderr(&accepted));

    let unsafe_title = "name\u{1b}[31m\r\nforged";
    Connection::open(&fixture.database)
        .unwrap()
        .execute(
            "UPDATE threads SET title = ?1, name = ?1 WHERE id = 'root'",
            params![unsafe_title],
        )
        .unwrap();
    let output = fixture.command(&["--thread-id", "root"]);
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(!stdout.contains('\u{1b}'));
    assert!(!stdout.contains('\r'));
    assert_eq!(stdout.lines().count(), 2);

    let unsafe_id = fixture.command(&["--thread-id", "child\u{1b}[31m\r\nforged"]);
    assert_concise_failure(&unsafe_id);
    assert!(!stderr(&unsafe_id).contains('\u{1b}'));
    assert!(!stderr(&unsafe_id).contains('\r'));
}
