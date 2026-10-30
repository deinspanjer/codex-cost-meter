# Selectable cost metrics

Status: planning and review only. On 2026-10-30 the owner extended the original dollars/credits proposal to include configurable subscription percentage-point estimates, combinations in reports, and explicit title placeholders. Implementation remains on hold until explicitly resumed by the owner.

## Why

API-list dollars, credit units, and subscription allowance consumption answer different questions. Users need to compare any combination, with subscription percentage-point estimates explicitly grounded in editable, dated calibration rather than a universal token-to-quota conversion.

## What Changes

- Extend global `--cost-metric` to an ordered comma-separated selection of `api-usd`, `credits`, and `quota-pp` (or `all`), with `api-usd` preserving the default behavior.
- Add fractional credit estimates from an independently sourced, effective-dated credit rate card. Label them estimates, not verified credit deductions.
- Add estimated subscription percentage points calculated from normalized tokens using an external, commented TOML configuration: subscription plan/variant, effective periods, allowance window, token weights by model, speed, context bands, and workload policy. No automatic plan detection or inferred plan scaling.
- Ship an installable, explicitly named owner-derived Pro 20× weekly preset. Embed the research method, exact coefficients, weights, validation results, and cloud-coverage correction in this change's design so implementation does not require another rollout scan.
- Apply combinations consistently to reports, rate tables and usage exports. Titles support explicit `{api-usd}`, `{credits}`, and `{quota-pp}` placeholders, including combinations, while preserving the existing `cost` slot.
- Preserve existing USD JSON fields and add ordered metric selection plus separate unit-bearing cost objects; never put credits or percentage points into a USD field.
- Retain JSONL schema version 1 for additive cost fields and document their optional nature; existing USD fields and record meanings remain unchanged.
- Keep incomplete estimates and known minima distinct for each metric.
- Keep unsupported long-context and pre-card history incomplete until applicable credit rules are evidenced.
- Render credits with the strict two-decimal `¤` prefix and quota with a distinct approximate, window-labeled form such as `~1.23pp/wk`. Keep weekly and five-hour denominators separate; the preset has no five-hour calibration.
- Persist title selection, format, allowance window, and absolute configuration path in installed schedules; warn about conflicting manual/scheduled settings and show installed settings in status.

## Capabilities

### New Capabilities

- `cost-metric-selection`: Composable metric selection, independent credit and subscription estimation, editable dated subscription configuration, structured output compatibility, title placeholders, and schedule propagation.

### Modified Capabilities

None. Existing tier-aware API-dollar behavior remains the default; this capability adds independent valuations over the same normalized usage.

## Impact

`src/cli.rs`, `src/main.rs`, valuation, a separate embedded credit catalog and installable quota preset under `data/`, report aggregation, human rendering, JSON/JSONL exports, title composition and suffix recognition, update selection, and native schedule arguments. Add a small TOML parser dependency if needed; retain one Rust crate. Update completions and user/developer documentation. No runtime network access, billing integration, automatic recalibration, or API/credit price overrides. This request changes planning artifacts only; it does not install the preset or alter titles/schedules.
