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

## Privacy and troubleshooting

Reports read local Codex JSONL files. Human and JSON reports, warnings, and runtime errors can include thread names, thread IDs, and local project paths, so review all output before sharing. Human output and runtime errors sanitize control characters and whitespace to prevent terminal or line injection; sanitization does not remove private values. JSON stays structured for programmatic use.

- **`rollout not found`** — confirm the exact ID and the selected Codex home; active and archived sessions are scanned.
- **Partial cost or warnings** — retain the result as incomplete. Unknown prices, ambiguous history, malformed or oversized JSONL, and unreadable nonselected inputs are intentionally not guessed.
- **macOS blocks execution** — recheck the archive checksum, then follow your organization’s Gatekeeper policy. The v0.1 ad-hoc signature does not establish publisher identity.
