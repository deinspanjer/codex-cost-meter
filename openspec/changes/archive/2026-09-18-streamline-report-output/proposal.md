## Why

Human reports repeat totals, normal zero states, static pricing methodology, and audit provenance until the result is harder to scan than the cost and usage it is meant to explain. Real reports ranging from one-turn roots to 81-rollout clusters show that a compact, conditional presentation can preserve decision-relevant context and uncertainty while removing substantial clutter.

## What Changes

- Give exact-rollout reports a compact identity header with the task name, short identifier, project, rollout count, and the model/reasoning pair used by the most root turns.
- Present root and all-agent totals with nested token components, explicit turn-time semantics, and conditional incomplete-turn detail; omit an identical all-agent row for isolated rollouts.
- Reduce model tables to useful aggregate rows, omit duplicate totals and wholly redundant single-model sections, and retain tier detail only when it changes interpretation.
- Adapt project, corpus, grouped, non-root, and empty human reports to the same hierarchy without applying root-only metadata to aggregate scopes.
- Replace repeated pricing sources, proxy history, tier history, and static column notes with concise per-report estimate metadata and conditional warnings; move static interpretation guidance to `report --help` while preserving exact structured metadata.
- Clear ordinary interactive progress before the final report while preserving explicitly forced redirected progress.
- Keep machine-readable JSON and all accounting, selection, pricing, and duration calculations unchanged.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-report-presentation`: Define the compact, conditional human layout across exact-rollout, project, corpus, grouped, non-root, empty, incomplete, and partial-cost reports.
- `tier-aware-pricing`: Reduce ordinary human tier labels and provenance while retaining material tier distinctions, assumed-tier warnings, and complete structured detail.
- `model-proxy-pricing`: Preserve exact proxy history in structured output without requiring full proxy history in every human report.

## Impact

Human rendering and report help in `src/output.rs` and `src/cli.rs`, interactive progress completion in `src/progress.rs`, focused presentation tests, terminal casts or equivalent width verification, and user documentation examples are affected. Structured report types and JSON, rollout analysis, cached data, title updates, pricing calculations, dependencies, and future session activity-range reporting are not affected.
