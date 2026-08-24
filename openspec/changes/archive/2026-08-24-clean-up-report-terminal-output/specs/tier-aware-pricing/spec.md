# Tier-Aware Pricing Delta

## MODIFIED Requirements

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
