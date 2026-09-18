## MODIFIED Requirements

### Requirement: Reports expose model-proxy history and uncertainty
Structured reports SHALL expose every target and effective-date boundary used by a model proxy as typed fields. Default human reports SHALL NOT repeat full proxy histories. `report --help` SHALL explain that internal model identities may use effective-dated public-model proxies for estimation, that such boundaries are evidence-based estimates rather than observed routing or billing cutovers, and that JSON contains the exact history.

#### Scenario: Report includes Auto-review usage
- **WHEN** a default human report prices `codex-auto-review` usage
- **THEN** it may omit the dated proxy mapping and announcement-boundary disclaimer from the report body
- **AND** it does not describe the estimate as observed routing, authoritative billed-model selection, or an exact account-level cutover

#### Scenario: User inspects proxy methodology
- **WHEN** the user runs `report --help`
- **THEN** the help explains proxy estimation uncertainty and directs the user to JSON for exact effective-dated mappings

#### Scenario: Structured consumers inspect proxy metadata
- **WHEN** a structured report contains a dated proxy
- **THEN** its proxy metadata preserves every target and effective date as separate fields rather than requiring consumers to parse a display string
