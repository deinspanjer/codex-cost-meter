# Design

## Context

The current Clap root has no global value options. `Catalog::cost` returns an untyped numeric cost, but report and export fields explicitly name USD. Pricing and aggregation are not cached; normalized usage is. Titles use `cost` in `--title-metrics`, format dollars, and recognize dollar suffixes when replacing prior metrics. Schedules persist executable arguments separately on each supported platform.

Official evidence checked 2026-10-30:

- [Codex token credit rates](https://learn.chatgpt.com/docs/pricing#token-rates) publish separate input, cached-input, and output rates per million tokens. They explicitly exclude a separate cache-write charge and distinguish credit billing from API pricing.
- The same source states purchased-credit and Enterprise pay-as-you-go Fast and Ultrafast rates are respectively 2x and 6x Standard where supported. These are the purchased-credit / Enterprise pay-as-you-go multipliers selected for this estimate; included subscription usage instead uses 2.5x Fast and 8x Ultrafast. The latter do not apply to credit valuation.
- Some Enterprise customers retain legacy rate cards; other Enterprise agreements use USD billing. This estimate does not apply to Enterprise agreements on the legacy rate card or USD billing; it targets only the published token-based credit card.

Credits are fractional accounting units. Their purchase price, discounts, commitments, included allowance, and overage contracts do not enter the calculation. The owner's description of negotiated arrangements may apply to particular contracts; it is not a universal plan requirement.

## Goals / Non-Goals

Goals: select one cost metric throughout an invocation; preserve default API valuation; estimate published credit units honestly; keep the architecture small and leave a future metric extensible.

Non-goals: actual invoices, account balances or deductions, quota percentage estimation, contract customization, plan detection, non-token tool/image/voice charges without adequate telemetry, or an exchange rate between dollars and credits. Plan-only work must not implement or install schedules.

## Decisions

### Confirmed CLI

Use `--cost-metric api-usd|credits`, default `api-usd`, declared global so it works before or after subcommands. Example: `codex-cost-meter --cost-metric credits report --project .`; `codex-cost-meter rates --cost-metric credits` is equivalent placement.

The owner confirmed this spelling on 2026-10-30. Do not add aliases or a short flag without a demonstrated need.

Potential future value: `quota-percent`. Its window selection and attribution need a separate design; five-hour and weekly percentages cannot be collapsed into a single undifferentiated number. Do not implement an enum variant, help entry, or stub for it now.

Commands that have no valuation work accept the global flag without changing behavior. `--title-metrics cost` continues to select the cost slot; the global flag chooses its basis. Token and timing metrics remain unchanged.

### Separate rate sources, shared usage accounting

Add a small `CostMetric` enum and a selected valuation path over the existing normalized events. Keep the existing API catalog intact and add an independent embedded credit catalog with source URL, observation date, supported model/tier/context combinations, and evidenced effective dates. Share only lookup/arithmetic that has identical semantics. Do not multiply API dollars by a universal conversion factor or inherit API context surcharges.

Before implementation, capture the applicable full credit rate card and speed/context rules, reconcile every supported model, and establish historical coverage. A current observed rate is not proof of its historical start. Unsupported history remains incomplete rather than being backfilled with today's card.

Historical evidence sources are Wayback snapshots of [learn.chatgpt.com/docs/pricing](https://web.archive.org/web/*/https://learn.chatgpt.com/docs/pricing) and the [Business/Enterprise credit rate card, article 11481834](https://web.archive.org/web/*/https://help.openai.com/en/articles/11481834*). The Help Center card is already named in the API catalog's source list; that listing alone does not establish any historical credit rate. Record the exact snapshot URL, capture timestamp, contents, and any explicit effective date when evidence is collected. These are evidence sources to investigate, not a claim that qualifying snapshots have already been found.

Pre-card history is expected to remain incomplete. Events before the earliest applicable evidenced card, and retired models such as GPT-5.5, GPT-5.4, and the GPT-5.x-codex family without archived rate evidence, produce incomplete `+` estimates rather than invented historical rates. Initial coverage is therefore expected to be most useful for current-model usage; retirement itself does not preclude pricing when an applicable historical card is recovered.

The current card states GPT-5.6 Sol promotional pricing is available **at least through November 21, 2026**. The catalog's effective-date mechanism must represent a later evidenced promotional rate change without rewriting earlier history. November 21 is a minimum promotional period, not proof of a November 22 change; do not invent a future rate or automatic expiry. Missing event timestamps follow the existing latest-known convention and disclose that assumption. Preserve existing model-proxy history only where its applicability to credit valuation is justified and disclosed.

For supported token-only events, use `(noncached_input * input_rate + cached_input * cached_rate + output * output_rate) / 1_000_000`. Input and output retain existing inclusive counter semantics: cached input is a subset of input and reasoning is a subset of output. Cache-write input is charged once as ordinary noncached input, with no additional write surcharge; verify the normalized counter partition before wiring this path. Apply published credit speed rules independently of API pricing, only for supported dated combinations. Assumed Standard stays visible. Carry the existing applied-tier versus served-tier limitation into credit provenance.

Do not copy the API catalog's zero-price auto-review policy into credits without source evidence. Unsupported model, component, special workload, tier, context length, or history retains known credit minima and marks the complete estimate unavailable. Context length is an explicit completeness dimension: for a model with a 272,000 gross-input threshold, an event above that boundary (for example 272,001 input tokens) has no complete credit estimate until a per-model, dated credit long-context rule is evidenced. Do not silently reuse base rates as a complete price or borrow the API surcharge. The published GPT-6.1 Sol subscription-only no-surcharge statement does not establish credit billing rules for that or other models. Retain an independently evidenced lower bound where available; absent evidence that base rates are a lower bound for the long-context event, that event adds zero to the known minimum while other supported events retain their priced contributions. Unknown non-token charges cannot be made complete merely because the token subtotal is computable; provenance defines this as a token-based estimate and identifies any observable excluded charges. Absence of such telemetry is an explicit scope limitation, not a claim of invoice completeness.

### Structured output and precision

Keep `estimated_cost_usd` and `known_model_cost_usd` with their current values and meanings, including in credits mode. Add `cost_metric` and a selected `cost` object wherever existing cost values occur, with `unit` (`usd` or `credits`), `estimated` (nullable), and `known_minimum`. Each selected cost has corresponding metric-specific provenance and completeness diagnostics; USD diagnostics remain identifiable as USD. API-only invocations need not calculate credits. Credit mode may calculate both to preserve the USD fields.

Retain JSONL `schema_version: 1`: selected-cost fields are additive, optional extensions for consumers, and all existing fields and record meanings remain unchanged. Document that readers should tolerate unknown fields; old exports without selection metadata retain their existing API-USD interpretation. Metadata, event records, and terminal summary consistently identify the metric. Ordinary JSON changes are additive. Never rename or reinterpret the existing USD fields. Rate-table output identifies its unit and rate basis; credit rates must not inherit USD cache-write-lifetime prose.

Use the project's existing `f64` calculation style, preserve up to eight decimal places in structured amounts, and round only after aggregation. Human credit amounts always use two decimals and thousands separators; report labels can spell out `credits`. In the title slot use the strict prefix form `¤12.34`, with U+00A4 (generic currency sign), and `<¤0.01` for a positive amount below 0.01. Report values may use the same form or spell out `<0.01 credits` under a credit label. Keep the known-minimum `+` convention. Human dollars retain their current rendering. Do not round credits to whole units or round each event before summing.

### Titles and scheduled updates

Use `¤12.34`, `¤0.50`, and `¤1,234.56` in the cost title slot, with `¤12.34+` for an incomplete known minimum, subject to existing width bounds. The canonical grammar is optional leading `<`, U+00A4 `¤`, a strictly grouped integer, exactly two decimal digits, and optional trailing `+`. Positive amounts below 0.01 render `<¤0.01` (or `<¤0.01+` when incomplete). Keep exact amounts in structured output and round only the human display. Match this grammar as strictly as the dollar `is_cost` recognizer; do not accept arbitrary credit words or loosely numeric prose. ASCII `c` (for example `c12.34`) is the acceptable fallback only if a rendering problem is found; if adopted, emit that form and recognize both it and the canonical U+00A4 form so earlier generated titles remain replaceable. Never use `¢`. Recognize both canonical credit and dollar suffixes regardless of current selection so switching metrics replaces the prior suffix rather than stacking it. Suffix matching used by idle selection must distinguish the selected cost basis; otherwise an existing dollar suffix could incorrectly suppress a credit update. Reports strip either recognized suffix from display identity. Avoid broad suffix matching that could remove user prose.

Persist the selected metric in native schedule installation arguments and propagate it into `schedule run` and updates on macOS, Windows, and Linux. Existing installed schedules that omit it remain API dollars. `schedule status` displays the installed metric, interpreting an absent flag as `api-usd`, independently of the metric selected for the status invocation. Selection on a read-only `schedule status` invocation does not reconfigure a job. Before manual `update` preview or apply, inspect the installed schedule read-only. If its metric differs from the selected update metric, warn with both values and explain that manual and scheduled runs can repeatedly replace each other's suffixes, consume the update limit, and append session_index entries. This includes default `api-usd` manual updates against a credits schedule, and the reverse. Warn without changing selection, reconfiguring the job, blocking an authorized apply, or adding a confirmation gate. Emit the warning on stderr so machine-readable stdout stays valid. If the installed metric cannot be determined, report that uncertainty rather than assert a match; do not assume a schedule exists when none is installed. Scheduled internal runs do not warn about their own configured metric. Switching a manual invocation does not rewrite the installed schedule or automatically migrate existing titles; use the established preview/apply and schedule-install workflows. During implementation, document the warning, flip-flop behavior, default metric, installed-metric status display, and how to align manual and scheduled selection in USERS.md; this planning update does not edit that user guide.

### Cache and implementation boundaries

No pricing-result cache is introduced. Reuse normalized usage across both metrics; a credit-only catalog change needs no analysis-cache version bump. If implementation discovers missing normalized facts, document the evidence and version only the affected cached representation. Keep one crate and use existing dependencies.

## Risks / Trade-offs

- Historical credit evidence may be sparse: expose incomplete estimates and provenance rather than inventing rates. Useful current coverage can ship while older models remain unpriced.
- Existing structured consumers may assume all numeric costs are dollars: preserve explicit USD fields, add optional unit-bearing values, and retain JSONL version 1 for this additive extension.
- Suffix matching or schedule propagation can leave stale dollar titles: verify transitions and platform command generation with existing temporary/fake-runner seams.
- Token telemetry and published rates cannot establish an actual credit deduction: label the result `Estimated credit cost` and document excluded billing dimensions.

## Implementation sequence

Implementation is on hold by owner instruction as of 2026-10-30; planning and review may continue. Once the owner explicitly resumes implementation: capture rate evidence; implement selected valuation and independent catalog; propagate through reports/exports/rates; implement title and schedule behavior; document and validate. No production account access or real scheduler installation is required for validation.
