# ADR 0003: Use session-index updated_at as the high-water mark

- Status: Accepted
- Date: 2026-08-06

## Context

Periodic runs need to skip roots whose current cost title is already based on the latest known task state. Repricing after a historical price-table correction must also be resumable across bounded batches.

A separate checkpoint database or sidecar file would duplicate state already recorded by each successful append to `session_index.jsonl`.

## Decision

Use the latest valid session-index record for each task as its high-water mark.

For normal updates, skip a root when its latest index name has a valid, bounded cost suffix and its index `updated_at` is at least the SQLite task `updated_at`. A successful title append records the current time and advances that root's high-water mark.

For repricing, accept one fixed `--reprice-before` timestamp for the entire campaign. A root remains eligible while its latest index timestamp precedes that cutoff. Each successful append moves it past the fixed cutoff, allowing later bounded runs to resume without a sidecar checkpoint.

Filter for eligibility before applying the batch limit so already-processed recent roots cannot starve older work. Do not impose an unrelated age window; active and archived roots are both eligible once they satisfy the configured idle period.

## Alternatives considered

- Track checkpoints in a sidecar file or database: rejected as duplicate state and permanent operational machinery.
- Use a moving repricing cutoff: rejected because the target would shift between batches and could never converge predictably.
- Apply the limit before eligibility filtering: rejected because it repeatedly selected already-priced rows and starved the backlog.
- Restrict selection to a seven-day window: rejected because the requirement covers old and archived roots too.

## Consequences

- The session index is both a Codex naming store and the updater's progress ledger.
- A fixed timestamp must be reused for every batch in one repricing campaign.
- A descendant update that never advances the root task timestamp can be delayed; the accepted prototype does not add a second watermark system for that edge case.
- Malformed index lines are ignored, and the latest valid record remains authoritative.
