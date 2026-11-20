## Purpose

Let users compare API dollars, published credits, and configured subscription percentage-point estimates while retaining explicit units, provenance, and safe title updates.

## ADDED Requirements

### Requirement: Global selection identifies the cost basis
The CLI SHALL accept an ordered comma-separated selection of `api-usd`, `credits`, and `quota-pp` through global `--cost-metric`, before or after subcommands. Standalone `all` SHALL select all three in that order. The default SHALL remain `api-usd`. Empty, duplicate, unknown, repeated-flag and combined-all selections SHALL be rejected.

#### Scenario: Selection placement
- **WHEN** a user selects credits before or after the report subcommand
- **THEN** both invocations produce equivalent credit valuations for the same selection

#### Scenario: Existing invocation
- **WHEN** no metric is specified
- **THEN** API-dollar calculations and human formatting retain their existing behavior

### Requirement: Credit cost counts fractional units from independent rates
The system SHALL estimate fractional credits using independently sourced, dated credit rates and the existing normalized usage. It SHALL exclude monetary purchase prices and contractual commitments. Cached input and reasoning output SHALL NOT be charged twice. Cache writes SHALL NOT attract a separate credit write surcharge.

#### Scenario: Mixed token components
- **WHEN** a supported Standard event records input containing cached input and cache-write input, plus output containing reasoning
- **THEN** each input token is charged once at its applicable input or cached rate and each output token once at its output rate
- **AND** no separate write fee or additional reasoning fee is added

#### Scenario: Speed mode
- **WHEN** a supported event has an explicit Fast or Ultrafast tier and an effective published credit rate
- **THEN** its credit estimate uses that rate independently of API or subscription-quota multipliers

### Requirement: Credit evidence bounds completeness
Credit estimates SHALL expose source and date provenance, applied-tier assumptions, unavailable history and unsupported model, component, workload, tier, context-length, and historical combinations. Unknown amounts SHALL preserve only evidenced known minima and make the complete estimate unavailable. Current credit rates SHALL NOT silently price unsupported historical periods.

#### Scenario: Historical rate unavailable
- **WHEN** an event predates the earliest evidenced applicable credit rate
- **THEN** it contributes no invented credit amount and the complete selected estimate is unavailable

#### Scenario: Dollar and credit coverage differ
- **WHEN** a model has a supported USD rate but no supported credit rate
- **THEN** its USD amount remains available while its credit amount is incomplete

#### Scenario: Credit long-context rule is unavailable
- **WHEN** an event records 272,001 gross input tokens for a model with a 272,000-input threshold and no evidenced applicable credit long-context rule
- **THEN** its complete credit estimate is unavailable rather than silently priced at base rates
- **AND** only independently evidenced lower bounds contribute to the known minimum, retaining priced contributions from other supported events
- **AND** a subscription-only no-surcharge statement does not establish a credit long-context rule

#### Scenario: Pre-card and retired-model history
- **WHEN** usage predates the earliest applicable archived card or a retired model has no applicable archived credit rate
- **THEN** the estimate remains incomplete with a known-minimum `+` display, rather than using today's card retroactively

#### Scenario: Promotional rate changes require evidence
- **WHEN** a later evidenced rate replaces GPT-5.6 Sol promotional pricing
- **THEN** dated catalog entries preserve the earlier rate for earlier events
- **AND** the stated minimum promotional period through November 21, 2026 does not itself create a later rate or expiry date

### Requirement: All cost outputs identify each selected unit
Reports, totals, rates, and exports SHALL show any selected combination separately in selection order. Existing USD fields SHALL retain their meanings. Additive `cost_metrics` and `costs` fields SHALL identify unit, nullable estimate, known minimum and independent diagnostics. JSONL SHALL retain schema version 1. Human credit and quota output SHALL distinguish positive sub-precision amounts from zero.

#### Scenario: Credits export
- **WHEN** usage is exported with credits selected
- **THEN** metadata identifies credits in cost_metrics and schema version 1, costs.credits contains credit units, and existing USD fields still contain USD values

#### Scenario: Fractional aggregation
- **WHEN** many small positive credit amounts are aggregated
- **THEN** the total is calculated before display rounding and is not the sum of individually rounded displays

### Requirement: Title costs switch basis without accumulating suffixes
The existing title `cost` slot SHALL expand the ordered selected metrics, retaining width limits and preview/apply safeguards. The system SHALL recognize canonical dollar, credit and quota suffixes independently of current selection, and distinguish expanded metric order and quota window when deciding whether an update is needed. Credit suffixes SHALL retain the strict U+00A4 grammar in the scenarios below.

#### Scenario: Switch an existing title
- **WHEN** credit mode previews an eligible title with a generated dollar suffix
- **THEN** the proposed title replaces the dollar suffix with a labeled credit suffix within its width bound
- **AND** reversing the metric likewise replaces the credit suffix without stacking suffixes

#### Scenario: Strict credit title forms
- **WHEN** a title cost renders 12.34, an incomplete known minimum of 12.34, 0.5, or 1234.56 credits
- **THEN** its suffix is respectively `¤12.34`, `¤12.34+`, `¤0.50`, or `¤1,234.56`
- **AND** suffix recognition accepts those forms while rejecting malformed grouping, missing or extra decimal digits, and loose credit prose

#### Scenario: Positive amount below display precision
- **WHEN** a positive title credit amount is below 0.01
- **THEN** its suffix is `<¤0.01`, or `<¤0.01+` for an incomplete known minimum
- **AND** suffix recognition accepts the optional leading `<` without treating the amount as zero

#### Scenario: Rendering requires an ASCII fallback
- **WHEN** an observed rendering problem requires replacing U+00A4 in emitted credit titles
- **THEN** the fallback prefix is ASCII `c`, with the same strict numeric grammar
- **AND** recognition retains both `¤` and `c` compatibility, never substituting `¢`

### Requirement: Installed schedules retain valuation and title settings
New schedules SHALL preserve the metric list, title recipe, quota window, and resolved absolute configuration path when quota is used, on all supported scheduler platforms. Existing jobs without new options SHALL retain default USD behavior. Status SHALL display installed settings independently of invocation flags. Read-only commands SHALL NOT reconfigure jobs.

#### Scenario: Credit schedule
- **WHEN** a schedule is installed with credits selected
- **THEN** the generated job invokes updates in credits mode

#### Scenario: Existing schedule
- **WHEN** an installed job invokes the executable without a metric option
- **THEN** it continues to update dollar titles

### Requirement: Manual updates warn about conflicting scheduled settings
Manual update preview/apply SHALL inspect the schedule read-only and warn on stderr if effective title metrics/order, quota window, or resolved config path differ. Warnings SHALL identify both settings and explain repeated title replacement, update-limit consumption and session_index appends, without changing selection, blocking apply, or corrupting structured stdout. Missing job metric arguments SHALL mean `api-usd`.

#### Scenario: Default manual update conflicts with a credits schedule
- **WHEN** both use the default cost title slot, the installed schedule selects credits, and a manual update omits the metric or explicitly selects api-usd
- **THEN** preview and apply warn that the two runs may repeatedly replace each other's generated title suffixes
- **AND** the update preserves its selected metric and machine-readable stdout

#### Scenario: Reverse mismatch
- **WHEN** both use the default cost title slot and a manual credits update encounters an api-usd installed schedule
- **THEN** it emits the same mismatch warning without reconfiguring the schedule

#### Scenario: Status reports persisted selection
- **WHEN** schedule status is invoked with any global metric selection
- **THEN** it displays the installed job's metrics, title recipe, quota window and applicable config path, treating a missing stored metric option as api-usd, without modifying the job

#### Scenario: No mismatch can be established
- **WHEN** all applicable effective title settings match, no schedule exists, or schedule inspection cannot determine those settings
- **THEN** matching or absent schedules produce no mismatch warning, and indeterminate inspection reports uncertainty rather than assuming a match

#### Scenario: Identical explicit title recipes
- **WHEN** manual and scheduled runs use the same explicit credit-only title recipe but different report metric selections
- **THEN** they do not warn about a title mismatch based solely on the unrelated report selection

### Requirement: Metric combinations retain independent coverage
The system SHALL support all seven nonempty subsets of the three metrics and retain selected display order. Unavailable values SHALL NOT suppress other metrics. Additive structured costs SHALL retain the existing USD fields and use separate completeness/provenance per metric.

#### Scenario: All three with a missing credit rule
- **WHEN** all metrics are requested and credit valuation is unsupported for one event while USD and configured quota are supported
- **THEN** the output includes the USD estimate, incomplete credit minimum, and quota estimate with separate labels and diagnostics

#### Scenario: Two selected metrics
- **WHEN** credits,quota-pp is selected
- **THEN** human output shows credits then quota, and structured output retains existing USD fields without presenting them as either selected unit

### Requirement: Title placeholders select explicit valuations
Title updates SHALL support explicit api-usd, credits and quota-pp slots and placeholders, independently of report selection. The title-format grammar SHALL be `{title}` followed by one or more ` · {placeholder}` segments, using cost or explicit metric/token placeholders. Explicit title-format and title-metrics SHALL conflict. Invalid grammar and duplicate expanded slots SHALL be rejected.

#### Scenario: All three placeholders
- **WHEN** an update uses `{title} · {api-usd} · {credits} · {quota-pp}` without selecting nondefault report metrics
- **THEN** it computes and displays all three title valuations, replacing the prior canonical suffix and respecting the title width

#### Scenario: Legacy slot and duplicate expansion
- **WHEN** cost,total-tokens is selected with api-usd,quota-pp
- **THEN** it expands to dollar, quota and token suffixes in that order
- **AND** cost,api-usd with the same selection is rejected as duplicate rather than displaying USD twice

#### Scenario: Safe template and width bounds
- **WHEN** a template has unknown fields, arbitrary literal suffixes, duplicate fields, or no leading title placeholder
- **THEN** validation rejects it before title writes
- **AND** a valid template that cannot fit all requested values and a visible base reports the existing width error rather than omitting a metric

### Requirement: Quota estimates use a human-editable external policy
Quota estimation SHALL use a versioned TOML file selected by quota-config path or its documented default. It SHALL support subscription plan/variant history, effective periods, separate allowance windows, model token weights, speed factors, context bands and workload factors. It SHALL NOT derive quota from the API total or public credits. Non-quota invocations SHALL NOT require this file.

#### Scenario: Default installation
- **WHEN** quota-config init is invoked against a missing resolved destination
- **THEN** it installs a commented owner-derived Pro 20x weekly preset with the research coefficient and weights documented in the design
- **AND** it identifies the preset's single-account scope without inferring the user's plan

#### Scenario: Existing user configuration
- **WHEN** initialization encounters an existing file or symlink
- **THEN** it refuses to overwrite it
- **AND** application upgrades and uninstall preserve the user-owned configuration

#### Scenario: Invalid configuration
- **WHEN** selected quota configuration is unreadable, missing, malformed, has unsupported schema/fields, nonfinite or negative numbers, duplicate IDs, or overlapping/inverted periods or context bands
- **THEN** the invocation reports an actionable path/key error before title writes or schedule installation rather than silently using default coefficients

#### Scenario: USD-only work with unrelated broken quota configuration
- **WHEN** neither report selection nor title slots request quota
- **THEN** the invocation does not fail because of an unused quota file

### Requirement: Subscription policy follows event time and explicit factors
Each quota contribution SHALL use the event's applicable half-open effective period, plan and independently configured window. Model-specific factors SHALL replace defaults for the same dimension; distinct dimensions SHALL multiply once. Unmatched periods/models/windows/factors SHALL make the full quota estimate unavailable while retaining supported contributions. Unknown timestamps SHALL NOT silently use today's policy.

#### Scenario: Plan change boundary
- **WHEN** a session spans an exclusive period end and the next period starts at that instant with a new plan or coefficient
- **THEN** each event uses its own period, the boundary event uses the new period, and output identifies both contributions

#### Scenario: Missing history and future extrapolation
- **WHEN** an event has no applicable period or no timestamp usable with disclosed provenance
- **THEN** its quota estimate is unsupported without backfilling current rules
- **AND** an event after calibrated-through in an open-ended period is estimated with explicit extrapolation provenance

#### Scenario: Custom context and speed
- **WHEN** a configured model has adjacent context bands 0–272000 and 272001-and-up and a model-specific Fast override
- **THEN** gross per-response input selects exactly one band at each boundary and the Fast override replaces the default speed factor
- **AND** API/credit context or speed surcharges are not additionally applied

#### Scenario: Unknown tier and unsupported speed
- **WHEN** tier metadata is missing but the existing assumed-Standard convention applies
- **THEN** output discloses the Standard assumption
- **AND** an explicitly recorded Fast or Ultrafast event with no configured factor is incomplete rather than priced as Standard

### Requirement: Quota computation preserves token and session accounting
Quota estimation SHALL count normalized token components and included descendants once. Cached input and cache writes SHALL be partitioned within input, and reasoning SHALL remain within output. Free-workload policy SHALL use recorded identity, not prose. Account-wide quota deltas and unexplained residuals SHALL NOT be attributed to each local session.

#### Scenario: Frozen preset arithmetic
- **WHEN** Standard GPT-6.1 Sol has 1000000 input, 800000 cached and 100000 output under the supplied flat-context preset
- **THEN** it contributes 1.48 reference units and approximately 0.12190500441696 weekly points before display rounding
- **AND** no additional reasoning, cache-write or API long-context charge is added

#### Scenario: Guardian and ordinary reviews
- **WHEN** the configured zero guardian factor applies to recorded guardian_review usage or exact codex-auto-review identity
- **THEN** those events contribute zero under that disclosed policy
- **AND** an ordinary review agent is charged normally, including when its title contains the word review

#### Scenario: Concurrent work and missing cloud usage
- **WHEN** several local sessions share quota snapshots while cloud usage is absent from the local token records
- **THEN** each local estimate uses only its own normalized included usage, with descendants counted once
- **AND** no account-quota residual is distributed among them or used as a measured session charge

#### Scenario: Several account identities
- **WHEN** selected records identify multiple accounts but only one subscription policy file is selected
- **THEN** quota aggregation refuses to mix them and explains how to separate the input; USD and credits remain independently reportable

### Requirement: Allowance windows and approximation remain explicit
Quota output SHALL identify percentage-point units, selected allowance window, plan periods, config fingerprint, calibration provenance, assumptions and coverage. The weekly preset SHALL NOT imply a five-hour estimate. Configured completeness SHALL NOT imply billing accuracy. Cumulative values SHALL NOT be clamped to 100 or presented as remaining quota.

#### Scenario: Uncalibrated five-hour window
- **WHEN** five-hour is selected with the supplied weekly-only preset
- **THEN** quota is unavailable for that window, with no inferred weekly-to-five-hour conversion

#### Scenario: Multiple reset periods
- **WHEN** supported usage spans quota resets and sums to 130 points
- **THEN** the report retains 130 estimated cumulative points with period/window context, not 100 or a negative remaining balance

#### Scenario: Empirical uncertainty
- **WHEN** the supplied pooled-context, mixed-model preset provides a numerical estimate
- **THEN** it remains labeled approximate with its calibration limits, rather than claiming measured per-session deductions or statistical confidence bounds

### Requirement: Quota title forms are canonical and window-specific
Quota title suffixes SHALL use approximate grouped two-decimal forms `~1.23pp/wk` or `~1.23pp/5h`, optional `<` after `~` for positive subprecision values, and optional trailing `+` for incomplete modeled subtotals. Unavailable values without a supported contribution SHALL use `~?pp/wk` or `~?pp/5h`. Recognition SHALL NOT remove loose percentage prose.

#### Scenario: Quota title completeness
- **WHEN** weekly quota is complete at 1.23, positive below 0.01, incomplete with subtotal 1.23, unavailable with no supported contribution, or exactly complete zero
- **THEN** the forms are `~1.23pp/wk`, `~<0.01pp/wk`, `~1.23pp/wk+`, `~?pp/wk`, and `~0.00pp/wk` respectively

#### Scenario: Window and metric switch
- **WHEN** an eligible title changes from dollar/credit/weekly quota segments to five-hour quota
- **THEN** the old canonical segments are replaced rather than stacked and weekly suffixes do not satisfy five-hour idle matching
- **AND** bare prose such as `12%` or `1.2pp` remains unchanged

### Requirement: Configuration changes are consistent within an invocation
The system SHALL read requested quota configuration once per invocation and expose its content fingerprint. Schedules SHALL use a resolved absolute path and load current contents on each run. Pricing-result caching SHALL NOT hide configuration edits; existing title idle/reprice controls SHALL remain explicit.

#### Scenario: Edited policy and cached usage
- **WHEN** a user edits a coefficient between invocations while normalized usage remains cached
- **THEN** the next report uses the changed coefficient and fingerprint without rereading unchanged rollouts
- **AND** already-written titles follow the documented idle/reprice policy rather than being silently migrated

#### Scenario: Scheduled path stability
- **WHEN** a schedule installed with quota runs from a different working directory or CODEX_HOME environment
- **THEN** it reads the same pinned absolute configuration path and reports any read/validation failure without falling back to another file
