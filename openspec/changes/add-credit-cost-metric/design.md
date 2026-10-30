# Design

## Context

The current Clap root has no global value options. `Catalog::cost` returns an untyped numeric cost, but report and export fields explicitly name USD. Pricing and aggregation are not cached; normalized usage is. Titles use `cost` in `--title-metrics`, format dollars, and recognize dollar suffixes when replacing prior metrics. Schedules persist executable arguments separately on each supported platform.

Official evidence checked 2026-10-30:

- [Codex token credit rates](https://learn.chatgpt.com/docs/pricing#token-rates) publish separate input, cached-input, and output rates per million tokens. They explicitly exclude a separate cache-write charge and distinguish credit billing from API pricing.
- The same source states purchased-credit and Enterprise pay-as-you-go Fast and Ultrafast rates are respectively 2x and 6x Standard where supported. These are the purchased-credit / Enterprise pay-as-you-go multipliers selected for this estimate; included subscription usage instead uses 2.5x Fast and 8x Ultrafast. The latter do not apply to credit valuation.
- Some Enterprise customers retain legacy rate cards; other Enterprise agreements use USD billing. This estimate does not apply to Enterprise agreements on the legacy rate card or USD billing; it targets only the published token-based credit card.

Credits are fractional accounting units. Their purchase price, discounts, commitments, included allowance, and overage contracts do not enter the calculation. The owner's description of negotiated arrangements may apply to particular contracts; it is not a universal plan requirement.

## Goals / Non-Goals

Goals: display any combination of three independent metrics; preserve default API valuation; estimate published credits and empirically calibrated subscription percentage points honestly; make subscription assumptions human-editable without recompilation; preserve bounded, replaceable title suffixes.

Non-goals: actual invoices, account balances/deductions, remaining quota, automatic plan detection or calibration, cloud-token collection, arbitrary expressions/plugins in configuration, non-token tool/image/voice charges without adequate telemetry, or a dollar/credit exchange rate. Plan-only work must not implement, install configuration, or change titles/schedules. The third metric estimates consumption in percentage points, not a percentage of the remaining balance.

## Decisions

### CLI selection and title placeholders

Retain the owner-selected flag spelling `--cost-metric`, now accepting a nonempty ordered comma-separated list of `api-usd`, `credits`, `quota-pp`, or standalone `all`. Default is `api-usd`; `all` expands in that order. Accept before or after a subcommand; reject duplicates, empty elements, unknown values and combining `all` with other values. One list argument is sufficient; repeating the flag is an error. No aliases or short flag. Commands without valuation work accept it without side effects.

Reports, grouped totals, rates and exports show each requested metric separately in selection order. An unavailable credit or quota estimate does not suppress available USD values or other selected estimates. All three share normalized usage and descendant/fork deduplication, never aggregate account-wide quota deltas as session costs.

Examples:

```sh
codex-cost-meter report --project . --cost-metric api-usd,quota-pp
codex-cost-meter --cost-metric all report --all --json
codex-cost-meter rates --cost-metric credits,quota-pp --as-of 2026-10-30
codex-cost-meter update --title-format '{title} · {api-usd} · {credits} · {quota-pp}'
codex-cost-meter --quota-config ./my-quota.toml --quota-window weekly report --cost-metric quota-pp
codex-cost-meter --quota-config ./my-quota.toml quota-config init
```

`--quota-window weekly|five-hour` is global, default `weekly`. It chooses one independently configured denominator for this invocation; it does not convert one window into another. `quota-pp` stays one of the three metrics. Five-hour selection with the weekly-only preset yields an unavailable estimate, not zero or a scaled weekly estimate. Reports/export metadata and quota title values always identify the window. A later run can select the other window.

Extend existing `--title-metrics` with explicit `api-usd`, `credits`, and `quota-pp` slots. Existing `cost` expands the invocation's ordered selected cost metrics; existing token slots remain unchanged. For backward compatibility, title-metrics `all` retains its old expansion (`cost,total-tokens,input-tokens,output-tokens`); `cost` then expands as above. Reject duplicate resolved slots, including `cost,api-usd` when cost already includes USD. Explicit cost slots request those valuations even when the report selection is left at its default.

Add `--title-format` to update and schedule installation as an alternative to an explicitly supplied `--title-metrics`. Its deliberately bounded grammar is `{title}` followed by one or more ` · {placeholder}` segments. Placeholders are `api-usd`, `credits`, `quota-pp`, `cost` and the existing token names. The same cost expansion/duplicate rules apply. Require title once at the beginning, reject unknown fields, extra braces and arbitrary literals; this is a readable form of the existing suffix formatter, not a general template language. Formatting any of the three or all three is supported without introducing unrecognizable generated prose or per-title template storage. Defaults remain the existing title-metrics behavior.

### Separate rate sources, shared usage accounting

Add a small `CostMetric` enum and independent valuation paths over the existing normalized events. Keep the existing API catalog intact and add an independent embedded credit catalog with source URL, observation date, supported model/tier/context combinations, and evidenced effective dates. Share only lookup/arithmetic that has identical semantics. The quota estimator has its own external configuration and frozen calibration weights; it does not inherit this credit catalog or changing API rates. Do not multiply API dollars by a universal conversion factor or inherit API context surcharges.

Before implementation, capture the applicable full credit rate card and speed/context rules, reconcile every supported model, and establish historical coverage. A current observed rate is not proof of its historical start. Unsupported history remains incomplete rather than being backfilled with today's card.

Historical evidence sources are Wayback snapshots of [learn.chatgpt.com/docs/pricing](https://web.archive.org/web/*/https://learn.chatgpt.com/docs/pricing) and the [Business/Enterprise credit rate card, article 11481834](https://web.archive.org/web/*/https://help.openai.com/en/articles/11481834*). The Help Center card is already named in the API catalog's source list; that listing alone does not establish any historical credit rate. Record the exact snapshot URL, capture timestamp, contents, and any explicit effective date when evidence is collected. These are evidence sources to investigate, not a claim that qualifying snapshots have already been found.

Pre-card history is expected to remain incomplete. Events before the earliest applicable evidenced card, and retired models such as GPT-5.5, GPT-5.4, and the GPT-5.x-codex family without archived rate evidence, produce incomplete `+` estimates rather than invented historical rates. Initial coverage is therefore expected to be most useful for current-model usage; retirement itself does not preclude pricing when an applicable historical card is recovered.

The current card states GPT-5.6 Sol promotional pricing is available **at least through November 21, 2026**. The catalog's effective-date mechanism must represent a later evidenced promotional rate change without rewriting earlier history. November 21 is a minimum promotional period, not proof of a November 22 change; do not invent a future rate or automatic expiry. Missing event timestamps follow the existing latest-known convention and disclose that assumption. Preserve existing model-proxy history only where its applicability to credit valuation is justified and disclosed.

For supported token-only events, use `(noncached_input * input_rate + cached_input * cached_rate + output * output_rate) / 1_000_000`. Input and output retain existing inclusive counter semantics: cached input is a subset of input and reasoning is a subset of output. Cache-write input is charged once as ordinary noncached input, with no additional write surcharge; verify the normalized counter partition before wiring this path. Apply published credit speed rules independently of API pricing, only for supported dated combinations. Assumed Standard stays visible. Carry the existing applied-tier versus served-tier limitation into credit provenance.

Do not copy the API catalog's zero-price auto-review policy into credits without source evidence. Unsupported model, component, special workload, tier, context length, or history retains known credit minima and marks the complete estimate unavailable. Context length is an explicit completeness dimension: for a model with a 272,000 gross-input threshold, an event above that boundary (for example 272,001 input tokens) has no complete credit estimate until a per-model, dated credit long-context rule is evidenced. Do not silently reuse base rates as a complete price or borrow the API surcharge. The published GPT-6.1 Sol subscription-only no-surcharge statement does not establish credit billing rules for that or other models. Retain an independently evidenced lower bound where available; absent evidence that base rates are a lower bound for the long-context event, that event adds zero to the known minimum while other supported events retain their priced contributions. Unknown non-token charges cannot be made complete merely because the token subtotal is computable; provenance defines this as a token-based estimate and identifies any observable excluded charges. Absence of such telemetry is an explicit scope limitation, not a claim of invoice completeness.

### External subscription estimation configuration

Use commented TOML, independently of the embedded API and credit catalogs. `--quota-config PATH` selects a file. Otherwise resolve `$CODEX_HOME/codex-cost-meter/quota.toml`, falling back to `~/.codex/codex-cost-meter/quota.toml`; storage-selection `--codex-home` does not implicitly select a different estimator. Document this distinction. Relative explicit paths resolve against the invocation's working directory. Only load this file when quota is requested by output or a title slot; an unrelated broken quota config must not break USD/credits-only work.

Add `quota-config init` to create the commented preset below at the resolved path, creating parent directories and refusing to overwrite any existing file (including a dangling symlink). It prints the path and the fact that the preset is calibrated from one owner's Pro 20× account, not all Pro plans. There is no silent first-run creation, automatic upgrade replacement, or implicit subscription selection for other users. Installation is opt-in; the user edits a different plan/calibration when needed. Package this exact preset with the executable so installation needs no private logs or network. Uninstall leaves the user-owned file intact. This planning revision does not execute initialization.

A selected missing, unreadable, malformed or semantically invalid config fails with an actionable path/key error before any title writes or schedule installation; never silently use compiled fallback coefficients. An otherwise valid config with no matching event period/model/window/factor is a coverage gap: retain other metrics and modeled subtotals, with nullable complete quota estimate and diagnostics. `rates` and report provenance expose the active configuration's path, version, content fingerprint and explanatory source notes. No secrets belong in this file.

Schema version 1 has nonoverlapping `periods`, each containing:

- Stable `id`, human `subscription_plan` and `subscription_variant` strings, required UTC-offset RFC3339 `effective_from`, optional exclusive `effective_until`, calibration observation bounds/ID and notes. For example `pro`/`20x`; other plan names are permitted but require their own coefficients. The variant does not automatically divide or multiply usage by 20. One file describes the selected account's subscription history; separate accounts require separate files/invocations. Do not silently apply one plan to several known account identities in a corpus; refuse quota aggregation for that input and explain how to separate it. Missing account identity is disclosed as an assumption, not invented.
- Independent window entries `weekly` (10,080 minutes) and/or `five-hour` (300 minutes), each with a positive `points_per_reference_unit`. These are fractions of a full allowance expressed as points: 1.0 means one percentage point, not a 0.01 ratio. Missing windows are uncalibrated. Dates select policy per event, not the report end or run date; events at an exclusive end use the next applicable period. Gaps remain unsupported. Last periods may be open-ended; events beyond `calibrated_through` are explicitly extrapolated, not evidence of continued accuracy.
- Exact model IDs mapped to nonnegative input, cached-input and output weights per million tokens, with optional cache-write weight (defaults explicitly to ordinary input). No implicit API-model proxy or changing price lookup; additional IDs/aliases require explicit entries. Unknown model or positive token component with no supported weight is incomplete.
- Default `speed_multipliers` keyed by `standard`, `fast`, `ultrafast`, with optional per-model overrides. An override replaces that model/tier's default, rather than multiplying two speed factors. Missing factors are unsupported, not automatically 1.0. Map persisted `default` to Standard; retain the existing applied-versus-served-tier caveat. Missing tier may use the existing assumed-Standard policy with a visible assumption.
- Default `context_bands`, optionally replaced by a model's own bands. Each has inclusive `min_input_tokens`, optional inclusive `max_input_tokens`, and nonnegative `multiplier`. Use gross input tokens of that normalized response, including cache reads/writes, not lifetime session tokens or the model's advertised context capacity. Choose one band; multiply once. Gaps are unsupported; overlaps are invalid. The preset uses a flat 1.0 assumption because the research did not isolate context effects. Explicitly identify it as a pooled-context assumption, not proof of zero long-context surcharge. Users may replace it with, for example, adjacent 0–272000 and 272001-and-up bands and their own evidenced multipliers. Model-specific speed and context factors combine multiplicatively; do not additionally apply API or credit surcharges.
- `workload_multipliers` for `ordinary` and `guardian_review`. The preset has 1 and 0 respectively, reproducing the study's exclusion of recorded `guardian_review` usage / exact `codex-auto-review` identity. Do not classify ordinary review agents as free merely because a title/prompt says review. Use recorded source/model identity; disclose the sign-in/free-review assumption and do not transfer it to credit valuation. Users can change this policy with a dated period. A zero workload factor can establish zero without model weights; nonzero workload requires ordinary model/factor coverage.

Reject unknown schema fields/version, duplicate IDs/keys, invalid dates, inverted or overlapping periods/bands, negative or nonfinite numbers and arithmetic overflow. Require evidence/calibration labels and explicit plan/variant; do not infer unknown factors. Configuration is data only: no environment interpolation in values, includes, scripts or arbitrary executable expressions. Any future independently configured factor must have normalized telemetry and defined precedence; absence must be exposed rather than guessed. A general rules engine is unnecessary for the present requirements.

Per event, for inclusive counters I=input, C=cached input, W=cache-write input and O=output:

```text
reference_units = ((I-C-W)*input_weight + C*cached_input_weight
                   + W*cache_write_weight + O*output_weight) / 1_000_000
estimated_points = reference_units * points_per_reference_unit[window]
                   * speed_multiplier * context_multiplier * workload_multiplier
```

Validate `C+W <= I` using the existing normalized counter contract. Reasoning is already within O and must not be added again. The preset's write weight equals ordinary input, so its equation reduces exactly to the research's `(I-C)*input + C*cached + O*output`. Research contained no explicit cache-write events; that equality is an explicit preset assumption. Carry metadata needed for workload/identity/timestamp provenance through normalization if absent, with the narrow required cache-version change. Do not reconstruct those classifications from mutable titles.

Sum event estimates and linked descendants exactly once; apply rounding after aggregation. Do not clamp cumulative session/report consumption to 100, reset it when the account meter resets, or subtract it from a current remaining balance. It may span multiple cycles or configured plans. Mark which event contributions are extrapolated/assumed and which are unsupported. Completeness means coverage under the configured estimator, never measured billing accuracy. Unknown account-level activity remains unattributed; no residual is charged to a local session.

### Installable owner-derived Pro 20× preset

The full data needed to construct the preset is below. Keep the formula and numbers pinned here even if the API catalog later changes. Reference units are a frozen token-weight scale derived from the study's Standard short-context dollar proxy, not money and not public credits. Choose the most recent successful fit, **0.08236824622767631** weekly points per reference unit (the exact source ratio is supplied below). Preserve at least this precision; do not round event amounts to displayed cents/points. This yields approximately 12.1406 reference units per weekly point.

```toml
schema_version = 1

# Opt-in example from one owner's Pro 20x account; not an official rate card.
# Copy a complete period and close its predecessor when your plan/rules change.
[[periods]]
id = "owner-pro-20x-2026-10"
subscription_plan = "pro"
subscription_variant = "20x"
effective_from = "2026-10-16T00:00:00Z"
# effective_until is exclusive; omitted means continue with extrapolation notice.
calibration_id = "local-rollouts-2026-10-30-cloud-corrected"
calibrated_from = "2026-10-16T00:00:00Z"
calibrated_through = "2026-10-30T15:52:46.863Z"
notes = "Single-account empirical fit; Oct28 train / Oct29-30 validation. Flat context effects unmeasured. No independent per-session validation."

[periods.windows.weekly]
points_per_reference_unit = 0.08236824622767631
# No five-hour entry: no recent five-hour calibration was observed.

[periods.defaults]
context_bands = [{ min_input_tokens = 0, multiplier = 1.0 }]

[periods.defaults.speed_multipliers]
standard = 1.0
# Fast and Ultrafast omitted: configure separately with your own evidence.

[periods.workload_multipliers]
ordinary = 1.0
guardian_review = 0.0

[periods.models]
# Weights per MILLION tokens; write-input defaults to ordinary input weight.
"gpt-6-astra" = { input = 10.0, cached_input = 1.0, output = 50.0 }
"gpt-6.1-sol" = { input = 2.0, cached_input = 0.1, output = 10.0 }
"gpt-6.1-terra" = { input = 1.0, cached_input = 0.05, output = 5.0 }
"gpt-6-sol" = { input = 2.0, cached_input = 0.2, output = 10.0 }
"gpt-6-luna" = { input = 0.1, cached_input = 0.01, output = 0.5 }
"gpt-5.6-sol" = { input = 4.0, cached_input = 0.4, output = 20.0 }
"gpt-5.6-terra" = { input = 2.0, cached_input = 0.2, output = 12.0 }
"gpt-5.6-luna" = { input = 0.2, cached_input = 0.02, output = 1.2 }
```

Within a model entry, optional `speed_multipliers = { fast = ... }` and `context_bands = [...]` use the same typed forms as defaults. A TOML table form can replace an inline entry for easier editing. Explicitly document precedence and date examples in the installed comments/user guide. Reasoning effort has no extra coefficient in this model: its recorded output tokens are counted once. The preset must not claim independent calibration for each model, context band, speed or effort; mixed-model data only supported the pooled weighted fit. The start date is an **estimation coverage boundary**, not evidence that the user's subscription or a provider price changed on October 16. Unknown earlier history stays incomplete. The cloud-overlap interval is excluded from fitting, not from estimating locally recorded tokens: those local events can still use the preset, without absorbing missing cloud consumption.

For a concrete arithmetic check, Standard GPT-6.1 Sol with 1,000,000 input tokens (800,000 cached) and 100,000 output contributes `(200000*2 + 800000*0.1 + 100000*10)/1e6 = 1.48` reference units, hence `0.12190500441696` weekly points under this flat-context preset. Context above 272000 is deliberately a labeled assumption here; a user-defined context factor 2 would yield `0.24381000883392`, without changing USD or credit valuations. This illustrates configurable arithmetic, not a measured long-context factor.

### Research handoff: sufficient evidence without rescanning private rollouts

Research collected 2026-10-30 from this owner's local active and archived `.jsonl`/`.jsonl.zst` rollouts. The owner identifies the subscription as **20×**; logs do not independently establish that entitlement. This research is single-account, single-local-home evidence. The following supersedes the initial report's interpretation that the October 24–27 discrepancy indicated an unstable denominator.

**Coverage and accounting.** 10,916 files / 10,902 logical owners; 197,013 distinct modern response records, September3–October 30 15:52:46.863 UTC. 196,397 (99.69%) paired to a subsequent identical-category token_count event with a quota reading (median lag 0.147s, p95 30.108s). Exclude 31,755 guardian-review responses / 2,043,675,938 raw tokens; 165,258 ordinary responses / 23,707,882,899 raw tokens remain. No duplicate response IDs, invalid input/cache or output/reasoning subsets, or input+output total mismatches were found. All 72,838 ordinary modern responses from October 16 onward have the same explicit account and OpenAI provider. Modern records cover every turn with legacy token deltas after September 17, so do not add modern and legacy counters together. No explicit cache-write counters were observed in the modern sample.

**Window construction.** Use the main `codex` quota bucket's weekly window (10080 minutes); there were 329,669 deduplicated weekly observations and 40,040 historical five-hour observations. Last five-hour observation: July 10, 2026, insufficient for a current preset. Cluster reset-anchor jitter within 10 seconds; choose an actual observation closest to the minute's median used-percent (latest timestamp breaks ties). Build fixed UTC clock-hour intervals using the latest observation at/before each boundary, at most 5 minutes stale. Exclude resets, saturation >=100, internal gaps >30 minutes, backward movement >1 point, negative endpoint delta, no token records, or any non-default/missing tier. Include zero-movement intervals. Sum distinct response usage in `(left_observation,right_observation]`. The resulting 73 nominal-hour samples begin October 16. Use input/cache/output weights exactly as above, with no long-context/speed/tool premium. Fit `sum(observed_pp)/sum(reference_units)`; do not use an unweighted mean of per-hour ratios.

**Calibration and chronological validation (UTC; selected intervals, not whole-day totals).**

| Dataset | Hours | Observed weekly points | Reference units | Result |
|---|---:|---:|---:|---|
| Before documented cloud wave, Oct 16–23 | 29 | 134 | 1723.82531186 | coefficient 0.07773409467773422 |
| During cloud envelope Oct 23 21:29–Oct 27 16:36 | 22 | 167 | 901.86625060 | earlier coefficient predicts 70.10575651; residual 96.89424349 |
| After wave, Oct 28–30 | 22 | 64 | 777.16051792 | earlier coefficient predicts 60.41186928; hourly MAE 0.36775502 |
| Latest fit, Oct 28 only | 5 | 15 | 182.10901272 | coefficient 0.08236824622767631 (15 divided by reference units) |
| Holdout, Oct 29–30 | 17 | 49 | 595.05150520 | latest fit predicts 49.01334890; hourly MAE 0.2561 |

For the latest holdout: p90 absolute error 0.5427points, maximum 0.6862points, all 17 within 1 point. Moving token boundaries by ±60/120 seconds gives MAE 0.26–0.30 and total 48.56–49.81 versus 49 observed. Date splits were explored after observing the data, not preregistered; the most recent validation is short and within one weekly period. Errors are not per-session confidence intervals. Earlier-to-later agreement (~6% difference in ratios) supports the provisional range 0.078–0.082 points/reference-unit; this is neither a universal rate nor a statistical confidence interval. Nonnegative model/category regressions did not generalize consistently better; do not invent independently fitted model coefficients.

**Cloud correction.** Verified 21 cloud root chats assigned by Workstream Integration Overseer `01a135d0-ee85-7563-89f8-ea4096c34bf4`: four batches of five on Oct 23–26 plus a sixth Oct 26 slot, all acknowledged by the `durable` host with subsequent progress observations. None was a local rollout owner in the scanned inventory; cloud descendants add unquantified consumption. Class Copy #188 and report-to-ticket #209/#210 continued into Oct 27. The during-wave 96.9-point residual is **not measured cloud usage**; missing usage is strongly supported, but other factors remain possible. Do not use cloud-overlap account deltas to fit local token rates or allocate them to local sessions. `codex cloud list` returned no tasks and historical `read_thread` on durable was unavailable at investigation time; saved dispatch/progress outputs establish the missing coverage. A narrow scan of four later overseers found no explicit cloud routes across 3185 tool calls; that is not proof of zero cloud activity elsewhere.

**Attribution limits.** The 73 hourly samples contain a median 5 root sessions (range 1–16); only 4 have one. Account-level agreement does not independently validate per-session allocation. Sum each session's normalized tokens and included descendants once and label the result estimated. Other devices, cloud tasks, shared usage, model changes and future plan rules can invalidate calibration. No absolute account quota denominator, plan-to-plan scaling, Fast/Ultrafast factor or isolated context surcharge was established. Do not import published purchased-credit multipliers into this estimator. Weighting from published Standard prices was an experimental normalization, not a dollar/credit/quota billing equivalence.

**Evidence locations (optional audit, not implementation prerequisites).** The complete durable recipe and numbers are above. Original artifacts remain at `/Users/dre/.codex/visualizations/2026/10/30/01a18d65-05cc-7e61-9fd5-bff3c05b756c/quota-analysis/`: `report.txt`, `cloud-addendum.txt`, `quality.json`, `ratio-validation.json`, `timing-sensitivity.json`, `fixed-windows.json`, `cloud-evidence.json`, `manifest.json`, plus extraction/calibration scripts and checksum manifests. The cloud addendum qualifies the original report. Do not copy private rollouts or tool-message bodies into the repository; implementation needs only this numerical handoff and small synthetic behavioral fixtures. Preserve this handoff/preset provenance in durable developer documentation before archiving the change.

### Structured output and precision

Keep `estimated_cost_usd` and `known_model_cost_usd` with their current values and meanings, even when USD is not selected for human display. Add `cost_metrics` (ordered array) and `costs` (map keyed by `api-usd`, `credits`, `quota-pp`) wherever existing cost values occur. Each selected entry has `unit` (`usd`, `credits`, `percentage_points`), `estimated` (nullable), `known_minimum`, and its own completeness/assumptions/provenance. For quota, a known minimum means the supported contribution under the configured model, not a statistical lower confidence bound on actual allowance consumption. Do not ship the earlier proposal's singular `cost_metric`/`cost` shape: it has not been implemented and cannot represent combinations unambiguously.

Quota objects additionally identify `window`, configuration schema/version and content fingerprint, calibration IDs, plan/variant and effective-period breakdown, calibrated-through timestamp, extrapolation/assumption flags, and unsupported-event counts by reason. Mixed subscription periods are valued event by event and identified in the breakdown; the aggregate is cumulative points of the respective allowances, not a single account-meter movement. Do not add weekly and five-hour quantities. Structured zero and unavailable values remain distinguishable; preserve supported minima even if another event cannot be estimated.

Retain JSONL `schema_version: 1`: these fields are optional additive extensions; old USD fields and record meanings stay unchanged. Document unknown-field tolerance. Old exports without selection metadata retain their API-USD interpretation. Metadata, event records and terminal summaries consistently identify selection, units, quota window and config provenance. Ordinary JSON changes are also additive. Compute only requested non-USD valuations (plus explicit title requests); existing USD fields remain available. Reports show an `Estimated weekly usage (pp)` or `Estimated five-hour usage (pp)` label, distinct from any observed account meter. Rates shows separate selected-unit sections, with effective dates and quota factor/assumption details rather than implying an official quota rate card. Credit rates must not inherit USD cache-write-lifetime prose.

Use the project's existing `f64` calculation style, preserve up to eight decimal places in structured amounts, and round only after aggregation. Human credit amounts always use two decimals and thousands separators; report labels can spell out `credits`. In the title slot use the strict prefix form `¤12.34`, with U+00A4 (generic currency sign), and `<¤0.01` for a positive amount below 0.01. Report values may use the same form or spell out `<0.01 credits` under a credit label. Keep the known-minimum `+` convention. Human dollars retain their current rendering. Do not round credits to whole units or round each event before summing.

### Titles and scheduled updates

Use `¤12.34`, `¤0.50`, and `¤1,234.56` in credit title slots, with `¤12.34+` for an incomplete known minimum, subject to existing width bounds. The canonical grammar is optional leading `<`, U+00A4 `¤`, a strictly grouped integer, exactly two decimal digits, and optional trailing `+`. Positive amounts below 0.01 render `<¤0.01` (or `<¤0.01+` when incomplete). Keep exact amounts in structured output and round only the human display. Match this grammar as strictly as the dollar `is_cost` recognizer; do not accept arbitrary credit words or loosely numeric prose. ASCII `c` (for example `c12.34`) is the acceptable fallback only if a rendering problem is found; if adopted, emit that form and recognize both it and the canonical U+00A4 form so earlier generated titles remain replaceable. Never use `¢`. Recognize canonical credit, quota and dollar suffixes regardless of current selection so switching metrics replaces the prior suffix rather than stacking it. Suffix matching used by idle selection must distinguish the selected cost basis; otherwise an existing dollar suffix could incorrectly suppress a credit update. Reports strip every recognized suffix from display identity. Avoid broad suffix matching that could remove user prose.

Quota titles use `~1.23pp/wk` or `~1.23pp/5h`, with grouped integers and exactly two decimals; `~<0.01pp/wk` represents a positive sub-0.01 estimate, and trailing `+` marks an incomplete modeled subtotal (for example `~1.23pp/wk+`). `~` always signals approximation, even when configured coverage is complete. An unavailable estimate with no supported contribution uses `~?pp/wk` or `~?pp/5h`, not a confident zero; an exactly zero complete estimate uses `~0.00pp/wk`. Recognize these exact forms only, including after selection/window changes. Do not strip bare user prose such as `12%`, `1.2pp`, or a plan name. Credits retain the original minimum-display policy. Assemble all requested canonical segments in order, enforce existing Unicode width/visible-base bounds, and report TooNarrow rather than silently drop a selected metric. Idle eligibility compares the full expanded slot order and quota window, so a different cost basis/window does not falsely match.

Persist metric list, expanded title recipe (or original validated format), quota window, and resolved absolute quota-config path when the title uses quota, in native schedule arguments on macOS, Windows and Linux. Pin the resolved default path as well as explicit paths; current working directory or CODEX_HOME changes must not redirect later reads. Existing schedules without new arguments remain USD. `schedule status` displays installed settings independently of flags supplied to status. Read the file anew on each run, once per invocation; do not copy user config into a job or overwrite it during upgrades. Config edits affect later valuations, but already-written title suffixes still use the established idle/reprice policy: document `--reprice-before` for deliberate repricing after config changes. No new per-title fingerprint database is required.

Before manual update preview/apply, inspect the installed job read-only and warn on stderr if its effective title recipe, quota window, or resolved quota-config path differs. Include both settings and explain repeated title replacement, update-limit consumption, and session_index appends. Compare effective title valuations, not an unrelated report selection: explicit identical `{credits}` templates do not conflict merely because global report flags differ. Preserve valid structured stdout. An absent job needs no warning; failed inspection reports uncertainty. Warnings neither block authorized apply nor reconfigure anything. Scheduled internal runs do not warn about their own settings. Document selections, config paths, format syntax, status, flip-flop behavior and alignment in USERS.md.

### Cache and implementation boundaries

No pricing-result cache is introduced. Reuse normalized usage across all three metrics; catalog/config-only changes need no analysis-cache version bump. Load and validate requested quota configuration once before valuation or any title/schedule mutation, and fingerprint those bytes so an invocation cannot mix revisions. Changes are visible on the next invocation without a pricing cache. If implementation discovers missing normalized facts, document the evidence and version only the affected cached representation. Keep one crate. Add a maintained TOML parser dependency for comments, readable tables and validated typed decoding rather than writing a parser; no expression evaluator or runtime network dependency.

## Risks / Trade-offs

- Historical credit evidence may be sparse: expose incomplete estimates and provenance rather than inventing rates. Useful current coverage can ship while older models remain unpriced.
- Existing structured consumers may assume all numeric costs are dollars: preserve explicit USD fields, add optional unit-bearing values, and retain JSONL version 1 for this additive extension.
- Suffix matching or schedule propagation can leave stale dollar titles: verify transitions and platform command generation with existing temporary/fake-runner seams.
- Token telemetry and published rates cannot establish an actual credit deduction: label the result `Estimated credit cost` and document excluded billing dimensions.
- Missing cloud/device usage can distort quota calibration: retain the frozen research-derived estimator and its coverage limits; never distribute an account residual to local sessions. Per-session accuracy remains inferred.
- User-edited factors can change historical estimates: use explicit effective periods, expose the loaded fingerprint and modeled assumptions, and preserve the user's file. An open-ended period is extrapolation after its observation bounds, not a provider guarantee.
- A fixed speed-times-context product cannot express every possible provider rule: support the stated configurable model without implying unobserved interactions are calibrated; unsupported combinations remain visible. Do not turn this release into a general billing rules engine.

## Implementation sequence

Implementation is on hold by owner instruction as of 2026-10-30; planning and review may continue. Once the owner explicitly resumes implementation: capture credit evidence; implement independent valuations and the externally configurable quota preset from the research above; propagate combinations through reports/exports/rates; implement explicit title placeholders and schedule persistence; document and validate with focused checks. No production account access or real scheduler installation is required for validation.
