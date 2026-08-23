## Context

See `proposal.md` for motivation and `specs/tier-aware-pricing/spec.md` for the behavior contract.

Rollout analysis currently emits per-request usage with model and timestamp, and report aggregation calls the effective-dated catalog for each event. The embedded catalog has one flat Standard rate set per model/date. `thread_settings_applied` events provide an ordered applied service-tier snapshot, but current `token_count` records do not contain the tier returned by the API. Cached analysis serializes usage events under analysis version 1.

The implementation-day API sources are the OpenAI [pricing page](https://developers.openai.com/api/docs/pricing), [Fast-mode guide](https://developers.openai.com/api/docs/guides/fast-mode), and [Fast-mode rate table](https://openai.com/api-fast-mode/), captured 2026-08-22. The estimator uses the published per-model Fast premiums for explicit markers beginning at stable `0.144.0`, when durable rollout evidence first becomes possible.

## Goals / Non-Goals

**Goals:**

- Preserve per-event historical selection and known-lower-bound behavior across tier and context dimensions.
- Keep estimates useful when tier metadata is missing by applying a documented Standard fallback and counting post-release assumptions separately.
- Invalidate cached analysis deterministically when usage events gain tier attribution.

**Non-Goals:**

- Inferring the API-served tier, billing actual subscription charges, or compensating for Fast-to-Standard downgrade.
- Runtime pricing fetches, pricing overrides, new CLI controls, regional/Batch/Flex pricing, or automatic title rewrites.
- Speculative parsing of a future served-tier field that Codex does not currently persist.

## Decisions

### Carry one applied tier on every normalized usage event

Add a small serializable tier value with `Standard`, `AssumedStandard`, `Fast`, and `Unpriced(value)` states. The second rollout-analysis pass updates its current tier whenever it sees `thread_settings_applied`. Each subsequent `token_count` event copies an explicit tier or resolves missing metadata using the public-release boundary.

This follows the persisted chronology and supports multiple changes within a rollout without changing model attribution. Associating a tier only with a turn context was rejected because settings can change between model requests in one turn. Retaining the previous tier when a full settings snapshot omits the field was rejected because it hides the assumption; `AssumedStandard` prices the request while keeping that assumption visible.

### Use public Fast capability and actual usage time as fallback signals

The Codex source history shows commit `2f5b01abd605dfa1304b3b8a12b0033ddf020c75` adding the `/fast` toggle on March 2, 2026. Public prerelease `0.108.0-alpha.2`, published at 2026-03-03T05:35:04Z (March 2 PST), was the first distributed package containing that explicitly enabled, gated capability. Stable `0.111.0`, published at 2026-03-05T19:12:13Z, was the first ordinary-user release with Fast enabled by default. Stable `0.144.0`, published on July 9, was the first stable release that persisted applied-tier snapshots.

For missing tier metadata, either a Fast-capable canonical creator version or a usage timestamp at or after the first public prerelease makes the event `AssumedStandard`. An event is definitive Standard only when its timestamp predates that release and its creator version does not prove Fast capability. This covers older rollouts resumed by newer clients without pretending that `session_meta.cli_version` identifies the writer of every appended line. Missing or malformed signals do not create a new unpriced state; unless the available evidence proves the event predates public Fast capability, the fallback remains assumed Standard. Explicit recorded tiers always win.

### Version a complete price grid per catalog point

Each model keeps its existing ordered history. A history point contains short-context Standard rates plus optional Fast rates and optional long-context Standard/Fast rates. The model entry carries an optional gross-input threshold. When one cell changes, the next history point repeats unchanged cells; the small embedded catalog makes that duplication clearer than multiple independently joined histories.

Explicit rates are used instead of global multipliers because published premiums and supported context combinations vary by model. A global multiplier was rejected as smaller code with a larger correctness surface.

### Treat the threshold as a model constraint, not a dated discount

When a model has a published threshold, every event above it selects the long-context cell. If no effective long-context rate exists for that event date/tier, the event is incomplete; it never falls back to short-context rates. This prevents historical usage from being silently underpriced while allowing source-backed rate cells to begin on conservative dates.

Fast price cells are available beginning at the exact stable `0.144.0` publication timestamp, 2026-07-09T16:47:12Z. That boundary is deliberately tied to observable rollout evidence rather than the earlier service launch: before it, the estimator cannot receive a durable marker to justify the premium. At and after it, explicit Priority/Fast markers use the corresponding model's embedded premium; missing attribution uses the Standard rate effective on the event date.

### Keep estimates useful and isolate real gaps

Add `assumed_standard_tokens` and `unpriced_service_tiers` to aggregate report JSON. Missing tier attribution uses Standard rates and contributes to the assumption counter only after the public-release boundary. Unsupported explicit tiers contribute zero known cost and remain under `unpriced_service_tiers`. A recognized Standard/Fast tier whose model or token component lacks a rate continues through the catalog's existing partial-cost path and remains represented under `unpriced_models`.

This preserves a useful best-effort estimate while making the exact assumed token volume reviewable. Treating every missing snapshot as unpriced was rejected because Codex's default is Standard and blanket incompleteness defeats the estimator's purpose.

### Report applied-tier provenance

Replace the Standard-only pricing basis with a concise applied-tier API-list statement that says the served tier is unavailable. Do not add a served-tier precedence branch or test until a concrete persisted field exists.

### Keep model totals and add per-tier usage detail

Retain the existing aggregate model row, including its model-attributed turn count and duration, and add nested Standard, assumed Standard, Fast, and unavailable-tier usage/cost detail. Human output renders those details directly beneath the model, labels assumptions `Standard (assumed)`, and prefixes Fast with `⚡`; JSON uses stable tier keys such as `standard`, `assumed_standard`, `fast`, and the preserved unavailable value.

Turns and durations are not split across tier details because applied tier is persisted on request-adjacent settings snapshots and may change between usage events within one turn. Duplicating or heuristically apportioning them would make tier rows sum incorrectly. Replacing the existing model row with composite model-tier rows was rejected for the same reason and because it would discard the honest model-level timing total.

### Invalidate analysis cache entries

Increment the rollout analysis cache version. Existing rows remain harmless and are recomputed lazily; no schema migration or compatibility shim is needed.

## Risks / Trade-offs

- [Some post-release rollouts or early events lack tier snapshots] → Price them at the default Standard rate, show their token total as assumed, and let explicit settings override the fallback.
- [Applied Fast can be served and billed as Standard] → State the limitation in pricing provenance and avoid an authoritative-cost label.
- [Historical Fast premiums are estimated from the captured per-model table] → Apply them only where a durable explicit marker is possible, beginning with stable `0.144.0`, and retain the source capture date in report provenance.
- [The embedded catalog becomes more verbose] → Prefer repeated explicit rates over multiplier logic or a runtime pricing dependency.

## Migration Plan

1. Ship the parser, cache-version, catalog, report, and documentation changes together so no tier-aware event is priced by the old catalog.
2. Let reports lazily repopulate analysis-version-4 cache rows.
3. Keep existing titles unchanged; users can invoke the established bounded repricing option after reviewing the new estimates.
4. Rollback requires only the previous binary; it will miss newer cache-version rows and rebuild the older analysis format.
