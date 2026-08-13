# Python prototype developer guide

## Architecture

The prototype has two scripts:

1. `rollout_stats.py` indexes active and archived rollout JSONL, links descendants, converts cumulative token counters into deltas, attributes usage to a model and event timestamp, and estimates known cost.
2. `update_thread_cost_titles.py` selects root tasks, formats a bounded title, updates `state_5.sqlite`, and appends the matching name record to `session_index.jsonl`.

The operating system owns scheduling. The application is a run-once process, not a daemon.

## Compatibility contract

- Search both `sessions` and `archived_sessions`; when duplicate rollout IDs exist, use the newest file.
- Include linked descendants and classify root, ordinary subagent, review, security-review, compaction, and memory rollouts.
- Price token deltas at the token event timestamp, using the newest rate effective no later than that event.
- Recover unattributed usage only for a legacy rollout with exactly one session metadata record. Multi-metadata embedded history remains incomplete rather than being double-counted.
- Keep unknown models visible and mark the total incomplete instead of inventing a price proxy. Only the explicit prototype proxies are established behavior.
- Select root tasks only. A stored first prompt is a preview, not automatically a safe sidebar name.
- Limit the complete persisted title, including suffix, to 65 characters.
- Update both SQLite and the append-only session index. A rerun may repair the accepted crash gap between those writes.
- Keep dry-run as the default and require an explicit apply operation.

The canonical cross-implementation decisions are recorded in the repository's [ADR index](../docs/adr/README.md).

## Lessons from implementation

The prototype evolved through several private Codex tasks covering rollout accounting, title persistence, repair work, and fleet portability. Their identifiers are intentionally omitted from the public repository.

- Historical pricing was an explicit requirement. Date-aware machinery without researched effective dates was insufficient; later partial tables also inferred unsupported gaps. The corrected table uses dated evidence and records changes only when supported.
- Missing terminal/tool output is not evidence that the sidebar or database is empty. Persisted rollout and storage artifacts are the source of truth for diagnosis.
- Legacy rollouts can contain token usage without modern start events. Handle the unambiguous single-metadata case conservatively and expose the rest as incomplete.
- Raw first prompts can be enormous. Codex distinguishes previews from user-facing names, so promoting `threads.title` corrupted hidden non-root titles and produced unbounded output. Root filtering and whole-title bounds are load-bearing invariants.
- Apply batch limits after eligibility filtering, or already-priced recent tasks starve the backlog. Build the rollout index once per run, not once per task.
- A seven-day selection window was not required and excluded archived roots. Selection covers all active and archived roots once they meet the idle condition.
- The SQLite-to-index write is intentionally not crash-atomic. A failure can cause recomputation, and a rerun repairs the state; cross-store transaction machinery was not justified.
- One-off repair flags and sidecar checkpoints were rejected as permanent operational cruft. Existing root selection and fixed `--reprice-before` timestamps provide repair and resumability.
- Embedded self-tests capture parser, pricing, selection, recovery, and title-bound contracts. They are developer checks, not user ceremony.

## Verification

From the repository root, run:

```sh
python3 python-prototype/rollout_stats.py --self-test
python3 python-prototype/update_thread_cost_titles.py --self-test
python3 -m py_compile python-prototype/rollout_stats.py python-prototype/update_thread_cost_titles.py
```

The Rust port should preserve the behavioral fixtures represented by these checks without copying the Python module structure.
