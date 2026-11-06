use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tempfile::TempDir;

fn run(user: &Path, codex: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"));
    command.args(args);
    if args.first() != Some(&"rates") {
        command.arg("--codex-home").arg(codex);
    }
    command
        .env("HOME", user)
        .env("USERPROFILE", user)
        .env("CODEX_HOME", codex)
        .output()
        .unwrap()
}
fn write_catalog(user: &Path, catalog: Value) {
    let dir = user.join(".codex-cost-meter");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("model-prices.json"), catalog.to_string()).unwrap();
}
fn rollout(home: &Path) {
    let dir = home.join("sessions");
    fs::create_dir_all(&dir).unwrap();
    let rows = [
        json!({"type":"session_meta","timestamp":"2026-10-08T12:00:00Z","payload":{"id":"root","source":"cli"}}),
        json!({"type":"event_msg","payload":{"type":"thread_settings_applied","thread_settings":{"service_tier":"standard"}}}),
        json!({"type":"turn_context","payload":{"model":"gpt-6.1-sol"}}),
        json!({"type":"event_msg","timestamp":"2026-10-08T12:00:01Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":100,"total_tokens":100}}}}),
    ];
    fs::write(
        dir.join("root.jsonl"),
        rows.iter().map(|r| format!("{r}\n")).collect::<String>(),
    )
    .unwrap();
}
fn json_output(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn point() -> Value {
    json!({"model":"gpt-6.1-sol","tier":"standard","context":"short","effective_from":"2026-10-08","rates":{"input":0.01}})
}
#[test]
fn sparse_and_standalone_pricing_follow_user_home_across_codex_homes() {
    let user = TempDir::new().unwrap();
    let codex1 = TempDir::new().unwrap();
    let codex2 = TempDir::new().unwrap();
    rollout(codex1.path());
    rollout(codex2.path());
    write_catalog(
        user.path(),
        json!({"schema_version":1,"version":"penny","label":"My prices","points":[point()]}),
    );
    for codex in [codex1.path(), codex2.path()] {
        let rates = json_output(run(
            user.path(),
            codex,
            &["rates", "--model", "gpt-6.1-sol", "--json"],
        ));
        assert_eq!(rates["metadata"]["mode"], "embedded + overlay");
        assert_eq!(rates["metadata"]["label"], "My prices");
        assert_eq!(rates["rows"][0]["input"], 0.01);
        assert!(rates["rows"][0]["output"].is_null());
        let report = json_output(run(user.path(), codex, &["report", "root", "--json"]));
        assert_eq!(report["pricing"]["metadata"]["version"], "penny");
        assert_eq!(report["tree"]["estimated_cost_usd"], 0.000001);
    }
    write_catalog(
        user.path(),
        json!({"schema_version":1,"version":"penny","label":"My prices","base":"none","points":[point()]}),
    );
    let rates = json_output(run(user.path(), codex1.path(), &["rates", "--json"]));
    assert_eq!(rates["metadata"]["mode"], "standalone");
    assert!(rates["catalog_as_of"].is_null());
    assert!(rates["source"].is_null());
    assert!(
        rates["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["model"] == "gpt-6.1-sol")
    );
    let report = json_output(run(
        user.path(),
        codex2.path(),
        &["report", "root", "--json"],
    ));
    assert_eq!(report["pricing"]["metadata"]["mode"], "standalone");
    assert!(report["pricing"]["as_of"].is_null());
    assert!(report["pricing"]["source"].is_null());
    assert_eq!(report["tree"]["estimated_cost_usd"], 0.000001);
}
#[test]
fn invalid_catalog_stops_update_before_title_mutation() {
    let user = TempDir::new().unwrap();
    let codex = TempDir::new().unwrap();
    let db = codex.path().join("state_5.sqlite");
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection.execute_batch("CREATE TABLE threads (id TEXT PRIMARY KEY, title TEXT, name TEXT, history_mode TEXT, updated_at INTEGER, first_user_message TEXT, source TEXT); INSERT INTO threads VALUES ('root','Original','Original','paginated',0,'Prompt','cli');").unwrap();
    rollout(codex.path());
    let index = codex.path().join("session_index.jsonl");
    fs::write(&index, "{\"id\":\"root\",\"thread_name\":\"Original\"}\n").unwrap();
    let before = fs::read(&index).unwrap();
    write_catalog(user.path(), json!({"schema_version":999,"version":"bad"}));
    let out = run(
        user.path(),
        codex.path(),
        &["update", "--thread-id", "root", "--apply"],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("regenerate"));
    let name: String = connection
        .query_row("SELECT name FROM threads WHERE id='root'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(name, "Original");
    assert_eq!(fs::read(index).unwrap(), before);
}
