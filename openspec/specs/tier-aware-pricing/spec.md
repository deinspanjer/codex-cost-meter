# Tier-Aware Pricing Specification

## Purpose

Provide reproducible API-list cost estimates that account for the applied Codex service tier and request-size pricing without presenting uncertain usage as fully priced.

## Requirements

### Requirement: Usage retains chronological service-tier attribution
The system SHALL associate each priced usage event with the most recent service tier recorded by a preceding applied-settings event. When no tier is recorded, the system SHALL use Standard pricing and distinguish post-release assumptions from explicit or pre-release Standard usage.

#### Scenario: Tier changes within one rollout
- **WHEN** a rollout records Standard, then Fast, then Standard settings around three usage events
- **THEN** the system attributes the three events to Standard, Fast, and Standard respectively

#### Scenario: Settings omit the service tier
- **WHEN** an applied-settings snapshot omits the service tier before a usage event
- **THEN** the system prices that event as assumed Standard rather than retaining a prior tier or making the whole estimate incomplete

#### Scenario: Usage predates public Fast support
- **WHEN** usage without a recorded tier predates public Codex CLI `0.108.0-alpha.2` at 2026-03-03T05:35:04Z and its creator version does not prove Fast capability
- **THEN** the system prices it as Standard without marking it as an assumption because Fast was not yet publicly selectable

#### Scenario: Creator version proves Fast capability
- **WHEN** usage lacks a recorded tier but its canonical creator version is `0.108.0-alpha.2` or newer
- **THEN** the system prices it as assumed Standard even when its timestamp is absent or contradictory

#### Scenario: An old rollout is resumed after Fast release
- **WHEN** usage lacks a recorded tier, its creator version predates `0.108.0-alpha.2`, and the usage timestamp is at or after 2026-03-03T05:35:04Z
- **THEN** the system prices it as assumed Standard

### Requirement: Persisted tier values are normalized conservatively
The system SHALL normalize `fast` and `priority` as Fast and `default` and `standard` as Standard. Missing values SHALL follow the Standard fallback policy. Empty, unsupported, or unrecognized explicit values SHALL remain unpriced.

#### Scenario: Priority spelling selects Fast
- **WHEN** a usage event follows an applied tier value of `priority`
- **THEN** the system selects Fast pricing for that event

#### Scenario: Unsupported tier is not guessed
- **WHEN** a usage event has an applied tier other than a recognized Standard or Fast value
- **THEN** the report marks its price incomplete without substituting another tier

### Requirement: Pricing selects the event's effective rate grid
The system SHALL select rates using the event model, event date, attributed tier, and gross recorded input tokens. A request above a published input threshold SHALL use the corresponding long-context rates for the entire request; a request at or below the threshold SHALL use short-context rates.

#### Scenario: Long-context boundary
- **WHEN** otherwise identical usage events contain 272,000 and 272,001 gross input tokens for a model whose published threshold is 272,000
- **THEN** the first event uses short-context rates and the second uses long-context rates for all priced token components

#### Scenario: Fast long-context request
- **WHEN** a Fast usage event exceeds the model's long-context threshold and explicit Fast long-context rates are effective on the event date
- **THEN** every token component is priced using that Fast long-context rate set

#### Scenario: Historical rate boundary
- **WHEN** two equivalent events fall immediately before and on a rate set's effective date
- **THEN** each event uses only the rate set effective on its own date

### Requirement: Uncertain combinations preserve incomplete-cost semantics
The system SHALL return a complete best-effort estimate when every included usage event can use explicit or fallback Standard/Fast pricing and every consumed token component has an effective rate. It SHALL count post-release assumed-Standard tokens separately. Unsupported explicit tiers or unavailable rate cells SHALL retain known costs, expose the affected usage as unpriced, and mark the complete estimate unavailable.

#### Scenario: Fast marker predates durable stable support
- **WHEN** a Fast usage event predates stable `0.144.0` at 2026-07-09T16:47:12Z
- **THEN** the report excludes unsupported components from known cost and exposes an incomplete estimate

#### Scenario: Durable Fast marker uses its model premium
- **WHEN** a recognized Fast/Priority usage event occurs at or after stable `0.144.0`
- **THEN** the system applies that model's embedded Fast rate to the Standard rate effective for the event date

#### Scenario: Recorded and assumed usage are combined
- **WHEN** a report contains one explicitly priced event and one post-release event without recorded tier metadata
- **THEN** the report prices both events, emits a complete best-effort estimate, and reports the assumed-Standard token count

### Requirement: Reports identify the applied-tier estimate basis
The system SHALL identify pricing as an API-list estimate based on the tier applied in Codex rollout settings and SHALL state that the tier actually served is unavailable from current Codex token records.

#### Scenario: Pricing provenance is emitted
- **WHEN** a human or JSON report includes price metadata
- **THEN** the metadata describes the applied-tier basis and does not claim authoritative billing or served-tier attribution

### Requirement: Model reports expose service-tier detail
The system SHALL retain each model's aggregate token usage and cost and SHALL expose Standard, assumed Standard, Fast, and unavailable-tier detail in structured output under stable text keys. Human output SHALL combine explicit and assumed Standard detail into one displayed Standard mode, marked `Standard*` when any included usage is assumed, SHALL display Fast without a lightning-bolt glyph, and SHALL explain the asterisk once per report. A model with exactly one displayed service mode SHALL annotate its aggregate model row with that mode and omit service-mode child rows. A model with multiple displayed modes SHALL retain its aggregate row and render one child row per displayed Standard, Fast, or unavailable mode. Because tier is recorded per usage event rather than per turn, model turn count and duration SHALL remain on the aggregate model row and SHALL NOT be guessed or duplicated across tier detail rows.

#### Scenario: One model uses Standard and Fast
- **WHEN** a model has priced Standard and Fast usage events
- **THEN** its structured report contains separate Standard and Fast token and cost detail whose sums equal the aggregate usage and known cost
- **AND** human output retains the aggregate row and contains separate `Standard` and `Fast` child rows without a lightning-bolt glyph

#### Scenario: One model has explicit and assumed Standard only
- **WHEN** a model contains explicit Standard and assumed-Standard usage but no Fast or unavailable usage
- **THEN** human output contains one aggregate model row annotated `Standard*`, no child rows for that model, and one report-level asterisk explanation
- **AND** structured output preserves separate `standard` and `assumed_standard` entries

#### Scenario: One model has unavailable tier evidence
- **WHEN** a model has post-release usage before any applied-tier snapshot
- **THEN** its structured report contains an assumed-Standard detail entry with Standard cost and the report identifies the assumed token count
- **AND** human output marks the combined Standard mode as `Standard*`

#### Scenario: One model uses Fast only
- **WHEN** a model contains only Fast usage
- **THEN** human output contains one aggregate model row annotated `Fast`, no child row, and no lightning-bolt glyph

#### Scenario: One model has an unsupported explicit tier
- **WHEN** a model has usage after an unsupported tier value
- **THEN** its structured report contains a corresponding unavailable-tier detail row with zero known cost and an incomplete estimate
- **AND** human output exposes that unavailable mode without inventing turn or duration detail
