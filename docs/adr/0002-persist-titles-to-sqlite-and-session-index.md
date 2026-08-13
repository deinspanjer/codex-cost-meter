# ADR 0002: Persist titles to SQLite and the session index

- Status: Accepted
- Date: 2026-08-06

## Context

Changing only `state_5.sqlite` or only `session_index.jsonl` did not reliably produce the intended Codex sidebar state. Codex reads naming information from both stores, and the session index is append-only.

The prototype also exposed two unsafe assumptions: non-root internal tasks should never receive sidebar titles, and a SQLite `title` equal to the first user message is a preview rather than a safe user-facing name.

## Decision

For each eligible root task, persist the same bounded cost title to both stores:

1. Update the task's SQLite `title` and `name` in a transaction and commit it.
2. Append a JSONL record containing the task ID, title as `thread_name`, and write timestamp to `session_index.jsonl`.
3. Flush and synchronize the appended index data before reporting success.

Default to dry-run and require explicit `--apply` for mutation. Hold a single-process lock during mutation. If an interrupted writer left a partial final JSONL record, terminate that fragment with a newline before appending a new complete record.

Only root tasks are eligible. Resolve an existing user-facing name according to Codex history-mode behavior; synthesize from prompt text only as an explicitly bounded fallback. Limit the complete title, including the cost suffix, to 65 characters.

## Alternatives considered

- Update one store only: rejected because it does not reliably update all Codex name readers.
- Rewrite the append-only index: rejected because an append preserves existing history and matches Codex's storage behavior.
- Promote every SQLite `title`: rejected because some values are enormous raw prompts or hidden internal-task previews.
- Make both stores transactionally atomic: not available across SQLite and a JSONL file. The accepted failure mode is recoverable recomputation.

## Consequences

- A crash after the SQLite commit and before the index append can temporarily leave the stores out of sync; rerunning repairs the title.
- Storage mutation remains unsupported and must be verified against Codex changes.
- Root filtering and the whole-title bound are compatibility invariants, not presentation preferences.
