# User guide

## Install and run

Download the macOS Universal 2 archive and its `.sha256` checksum from the release, verify the checksum, and extract the archive. Place the binary in a directory you choose. If you have no preference, `~/.codex/codex-cost-meter` is a concrete location:

```text
~/.codex/codex-cost-meter report <THREAD_ID>
~/.codex/codex-cost-meter report <THREAD_ID> --json
~/.codex/codex-cost-meter report <THREAD_ID> --codex-home <PATH>
```

Use bare `codex-cost-meter` only when you place the binary in an existing directory on `PATH`; v0.1 does not create or use `~/.codex/bin`. The tool resolves the Codex directory in this order: `--codex-home`, `CODEX_HOME`, then `~/.codex`. It reports that exact ID and its linked descendants only; v0.1 is read-only and does not require SQLite.

The archive is ad-hoc signed, not Developer ID signed or notarized. Gatekeeper can therefore require a user decision before first launch. Verify the downloaded checksum and use your organization's macOS trust process; production signing is planned for a later release.

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

v0.2 does not install, inspect, or run scheduling. Keep periodic execution outside this release; macOS scheduling remains planned for v0.3.

## Privacy and troubleshooting

Reports read local Codex JSONL files. Reports, update previews, warnings, and runtime errors can include thread names, thread IDs, old/new titles, and local project paths, so review all output before sharing. Human output and runtime errors sanitize control characters and whitespace to prevent terminal or line injection; sanitization does not remove private values. JSON stays structured for programmatic use.

- **`rollout not found`** — confirm the exact ID and the selected Codex home; active and archived sessions are scanned.
- **Partial cost or warnings** — retain the result as incomplete. Unknown prices, ambiguous history, malformed or oversized JSONL, and unreadable nonselected inputs are intentionally not guessed.
- **`another title updater is already running`** — wait for the other updater to finish, then rerun the dry run or apply command.
- **Database, schema, or session-index error** — do not edit Codex storage by hand. Resolve the local access issue and rerun; a failed index append after commit is intentionally recoverable by rerunning the same selection with `--apply`.
- **macOS blocks execution** — recheck the archive checksum, then follow your organization’s Gatekeeper policy. The ad-hoc signature does not establish publisher identity.
