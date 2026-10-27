use std::process::{Command, Output};

use serde_json::Value;

fn rates(args: &[&str]) -> Output {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("must-stay-absent");
    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .arg("rates")
        .args(args)
        .env("CODEX_HOME", &home)
        .output()
        .unwrap();
    assert!(
        !home.exists(),
        "rate listing must not initialize session storage"
    );
    output
}

fn json(args: &[&str]) -> Value {
    let mut args = args.to_vec();
    args.push("--json");
    let output = rates(&args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn rates_show_exact_components_context_and_tiers_without_session_storage() {
    let table = json(&["--model", "gpt-6.1-terra"]);
    assert_eq!(table["units"], "USD per 1M tokens");
    assert_eq!(table["cache_write_lifetime"], "30m");
    assert_eq!(table["effective_on"], Value::Null);
    assert!(
        table["source"]
            .as_str()
            .unwrap()
            .contains("https://developers.openai.com/")
    );
    let rows = table["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    for (row, tier, min, max, components) in [
        (
            &rows[0],
            "standard",
            0,
            Some(272_000),
            [1.0, 0.05, 1.25, 5.0],
        ),
        (&rows[1], "standard", 272_001, None, [2.0, 0.1, 2.5, 7.5]),
        (&rows[2], "fast", 0, Some(272_000), [2.0, 0.1, 2.5, 10.0]),
        (&rows[3], "fast", 272_001, None, [4.0, 0.2, 5.0, 15.0]),
    ] {
        assert_eq!(row["model"], "gpt-6.1-terra");
        assert_eq!(row["priced_as"], "gpt-6.1-terra");
        assert_eq!(row["effective_from"], "2026-10-26");
        assert_eq!(row["tier"], tier);
        assert_eq!(row["min_input_tokens"], min);
        assert_eq!(row["max_input_tokens"], serde_json::json!(max));
        for (field, expected) in ["input", "cached_input", "cache_write_input", "output"]
            .into_iter()
            .zip(components)
        {
            assert_eq!(row[field].as_f64().unwrap(), expected);
        }
    }
    let human = rates(&["--model", "gpt-6.1-terra"]);
    assert!(human.status.success());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("0.05") && human.contains(">=272001") && human.contains("Write (30m)"));
}

#[test]
fn historical_selection_preserves_proxy_boundaries_and_price_changes() {
    let old = json(&["--model", "gpt-5.6-sol", "--as-of", "2026-08-20"]);
    let new = json(&["--model", "gpt-5.6-sol", "--as-of", "2026-08-21"]);
    assert_eq!(old["rows"][0]["input"], 5.0);
    assert_eq!(new["rows"][0]["input"], 4.0);
    assert_eq!(new["effective_on"], "2026-08-21");
    for (date, target, rate) in [
        ("2026-10-05", "gpt-5.6-luna", 0.2),
        ("2026-10-06", "chatgpt-auto-review-free", 0.0),
    ] {
        let table = json(&["--model", "codex-auto-review", "--as-of", date]);
        assert_eq!(table["rows"][0]["model"], "codex-auto-review");
        assert_eq!(table["rows"][0]["priced_as"], target);
        assert_eq!(table["rows"][0]["input"], rate);
    }
    let table = json(&["--as-of", "2026-10-25"]);
    assert!(
        table["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["model"] != "gpt-6.1-terra")
    );
    assert!(
        json(&["--as-of", "2020-01-01"])["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn unknown_cells_stay_unknown_and_latest_listing_is_sorted() {
    let table = json(&[]);
    let rows = table["rows"].as_array().unwrap();
    assert!(
        rows.windows(2)
            .all(|pair| pair[0]["model"].as_str() <= pair[1]["model"].as_str())
    );
    assert!(rows.iter().any(|row| row["model"] == "gpt-6.1-terra"));
    let table = json(&["--model", "gpt-5.4"]);
    let long_fast = table["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["tier"] == "fast" && row["min_input_tokens"] == 272_001)
        .unwrap();
    for field in ["input", "cached_input", "cache_write_input", "output"] {
        assert!(long_fast[field].is_null());
    }
    let human = rates(&["--model", "gpt-5.4"]);
    assert!(String::from_utf8(human.stdout).unwrap().contains("unknown"));
}

#[test]
fn invalid_dates_unknown_models_and_prelaunch_filters_fail_clearly() {
    for args in [
        vec!["--as-of", "2026-02-30"],
        vec!["--as-of", "yesterday"],
        vec!["--model", "missing-model"],
        vec!["--model", "gpt-6.1-terra", "--as-of", "2026-10-25"],
    ] {
        let result = rates(&args);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!result.stderr.is_empty());
    }
}
