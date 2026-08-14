# ADR 0001: Read task metadata from SQLite and usage from rollouts

- Status: Accepted
- Date: 2026-08-06

## Context

No public Codex API exposes the complete combination needed by the title-cost tool: sidebar identity and naming state, active and archived token history, descendant relationships, and model attribution.

The local stores serve different purposes:

- `state_5.sqlite` contains the task rows used for update selection and persisted-name mutation, including ID, title, name, history mode, first user message, and `updated_at`.
- Active and archived rollout JSONL contain session metadata, parent relationships, token events, model changes, and event timestamps needed to calculate cost.
- `session_index.jsonl` contains the optional latest display name needed by an exact-ID read-only report.
- UI or terminal visibility is not reliable evidence of persisted usage or identity.

## Decision

For title updates, read task selection and persisted naming metadata from `state_5.sqlite`. Read usage, model, timing, root/descendant identity, and active/archive coverage from rollout JSONL.

An exact-ID read-only report does not need SQLite selection, history-mode naming, or update state. It reads identity and usage from rollout JSONL and may read the latest display name from `session_index.jsonl`. This narrower path is an exception for reporting, not a second source of truth for title mutation.

Build one rollout index per run across both `sessions` and `archived_sessions`. If the same rollout ID appears more than once, use the newest file by modification time. Derive root eligibility from rollout metadata rather than treating every SQLite task row as a sidebar root.

Convert cumulative token counters into deltas and attribute each delta to the model and timestamp of its event. Include linked descendants in the root total. For legacy data, recover usage only when one session-metadata record makes attribution unambiguous; otherwise expose the total as incomplete.

## Alternatives considered

- Read SQLite alone: rejected because it does not contain the event-level token and descendant data needed for pricing.
- Read rollouts alone for title updates: rejected because persisted task naming, history mode, and update state come from SQLite.
- Require SQLite for an exact-ID report: rejected because it adds a failure dependency without supplying data required by that report.
- Infer state from visible UI or terminal output: rejected because missing tool output was previously mistaken for zero usage.
- Guess ambiguous legacy attribution: rejected because an incomplete estimate is safer than double-counting.

## Consequences

- The implementation depends on internal, version-sensitive Codex storage formats.
- Rollout discovery can be expensive, so the index is shared by every task processed in one run.
- Cost reports include active and archived roots and their linked descendants.
- Unknown or ambiguous portions remain visible through the incomplete marker.
