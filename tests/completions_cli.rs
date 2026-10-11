use std::process::Command;

#[test]
fn completions_generate_for_supported_shells_without_codex_storage() {
    let home = tempfile::TempDir::new().unwrap();
    let missing_home = home.path().join("missing");
    for shell in ["bash", "zsh"] {
        let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
            .args(["completions", shell])
            .env("CODEX_HOME", &missing_home)
            .env_remove("HOME")
            .env_remove("USERPROFILE")
            .output()
            .unwrap();

        assert!(output.status.success(), "{shell}: {:?}", output.stderr);
        assert!(output.stderr.is_empty());
        let script = String::from_utf8(output.stdout).unwrap();
        assert!(script.contains("codex-cost-meter"));
        assert!(script.contains("--thread-id"));
        assert!(script.contains("--json"));
        assert!(!missing_home.exists());
    }
}

#[test]
fn completions_reject_missing_or_unsupported_shell() {
    for args in [vec!["completions"], vec!["completions", "fish"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[cfg(unix)]
#[test]
fn bash_completion_handles_subcommand_options_for_hyphenated_binary() {
    let directory = tempfile::TempDir::new().unwrap();
    let script_path = directory.path().join("completion.bash");
    let output = Command::new(env!("CARGO_BIN_EXE_codex-cost-meter"))
        .args(["completions", "bash"])
        .output()
        .unwrap();
    assert!(output.status.success());
    std::fs::write(&script_path, output.stdout).unwrap();

    let output = Command::new("bash")
        .args([
            "--noprofile",
            "--norc",
            "-c",
            r#"
set -e
source "$1"
spec=$(complete -p codex-cost-meter)
function_name=${spec#* -F }
function_name=${function_name%% *}
COMP_WORDS=(codex-cost-meter report --j)
COMP_CWORD=2
"$function_name" codex-cost-meter --j report
[[ " ${COMPREPLY[*]} " == *" --json "* ]]
COMP_WORDS=(codex-cost-meter update --thr)
"$function_name" codex-cost-meter --thr update
[[ " ${COMPREPLY[*]} " == *" --thread-id "* ]]
COMP_WORDS=(codex-cost-meter schedule install --idle)
COMP_CWORD=3
"$function_name" codex-cost-meter --idle install
[[ " ${COMPREPLY[*]} " == *" --idle-minutes "* ]]
"#,
            "completion-test",
        ])
        .arg(script_path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(output.stderr.is_empty());
}
