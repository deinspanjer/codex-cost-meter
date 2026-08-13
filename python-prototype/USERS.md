# Python prototype user guide

The Python implementation is preserved as a reference implementation. It can still inspect local Codex data, but the title updater writes undocumented Codex storage and should be treated as an expert tool.

Run the examples below from the repository root.

## Requirements

- Python 3 on macOS or another Unix-like host.
- Local Codex state under `~/.codex`, or a different directory supplied with `--codex-home`.
- For title updates, a Codex `state_5.sqlite` database and `session_index.jsonl`.

The updater is not Windows-compatible: it uses Unix `fcntl` locking, and its scheduler installer uses macOS `launchctl`.

## Estimate a task

```sh
python3 python-prototype/rollout_stats.py THREAD_ID
```

Add `--json` for machine-readable output or `--codex-home PATH` for another Codex home.

The report searches active and archived rollout files, includes linked descendants, and applies the price known for each model at each token event. A trailing `+` means some cost is unknown because a model was unpriced or token usage could not be attributed safely.

Human and JSON reports contain local task names and may contain the task's project path. Treat redirected or shared reports as private workstation data.

## Preview a title update

The updater is dry-run by default:

```sh
python3 python-prototype/update_thread_cost_titles.py --thread-id THREAD_ID
python3 python-prototype/update_thread_cost_titles.py --match-title "unique title text"
python3 python-prototype/update_thread_cost_titles.py --idle-minutes 15 --limit 20
```

Review the proposed titles, then repeat the command with `--apply` to write them. The script updates both SQLite and `session_index.jsonl`; writing only one does not reliably update the desktop sidebar.

Use a fixed `--reprice-before` timestamp to reprice older titles in resumable batches. The session-index timestamp is the checkpoint, so no sidecar state is required.

## Verify the archived implementation

The tests are embedded in the scripts:

```sh
python3 python-prototype/rollout_stats.py --self-test
python3 python-prototype/update_thread_cost_titles.py --self-test
```

These are developer regression checks, not a required preflight before every dry run.

## LaunchAgent template

`com.openai.codex.thread-cost-titles.plist` is preserved as a deployment template. Replace these placeholders before installing it:

- `__PYTHON_EXECUTABLE__`
- `__SCRIPT_PATH__`
- `__CODEX_HOME__`
- `__LOG_PATH__`

The updater can instead generate a current-user LaunchAgent with `--install-launch-agent`. Installation immediately loads a job that runs every five minutes with `--apply`, updating as many as 500 eligible tasks per run. Its log contains task identifiers and old/new titles. Review the generated plist and protect or rotate its log before enabling it.

Both LaunchAgent mechanisms are macOS-only prototype behavior, not the cross-platform design.

## Interpretation and safety

- Costs use a static historical price table and are API-price equivalents, not invoices.
- Unknown pricing and ambiguous legacy history are reported as incomplete instead of guessed.
- Titles are limited to 65 characters including the cost suffix.
- Only root tasks are eligible, including with explicit `--thread-id` and `--match-title` selection; subagents and review/guardian tasks do not belong in the sidebar.
- SQLite commits before the index append. A crash in that gap can require a rerun, but does not lose the original rollout data.
