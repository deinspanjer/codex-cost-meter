use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use rusqlite::Connection;
use serde_json::{Value, json};
use tempfile::TempDir;

fn run(home: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(arguments)
        .current_dir(home)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("CODEX_HOME", home)
        .output()
        .unwrap()
}

fn parsed(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn policy() -> String {
    let preset = include_str!("../data/quota-pro-20x.toml");
    let first = preset.replace("0.08236824622767631", "1000.0").replace(
        "effective_from = \"2026-10-16T00:00:00Z\"",
        "effective_from = \"2026-10-16T00:00:00Z\"\neffective_until = \"2026-11-01T00:00:00Z\"",
    );
    let second = preset
        .split_once("[[periods]]")
        .unwrap()
        .1
        .replace("owner-pro-20x-2026-10", "second-plan")
        .replace(
            "subscription_plan = \"pro\"",
            "subscription_plan = \"custom\"",
        )
        .replace(
            "effective_from = \"2026-10-16T00:00:00Z\"",
            "effective_from = \"2026-11-01T00:00:00Z\"",
        )
        .replace("0.08236824622767631", "2000.0");
    format!("{first}\n[[periods]]{second}")
}

fn usage(at: &str, input: u64, cached: u64, output: u64) -> Value {
    json!({"type":"event_msg","timestamp":at,"payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":input,"cached_input_tokens":cached,"cache_write_input_tokens":0,"output_tokens":output,"reasoning_output_tokens":output / 2,"total_tokens":input + output}}}})
}

fn write_rollout(home: &Path, id: &str, rows: &[Value]) {
    fs::create_dir_all(home.join("sessions")).unwrap();
    fs::write(
        home.join(format!("sessions/{id}.jsonl")),
        rows.iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>(),
    )
    .unwrap();
}

fn fixture() -> TempDir {
    let home = TempDir::new().unwrap();
    write_rollout(
        home.path(),
        "root",
        &[
            json!({"type":"session_meta","timestamp":"2026-10-31T23:59:00Z","payload":{"id":"root","source":"cli","creator_account_id":"synthetic-account"}}),
            json!({"type":"turn_context","payload":{"model":"gpt-6.1-terra"}}),
            json!({"type":"event_msg","payload":{"type":"thread_settings_applied","thread_settings":{"service_tier":"standard"}}}),
            usage("2026-10-31T23:59:59Z", 200_000, 160_000, 20_000),
            usage("2026-11-01T00:00:00Z", 200_000, 160_000, 20_000),
            json!({"type":"turn_context","payload":{"model":"gpt-6.1-sol"}}),
            json!({"type":"event_msg","payload":{"type":"thread_settings_applied","thread_settings":{"service_tier":"fast"}}}),
            usage("2026-11-01T20:00:00Z", 1_000, 0, 100),
        ],
    );
    write_rollout(
        home.path(),
        "child",
        &[
            json!({"type":"session_meta","timestamp":"2026-11-01T20:00:00Z","payload":{"id":"child","parent_thread_id":"root","source":{"subagent":{"other":"ordinary-review"}},"creator_account_id":"synthetic-account"}}),
            json!({"type":"turn_context","payload":{"model":"gpt-6.1-terra"}}),
            usage("2026-11-01T20:00:00Z", 10_000, 0, 0),
        ],
    );
    let db = Connection::open(home.path().join("state_5.sqlite")).unwrap();
    db.execute_batch("CREATE TABLE threads (id TEXT PRIMARY KEY, title TEXT, name TEXT, history_mode TEXT, updated_at INTEGER, first_user_message TEXT, source TEXT); INSERT INTO threads VALUES ('root','Original','Original','paginated',0,'Prompt','cli');").unwrap();
    fs::write(
        home.path().join("session_index.jsonl"),
        format!(
            "{}\n",
            json!({"id":"root","thread_name":"Original","updated_at":"2026-10-31T23:59:00Z"})
        ),
    )
    .unwrap();
    fs::write(home.path().join("quota.toml"), policy()).unwrap();
    home
}

#[test]
fn shared_usage_reconciles_independent_metrics_across_policy_boundaries() {
    let home = fixture();
    let args = [
        "report",
        "root",
        "--json",
        "--cost-metric",
        "all",
        "--quota-config",
        "quota.toml",
    ];
    let report = parsed(run(home.path(), &args));
    let tree = &report["tree"];
    assert_eq!(
        tree["cost_metrics"],
        json!(["api-usd", "credits", "quota-pp"])
    );
    assert_eq!(tree["input_tokens"], 411_000);
    assert!(tree["estimated_cost_usd"].is_number());
    assert!(tree["costs"]["credits"]["estimated"].is_null());
    assert_eq!(tree["costs"]["quota-pp"]["known_minimum"], 464.0);
    assert!(tree["costs"]["quota-pp"]["estimated"].is_null());
    assert_eq!(
        tree["costs"]["quota-pp"]["unsupported_events"]["quota_speed_unavailable"],
        1
    );
    assert_eq!(
        tree["costs"]["quota-pp"]["period_contributions"]["owner-pro-20x-2026-10"],
        148.0
    );
    assert_eq!(
        tree["costs"]["quota-pp"]["period_contributions"]["second-plan"],
        316.0
    );
    let fingerprint = tree["costs"]["quota-pp"]["provenance"]["config_fingerprint"].clone();

    let export = run(
        home.path(),
        &[
            "report",
            "root",
            "--jsonl",
            "--cost-metric",
            "all",
            "--quota-config",
            "quota.toml",
        ],
    );
    assert!(
        export.status.success(),
        "{}",
        String::from_utf8_lossy(&export.stderr)
    );
    let lines = std::str::from_utf8(&export.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 6);
    assert_eq!(lines[0]["schema_version"], 1);
    assert_eq!(
        lines[0]["costs"]["quota-pp"]["provenance"]["config_fingerprint"],
        fingerprint
    );
    for event in &lines[1..5] {
        assert_eq!(
            event["costs"]["quota-pp"]["provenance"]["config_fingerprint"],
            fingerprint
        );
    }
    assert_eq!(lines[5]["stats"]["costs"], tree["costs"]);
    assert_eq!(
        lines[5]["stats"]["estimated_cost_usd"],
        tree["estimated_cost_usd"]
    );

    let index_before = fs::read(home.path().join("session_index.jsonl")).unwrap();
    let preview = run(
        home.path(),
        &[
            "update",
            "--thread-id",
            "root",
            "--title-format",
            "{title} · {api-usd} · {credits} · {quota-pp}",
            "--quota-config",
            "quota.toml",
            "--max-width",
            "128",
        ],
    );
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    let preview = String::from_utf8(preview.stdout).unwrap();
    assert!(preview.contains('¤') && preview.contains("~464.00pp/wk+") && preview.contains('$'));
    assert_eq!(
        fs::read(home.path().join("session_index.jsonl")).unwrap(),
        index_before
    );
    let narrow = run(
        home.path(),
        &[
            "update",
            "--thread-id",
            "root",
            "--title-format",
            "{title} · {api-usd} · {credits} · {quota-pp}",
            "--quota-config",
            "quota.toml",
            "--max-width",
            "10",
            "--apply",
        ],
    );
    assert!(!narrow.status.success());
    assert_eq!(
        fs::read(home.path().join("session_index.jsonl")).unwrap(),
        index_before
    );

    let grouped = parsed(run(
        home.path(),
        &[
            "report",
            "--all",
            "--json",
            "--group-by",
            "day,model",
            "--cost-metric",
            "all",
            "--quota-config",
            "quota.toml",
        ],
    ));
    assert_eq!(grouped["tree"]["costs"], tree["costs"]);
    let sum: f64 = grouped["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| {
            group["stats"]["costs"]["quota-pp"]["known_minimum"]
                .as_f64()
                .unwrap()
        })
        .sum();
    assert_eq!(sum, 464.0);

    fs::write(
        home.path().join("quota.toml"),
        policy().replace("1000.0", "2000.0"),
    )
    .unwrap();
    let changed = parsed(run(home.path(), &args));
    assert_eq!(changed["tree"]["costs"]["quota-pp"]["known_minimum"], 612.0);
    assert_ne!(
        changed["tree"]["costs"]["quota-pp"]["provenance"]["config_fingerprint"],
        fingerprint
    );
    assert_eq!(
        changed["tree"]["estimated_cost_usd"],
        tree["estimated_cost_usd"]
    );
    assert_eq!(
        changed["tree"]["costs"]["credits"],
        tree["costs"]["credits"]
    );

    let pricing = home.path().join(".codex-cost-meter");
    fs::create_dir_all(&pricing).unwrap();
    fs::write(
        pricing.join("model-prices.json"),
        json!({"schema_version":1,"version":"synthetic","base":"none","points":[]}).to_string(),
    )
    .unwrap();
    let repriced = parsed(run(home.path(), &args));
    assert!(repriced["tree"]["estimated_cost_usd"].is_null());
    assert_eq!(
        repriced["tree"]["costs"]["quota-pp"],
        changed["tree"]["costs"]["quota-pp"]
    );
    assert_eq!(
        repriced["tree"]["costs"]["credits"],
        changed["tree"]["costs"]["credits"]
    );
}

#[test]
fn unused_policy_does_not_break_other_metrics_or_change_usd_fields() {
    let home = fixture();
    fs::write(home.path().join("quota.toml"), "broken policy").unwrap();
    let usd = parsed(run(home.path(), &["report", "root", "--json"]));
    let credits = parsed(run(
        home.path(),
        &[
            "report",
            "root",
            "--json",
            "--cost-metric",
            "credits",
            "--quota-config",
            "quota.toml",
        ],
    ));
    assert_eq!(
        credits["tree"]["estimated_cost_usd"],
        usd["tree"]["estimated_cost_usd"]
    );
    assert_eq!(
        credits["tree"]["known_model_cost_usd"],
        usd["tree"]["known_model_cost_usd"]
    );
    let before = fs::read(home.path().join("session_index.jsonl")).unwrap();
    let output = run(
        home.path(),
        &[
            "update",
            "--thread-id",
            "root",
            "--title-metrics",
            "quota-pp",
            "--quota-config",
            "quota.toml",
            "--apply",
        ],
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        fs::read(home.path().join("session_index.jsonl")).unwrap(),
        before
    );
    let title: String = Connection::open(home.path().join("state_5.sqlite"))
        .unwrap()
        .query_row("SELECT title FROM threads WHERE id='root'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(title, "Original");
}
