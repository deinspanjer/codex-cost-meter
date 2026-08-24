## Context

See `proposal.md` for motivation and `specs/fork-replay-accounting/spec.md` for the behavior contract.

Discovery currently records one parent identifier without distinguishing explicit forks from subagent lineage. Analysis then normalizes each selected rollout independently, and its SQLite cache is keyed only by the rollout file revision. Replay recognition is necessarily parent-dependent, so a replay-adjusted child result cannot safely use that existing cache key.

The measured invariant is narrow: deterministic replay occurred only when the child's canonical metadata used `forked_from_id`, history was legacy mode, the child serialized the copied parent history, and its leading token-usage tuples exactly matched the parent snapshot. Rewritten child timestamps are expected. Partial cases ended one record before a concurrent parent write, so parent events merely preceding the child timestamp do not define the copied snapshot boundary.

## Goals / Non-Goals

**Goals:**

- Make explicit-fork lineage and replay eligibility structural rather than heuristic.
- Remove only proven copied usage while preserving the copied cumulative baseline needed to normalize the first genuine child request.
- Keep cached non-replay analysis unchanged and deterministic.

**Non-Goals:**

- Removing inherited conversation input, estimating prompt-cache hits, or reconciling an API invoice.
- General event deduplication, timing heuristics, suffix matching, or replay inference for ordinary subagent edges.
- Changing Codex rollout persistence or adding user configuration.

## Decisions

### Preserve lineage provenance during discovery

Extend discovery metadata with the minimum information needed to distinguish an explicit fork from other parent relationships and to identify legacy history. A missing history-mode field uses Codex's observed legacy default. Require the child's copied-history structure, including the embedded source-session metadata, before declaring it replay-eligible.

This avoids treating every parent-linked rollout as a context copy. Reusing only `parent_id` was rejected because V1 fresh agents and V2 `fork_turns=none` can have lineage without copied history. Increment the discovery cache version so older cached metadata cannot omit the new provenance.

Codex's state database does not persist `forked_from_id`, so cold targeted discovery reconciles canonical metadata for state-indexed paths before retaining the requested roots and descendants. The discovery cache narrows subsequent runs without losing explicit forks that have no spawn-edge row.

### Build a prefix plan before analyzing an eligible child

For each eligible child with an available parent, read the parent's valid token-usage tuples through the child's fork timestamp and the child's leading valid token-usage tuples. Compare the complete persisted components: input, cached input, cache-write input, output, reasoning output, and total. Ignore event timestamps because Codex rewrites them while materializing the child rollout.

The plan is simply the number of consecutive matching child usage records. Zero matches means no suppression. After the first mismatch, do not search for another match. This directly represents the measured copied prefix and handles the observed concurrent-parent race without guessing a later boundary.

Using a one-second burst, cross-session signatures, or a percentage similarity threshold was rejected because none identifies whether an API request occurred. Searching parent suffixes was rejected because the audited corpus provides no such invariant and paginated history already avoids the observed replay shape.

### Skip matched accounting while retaining its cumulative baseline

Pass the matched-record count into rollout analysis. For each matched token record, validate it and advance `previous_total`, but do not attribute it, emit a usage event, or add it to unattributed totals. Normal processing resumes at the first unmatched token record.

Advancing the cumulative baseline makes the next cumulative delta equal the genuine child request. The existing `last_token_usage` fallback remains available when cumulative totals are absent or reset. Removing the copied records before reading their totals was rejected because it would force avoidable fallback behavior and could obscure malformed data.

### Bypass the per-file analysis cache only for matched children

Replay eligibility and prefix length depend on both parent and child content, while the existing cache key contains only the child revision. Reuse the cache normally when the prefix length is zero. Analyze a matched child directly and do not store the adjusted result in the per-file cache.

Adding parent revision and replay policy to the cache schema was rejected as unnecessary complexity for the measured minority of rollouts. The replay check is rerun on each report, so a changed parent cannot leave a stale adjusted child result.

### Keep deterministic correction silent in report semantics

Do not add an approximation marker or a new user option. Proven copied accounting records are not usage, so excluding them corrects the existing totals. Unmatched, unavailable-parent, paginated, and otherwise ineligible cases retain all usage and the existing warning/incompleteness behavior.

An approximation marker was rejected because the accepted path has a structural invariant; timing-only candidates are neither removed nor labeled as replay.

## Risks / Trade-offs

- [An explicit fork with an unavailable or unreadable parent remains overcounted] → Retain all child usage rather than risk an opaque undercount, and preserve existing discovery/read warnings.
- [A parent continues writing near the fork] → Stop at the first mismatch; never require every parent record before the child timestamp to appear in the snapshot.
- [Replay detection adds file reads] → Restrict it to structurally eligible explicit legacy forks and continue caching ordinary analysis.
- [Cold targeted discovery must inspect state-indexed metadata to recover fork lineage] → Retain only the requested workset after reconciliation and use the discovery cache to narrow warm runs.
- [Future Codex history modes serialize a different replay shape] → Treat them as unmatched until new source and corpus evidence establishes another invariant.
- [A malformed matched record prevents exact comparison] → End suppression and let existing validation and incompleteness handling process the record.

## Migration Plan

1. Ship discovery provenance, prefix planning, adjusted analysis, and documentation together.
2. Let the discovery-version bump lazily refresh cached rollout metadata; retain existing analysis cache rows for ordinary rollouts.
3. Validate new totals against a read-only aggregate replay audit before any bounded title repricing.
4. Rollback to the previous binary restores the prior accounting behavior without a data migration; cache-version mismatches are rebuilt lazily.
