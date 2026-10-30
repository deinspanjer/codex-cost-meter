## ADDED Requirements

### Requirement: Global selection identifies the cost basis
The CLI SHALL accept `--cost-metric api-usd|credits` before or after subcommands, default to `api-usd`, and apply the selection to cost-valuing operations. It SHALL NOT accept quota percentage as an implemented metric in this release.

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

### Requirement: All cost outputs identify the selected unit
Reports, totals, rates, and exports SHALL identify the selected metric. Existing USD fields SHALL retain their meanings. Selected costs SHALL include unit, nullable estimate, and known minimum. JSONL SHALL retain schema version 1 with selected-cost fields documented as optional additive extensions. Human credit output SHALL allow fractions and distinguish positive values below display precision from zero.

#### Scenario: Credits export
- **WHEN** usage is exported with credits selected
- **THEN** metadata identifies credits and schema version 1, selected cost objects contain credits, and existing USD fields still contain USD values

#### Scenario: Fractional aggregation
- **WHEN** many small positive credit amounts are aggregated
- **THEN** the total is calculated before display rounding and is not the sum of individually rounded displays

### Requirement: Title costs switch basis without accumulating suffixes
The existing title `cost` slot SHALL use the selected metric and retain width limits and preview/apply safeguards. Credit suffixes SHALL use U+00A4 `¤`, a strictly grouped integer, exactly two decimals, optional leading `<`, and optional trailing `+`. The system SHALL recognize canonical dollar and credit suffixes when replacing or hiding generated metrics, and distinguish their bases when deciding whether an update is needed.

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

### Requirement: Installed schedules retain their selected metric
Newly installed schedules SHALL carry the selected metric into subsequent runs on every supported scheduler platform. Existing jobs without the option SHALL retain the default USD behavior. Read-only schedule commands SHALL NOT change installed valuation.

#### Scenario: Credit schedule
- **WHEN** a schedule is installed with credits selected
- **THEN** the generated job invokes updates in credits mode

#### Scenario: Existing schedule
- **WHEN** an installed job invokes the executable without a metric option
- **THEN** it continues to update dollar titles

### Requirement: Manual updates warn about installed metric mismatches
Manual update preview and apply SHALL inspect the installed schedule read-only and warn on stderr when its metric differs from the selected update metric. The warning SHALL identify both metrics and explain repeated title replacement, update-limit consumption, and session_index appends, without changing selection or blocking apply. Schedule status SHALL display the installed metric; missing job arguments mean `api-usd`.

#### Scenario: Default manual update conflicts with a credits schedule
- **WHEN** an installed schedule uses credits and a manual update omits the metric or explicitly selects api-usd
- **THEN** preview and apply warn that the two runs may repeatedly replace each other's generated title suffixes
- **AND** the update preserves its selected metric and machine-readable stdout

#### Scenario: Reverse mismatch
- **WHEN** a manual credits update encounters an api-usd installed schedule
- **THEN** it emits the same mismatch warning without reconfiguring the schedule

#### Scenario: Status reports persisted selection
- **WHEN** schedule status is invoked with any global metric selection
- **THEN** it displays the installed job's metric, treating a missing stored metric option as api-usd, without modifying the job

#### Scenario: No mismatch can be established
- **WHEN** the installed metric matches, no schedule exists, or schedule inspection cannot determine its metric
- **THEN** matching or absent schedules produce no mismatch warning, and indeterminate inspection reports uncertainty rather than assuming a match
