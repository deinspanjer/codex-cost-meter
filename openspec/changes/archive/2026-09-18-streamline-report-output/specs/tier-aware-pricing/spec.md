## MODIFIED Requirements

### Requirement: Reports identify the applied-tier estimate basis
The system SHALL identify pricing as an API-list estimate based on the tier applied in Codex rollout settings and SHALL NOT claim authoritative billing or served-tier attribution. Human output SHALL state this basis and the pricing-catalog date concisely; `report --help` SHALL explain that the tier actually served is unavailable from current Codex token records; JSON SHALL retain exact pricing metadata.

#### Scenario: Pricing provenance is emitted
- **WHEN** a human report includes a complete price estimate
- **THEN** its footer identifies the result as an API-list estimate and gives the pricing-catalog date without repeating pricing source URLs or the full tier methodology

#### Scenario: Structured pricing provenance is emitted
- **WHEN** a JSON report includes price metadata
- **THEN** the metadata describes the applied-tier basis and does not claim authoritative billing or served-tier attribution

### Requirement: Model reports expose service-tier detail
The system SHALL retain each model's aggregate token usage and cost and SHALL expose Standard, assumed Standard, Fast, and unavailable-tier detail in structured output under stable text keys. Human output SHALL omit ordinary Standard annotations, SHALL report the total assumed-Standard token count once as a concise conditional warning, and SHALL preserve Fast, mixed-mode, and unavailable-tier distinctions when they materially affect interpretation. A wholly redundant single-model section MAY be omitted only when doing so hides no exceptional tier distinction. Because tier is recorded per usage event rather than per turn, model turn count and duration SHALL remain on aggregate model rows and SHALL NOT be guessed or duplicated across tier detail rows.

#### Scenario: One model uses Standard and Fast
- **WHEN** a model has priced Standard and Fast usage events
- **THEN** its structured report contains separate Standard and Fast token and cost detail whose sums equal the aggregate usage and known cost
- **AND** human output retains the material Standard/Fast distinction without a lightning-bolt glyph

#### Scenario: Report contains assumed Standard usage
- **WHEN** one or more models contain post-release usage without a recorded tier
- **THEN** human output emits one concise report-level warning with the total tokens priced as assumed Standard
- **AND** structured output preserves separate `standard` and `assumed_standard` entries per model

#### Scenario: One model has explicit and assumed Standard only
- **WHEN** one model contains explicit Standard and assumed-Standard usage but no Fast or unavailable usage
- **THEN** human output uses one concise report-level assumed-token warning instead of annotating the aggregate model row or adding tier child rows
- **AND** structured output preserves separate `standard` and `assumed_standard` entries

#### Scenario: One model has unavailable tier evidence
- **WHEN** a model has post-release usage before any applied-tier snapshot
- **THEN** structured output contains an assumed-Standard detail entry with Standard cost and human output includes that usage in the report-level assumed-token warning

#### Scenario: One ordinary Standard model duplicates the scope
- **WHEN** exactly one model accounts for the report and it contains only ordinary or assumed Standard pricing
- **THEN** human output may omit the redundant model section while retaining any assumed-Standard warning

#### Scenario: One model uses Fast only
- **WHEN** a model contains only Fast usage
- **THEN** human output keeps the Fast distinction visible even if the model otherwise duplicates the report total

#### Scenario: One model has an unsupported explicit tier
- **WHEN** a model has usage after an unsupported tier value
- **THEN** its structured report contains a corresponding unavailable-tier detail row with zero known cost and an incomplete estimate
- **AND** human output exposes the unavailable mode without inventing turn or duration detail

#### Scenario: Complete estimate is unavailable
- **WHEN** unsupported or incomplete input prevents a complete price estimate
- **THEN** human output marks cost as a known lower bound and explains the marker concisely for that report
