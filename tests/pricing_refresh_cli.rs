use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(args)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .output()
        .unwrap()
}
#[test]
fn explicit_modes_reset_invalid_data_and_keep_json_clean() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".codex-cost-meter/model-prices.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"invalid catalog").unwrap();
    let bare = run(dir.path(), &["pricing", "reset"]);
    assert_eq!(bare.status.code(), Some(1));
    assert_eq!(fs::read(&path).unwrap(), b"invalid catalog");
    let dry = run(dir.path(), &["pricing", "reset", "--dry-run", "--json"]);
    assert!(dry.status.success());
    assert!(dry.stderr.is_empty());
    let result: Value = serde_json::from_slice(&dry.stdout).unwrap();
    assert_eq!(result["activation"], "dry_run");
    assert!(path.exists());
    let apply = run(dir.path(), &["pricing", "reset", "--apply", "--json"]);
    assert!(apply.status.success());
    assert!(apply.stderr.is_empty());
    assert!(!path.exists());
    let again = run(dir.path(), &["pricing", "reset", "--apply", "--json"]);
    assert_eq!(
        serde_json::from_slice::<Value>(&again.stdout).unwrap()["activation"],
        "unchanged"
    );
    let conflict = run(dir.path(), &["pricing", "refresh", "--apply", "--dry-run"]);
    assert_eq!(conflict.status.code(), Some(2));
}
#[test]
fn malformed_import_and_unverified_evidence_never_activate() {
    let home = tempfile::tempdir().unwrap();
    let proposal = home.path().join("proposal.json");
    fs::write(&proposal, b"{}").unwrap();
    let malformed = run(
        home.path(),
        &[
            "pricing",
            "refresh",
            "--from",
            proposal.to_str().unwrap(),
            "--apply",
            "--json",
        ],
    );
    assert_eq!(malformed.status.code(), Some(1));
    let key = json!({"model":"new-model","tier":"standard","context":"short","effective_from":"2026-11-01"});
    fs::write(&proposal,json!({"proposal_schema_version":1,"catalog_schema_version":1,"operations":[{"id":"new","group":"new","kind":"add","target":{"type":"point","key":key},"expected":{"type":"absent"},"replacement":{"type":"point","key":key,"rates":{"input":1,"output":2}},"evidence_ids":["fake"],"date_basis":"researched launch date"}],"evidence":[{"id":"fake","url":"https://example.invalid/pricing","retrieved_at":"2026-11-06T00:00:00Z","excerpt":"new-model input $1 output $2 per million tokens effective 2026-11-01"}],"coverage":[]}).to_string()).unwrap();
    let output = run(
        home.path(),
        &[
            "pricing",
            "refresh",
            "--from",
            proposal.to_str().unwrap(),
            "--apply",
            "--json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["activation"], "unchanged");
    assert_eq!(result["coverage_complete"], false);
    assert!(
        !home
            .path()
            .join(".codex-cost-meter/model-prices.json")
            .exists()
    );
    let prompt = run(home.path(), &["pricing", "prompt"]);
    assert!(prompt.status.success());
    assert!(!prompt.stdout.is_empty());
}
