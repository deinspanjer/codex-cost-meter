## Why

Legacy Codex forks physically copy historical `TokenCount` events into the child rollout even though those records do not represent new API requests. Corpus evidence shows that the current per-rollout normalization can therefore count billions of duplicated tokens, while the first genuine child request still needs to retain the input usage caused by its inherited conversation context.

## What Changes

- Recognize explicit `forked_from_id` lineage when constructing a rollout tree.
- Detect a deterministic leading child usage prefix copied from a resolvable parent snapshot.
- Exclude only the matched replay prefix from child accounting and retain the first mismatch and all later usage.
- Preserve warnings and incomplete accounting for usage that remains genuinely unattributed.
- Do not add timing-based suppression, session-agnostic event deduplication, or speculative suffix matching.
- Document the measured replay invariant and the distinction between copied accounting events and billable inherited prompt context.

## Capabilities

### New Capabilities

- `fork-replay-accounting`: Account for legacy explicit forks without double-counting physically copied parent usage records or suppressing genuine child request usage.

### Modified Capabilities

None.

## Impact

- Affects rollout lineage discovery, usage analysis, task/project aggregation, report totals, and completeness metadata.
- Changes totals for historical and current legacy explicit forks that contain a proven copied prefix; non-fork, paginated, subagent-only, and unmatched rollouts retain existing behavior.
- Requires no new dependency, CLI option, heuristic configuration, or upstream Codex change.
