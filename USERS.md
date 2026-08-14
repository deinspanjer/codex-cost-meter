# User guide

## Install and run

Download the archive for your platform and its `.sha256` checksum from the release, verify the checksum, and extract the archive. Place the binary in a directory you choose. If you have no preference, use `~/.codex/codex-cost-meter` on macOS or `%USERPROFILE%\.codex\codex-cost-meter.exe` on Windows.

On macOS, verify the download with `shasum -a 256 -c <CHECKSUM_FILE>`. On Windows, compare `(Get-FileHash <ZIP_FILE> -Algorithm SHA256).Hash` with the hash at the start of the downloaded checksum file. The Windows ZIP contains `codex-cost-meter.exe`, `README.md`, and `LICENSE`.

macOS examples:

```text
~/.codex/codex-cost-meter report <THREAD_ID>
~/.codex/codex-cost-meter report <THREAD_ID> --json
~/.codex/codex-cost-meter report <THREAD_ID> --codex-home <PATH>
```

Windows uses the same `report` and `update` arguments with `codex-cost-meter.exe`. For example:

```text
%USERPROFILE%\.codex\codex-cost-meter.exe report <THREAD_ID>
%USERPROFILE%\.codex\codex-cost-meter.exe update --thread-id <THREAD_ID>
```

Use the bare executable name only when you place it in an existing directory on `PATH`; v0.4 does not create or use a `bin` directory under the Codex home. The tool resolves the Codex directory in this order: `--codex-home`, `CODEX_HOME`, then the platform home plus `.codex`. `report` is read-only and reports that exact ID and its linked descendants without SQLite; `update` is dry-run by default and reads the supported local SQLite state only for title-update selection or application (details below).

The macOS archive is ad-hoc signed, not Developer ID signed or notarized. Gatekeeper can therefore require a user decision before first launch. The Windows executable is unsigned, so Windows or organizational controls can also require an explicit trust decision or block it. Verify the downloaded checksum and follow your organization's trust process; the checksum detects transfer corruption or tampering but does not establish publisher identity. Production signing is planned for a later release.

## Read the report

Human output shows root and whole-tree totals, per-model usage, price metadata, and summed agent-turn time. A trailing `+` on a human cost means the displayed amount is only the known partial cost. In JSON, the corresponding complete estimate is `null`; inspect `incomplete_input`, `unpriced_models`, `unattributed_usage_tokens`, and `incomplete_input_warnings` before treating an estimate as complete.

Cost uses the embedded historical catalog and is an API-list-price approximation, not a billing record. Reasoning is included in output usage; cache reads are included in input usage.

## Preview or apply title updates

`update` selects root tasks only. It never changes a title unless `--apply` is present, so begin with a dry run and review every proposed `id: old -> new` line:

```text
~/.codex/codex-cost-meter update --thread-id <THREAD_ID>
~/.codex/codex-cost-meter update --match-title "unique title text"
~/.codex/codex-cost-meter update --idle-minutes 15 --limit 20
~/.codex/codex-cost-meter update --thread-id <THREAD_ID> --apply
```

`--thread-id` and `--match-title` are repeatable and can be combined; a case-insensitive title match must resolve to exactly one root. `--idle-minutes` is an alternative selection mode. It chooses eligible idle roots newest first, after filtering, and accepts `--limit` (default 20), `--max-runtime SECONDS|MINUTESm`, and `--reprice-before` with an ISO date or RFC 3339 timestamp with a timezone. A repricing cutoff cannot be in the future.

Titles default to `--title-metrics cost,total-tokens` and `--max-width 65`. Choose an ordered comma-separated subset of `cost`, `total-tokens`, `input-tokens`, and `output-tokens`, or use `all`; duplicate metrics and a width too small for a visible base and suffix are rejected. The tool removes only its trailing canonical metric suffix before recomposing a title, preserves Unicode-scalar width, and marks incomplete known cost with `+`.

For dry runs, the tool opens Codex's state database read-only. With `--apply`, it locks one updater process, updates both `title` and `name` in a single SQLite transaction, then appends durable JSONL entries to `session_index.jsonl`. It supports `state_5.sqlite` directly under the selected Codex home or under `sqlite/`, and requires the `threads` columns `id`, `title`, `name`, `history_mode`, `updated_at`, and `first_user_message`; extra schema is accepted. SQLite and JSONL are deliberately not cross-store atomic: if index writing fails after a committed transaction, the command fails and a later identical `--apply` remains eligible to repair both stores. A busy updater, database/schema problem, or unreadable index is a safe actionable failure.

## Schedule idle title updates on macOS

Windows scheduling is not included in v0.4; run `report` or `update` directly there. On macOS, `schedule install` creates one current-user LaunchAgent. It runs when loaded and every five minutes, and it applies updates only to eligible idle root tasks. It uses these defaults: `--idle-minutes 15`, `--limit 500`, `--max-runtime 4m`, `--max-width 65`, and `--title-metrics cost,total-tokens`.

```text
~/.codex/codex-cost-meter schedule install
~/.codex/codex-cost-meter schedule install --idle-minutes 30 --limit 100 --max-runtime 2m
~/.codex/codex-cost-meter schedule status
~/.codex/codex-cost-meter schedule resume
~/.codex/codex-cost-meter schedule remove
~/.codex/codex-cost-meter uninstall
```

`schedule install` accepts the displayed defaults as overrides, plus `--reprice-before` and `--codex-home`; the scheduled command always applies updates and does not accept explicit thread or title selection. The job stores the canonical path of the executable. Moving or deleting that binary breaks the installed job; run `schedule remove` before moving it, then install again from its new location.

`schedule status` reports whether the property list is installed, whether launchd reports the job as loaded, and the bounded status record. When a run has occurred, its fields are `last run`, stable `result`, `consecutive failures`, `paused`, and fixed remediation; otherwise it reports `last run: never`. `schedule resume` clears a pause without re-registering the job. `schedule remove` is idempotent and removes only this tool's LaunchAgent and bounded status record. `uninstall` first performs that removal, then deletes only the currently running executable; it does not delete Codex data or a parent directory.

Scheduled runs create no append-only log and are silent on success and ordinary lock contention. They store only fixed result codes and remediation, not task metadata, paths, IDs, titles, prompts, or arbitrary error text. Three consecutive ordinary failures pause the schedule; disk-full, incompatible SQLite schema, and permission-denied failures pause it immediately. Use `schedule status`, correct the local issue, then run `schedule resume`.

## Privacy and troubleshooting

Reports read local Codex JSONL files. Reports, update previews, warnings, and runtime errors can include thread names, thread IDs, old/new titles, and local project paths, so review all output before sharing. Human output and runtime errors sanitize control characters and whitespace to prevent terminal or line injection; sanitization does not remove private values. JSON stays structured for programmatic use.

- **`rollout not found`** — confirm the exact ID and the selected Codex home; active and archived sessions are scanned.
- **Partial cost or warnings** — retain the result as incomplete. Unknown prices, ambiguous history, malformed or oversized JSONL, and unreadable nonselected inputs are intentionally not guessed.
- **`another title updater is already running`** — wait for the other updater to finish, then rerun the dry run or apply command.
- **Database, schema, or session-index error** — do not edit Codex storage by hand. Resolve the local access issue and rerun; a failed index append after commit is intentionally recoverable by rerunning the same selection with `--apply`.
- **Scheduled updates are paused** — use `schedule status` for the fixed remediation, correct the reported storage, schema, or permission problem, then run `schedule resume`.
- **Scheduled job no longer starts** — if the binary was moved or deleted, run `schedule remove` if possible and install again from its new location.
- **macOS blocks execution** — recheck the archive checksum, then follow your organization’s Gatekeeper policy. The ad-hoc signature does not establish publisher identity.
- **Windows blocks execution** — recheck the ZIP checksum, then follow your organization’s application-control or SmartScreen policy. The unsigned executable does not establish publisher identity.
