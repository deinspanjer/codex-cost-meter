# Selectable cost metric

Status: planning and review only. The owner selected `--cost-metric api-usd|credits` on 2026-10-30. Implementation is on hold until explicitly resumed by the owner.

## Why

API-list dollars do not express the credit units consumed under Business and Enterprise credit-based billing. Users need a selectable credit estimate without assuming a monetary price per credit or a customer contract.

## What Changes

- Introduce global `--cost-metric api-usd|credits`, with `api-usd` preserving the default behavior.
- Add fractional credit estimates from an independently sourced, effective-dated credit rate card. Label them estimates, not verified credit deductions.
- Apply selection consistently to reports, rate tables, usage exports, title updates, and newly installed schedules.
- Preserve existing USD JSON fields and add a unit-bearing selected-cost object; never put credits into a USD field.
- Retain JSONL schema version 1 for the additive selected-cost fields and document their optional nature; existing USD fields and record meanings remain unchanged.
- Keep incomplete estimates and known minima distinct for each metric.
- Keep unsupported long-context and pre-card history incomplete until applicable credit rules are evidenced.
- Render credit title costs with the strict two-decimal `¤` prefix, and warn before manual updates whose metric differs from the installed schedule; show the installed metric in schedule status.
- Leave quota percentage points for a later evidence-backed feature; do not accept a placeholder option now.

## Capabilities

### New Capabilities

- `cost-metric-selection`: Global selection, credit-unit accounting, structured output compatibility, title formatting, and schedule propagation.

### Modified Capabilities

None. Existing tier-aware API-dollar behavior remains the default; the new capability adds a separate credit valuation over the same normalized usage.

## Impact

`src/cli.rs`, `src/main.rs`, `src/pricing.rs`, a separate embedded credit catalog under `data/`, report aggregation, human rendering, JSON/JSONL exports, title composition and suffix recognition, update selection, and existing native schedule argument generation. Update CLI completions and user/developer documentation. No new runtime network access, billing integration, pricing overrides, or dependency is expected.
