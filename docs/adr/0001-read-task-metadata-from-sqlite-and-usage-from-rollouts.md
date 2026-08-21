# ADR 0001: Read task metadata from SQLite and usage from rollouts

- Status: Accepted
- Date: 2026-08-06
- Amended: 2026-08-21

## Context

No public Codex API exposes the complete combination needed by the title-cost tool: sidebar identity and naming state, active and archived token history, descendant relationships, and model attribution.

The local stores serve different purposes:

- `state_5.sqlite` contains the task rows used for update selection and persisted-name mutation, including ID, title, name, history mode, first user message, and `updated_at`.
- Active and archived rollout JSONL contain session metadata, parent relationships, token events, model changes, and event timestamps needed to calculate cost.
- `session_index.jsonl` contains the optional latest display name needed by an exact-ID read-only report.
- UI or terminal visibility is not reliable evidence of persisted usage or identity.

## Decision

For title updates, read task selection and persisted naming metadata from `state_5.sqlite`. Read usage, model, timing, root/descendant identity, and active/archive coverage from rollout JSONL.

An exact-ID read-only report does not require SQLite selection, history-mode naming, or update state. SQLite may narrow rollout discovery, but identity and usage still come from rollout JSONL and the latest display name may come from `session_index.jsonl`. If SQLite is unavailable, reporting remains usable through rollout fallback. This narrower path is an exception for reporting, not a second source of truth for title mutation.

For an exact-ID report or bounded title update, use SQLite rollout paths and `thread_spawn_edges` to select the requested roots and ordinary spawned descendants before opening rollout files. Probe the smaller set of SQLite subagent/internal rows without spawn edges because guardian, review, compaction, memory, and compatibility relationships may exist only in rollout metadata. Walk active and archived directories without opening indexed files, then inspect only paths absent from SQLite to recover state-database lag. Fall back to the full rollout scan when SQLite is absent or incompatible, a requested ID is absent, or a selected path cannot be read.

For project reports, select root candidates from Codex's SQLite projection and Desktop Project metadata before building the rollout workset. Inspect JSONL paths missing from the projection so newly persisted rollouts are not omitted, and use a full rollout scan only as compatibility fallback. If the same rollout ID appears more than once, use the newest file by modification time. Derive final root eligibility and descendant accounting from rollout metadata rather than treating every SQLite task row or spawn edge as complete truth.

Store versioned discovery and parsed-analysis facts in the app-owned `$CODEX_HOME/codex-cost-meter.sqlite`, not in Codex's database. Reuse parsed analysis only when the rollout file's modification time and size match the cached revision. Keep pricing and aggregation live, let `--refresh` bypass analysis reuse for the selected workset, and disable the cache for the remainder of a command after one reported cache error.

Convert cumulative token counters into deltas and attribute each delta to the model and timestamp of its event. Include linked descendants in the root total. For legacy data, recover usage only when one session-metadata record makes attribution unambiguous; otherwise expose the total as incomplete.

## Alternatives considered

- Read SQLite alone: rejected because it does not contain the event-level token and descendant data needed for pricing.
- Trust SQLite as a complete rollout index: rejected because Codex persists JSONL first, metadata synchronization is best-effort, and `thread_spawn_edges` represents only ordinary thread spawns.
- Read rollouts alone for title updates: rejected because persisted task naming, history mode, and update state come from SQLite.
- Require SQLite for an exact-ID report: rejected because it adds a failure dependency without supplying data required by that report.
- Add tables to Codex's `state_5.sqlite`: rejected because Codex owns and versions that database, its recovery moves the whole database aside, and it exposes no third-party extension contract.
- Trust Codex's projected update timestamp as the cache watermark: rejected because JSONL is flushed first, projection updates are best-effort and can be coalesced, and ordinary projection failures have no guaranteed immediate repair.
- Infer state from visible UI or terminal output: rejected because missing tool output was previously mistaken for zero usage.
- Guess ambiguous legacy attribution: rejected because an incomplete estimate is safer than double-counting.

## Consequences

- The implementation depends on internal, version-sensitive Codex storage formats.
- Exact and bounded operations avoid opening unrelated indexed rollouts; metadata-only child candidates and SQLite-missing files remain a deliberate reconciliation cost.
- Project reports avoid opening unrelated indexed rollouts; compatibility fallback scans remain deliberately available.
- The disposable cache is an additional local file containing derived task metadata and usage facts; it can be removed and rebuilt without affecting Codex data.
- Cost reports include active and archived roots and their linked descendants.
- Unknown or ambiguous portions remain visible through the incomplete marker.
