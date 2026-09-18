# Model Proxy Pricing Specification

## Purpose

Provide reproducible model-alias pricing when an internal model identity routes to different public model histories over time.

## Requirements

### Requirement: Model proxies select an effective target by event date
The system SHALL resolve a model proxy to the target effective for the usage event date before selecting that target model's effective price point. A proxy MAY have an undated baseline target followed by dated target changes.

#### Scenario: Auto-review usage crosses the announced migration date
- **WHEN** equivalent `codex-auto-review` usage events occur before and on 2026-07-30
- **THEN** the first event uses the GPT-5.4 price history and the second uses the GPT-5.6 Luna price history

#### Scenario: Static proxy remains valid for all dates
- **WHEN** a proxy has only an undated baseline target
- **THEN** the system resolves every dated event through that target

#### Scenario: Usage time is unavailable
- **WHEN** a proxied usage event has no timestamp
- **THEN** the system uses the proxy's newest target and that target's newest effective rate

### Requirement: Invalid proxy histories are rejected
The system SHALL reject an empty proxy history, a proxy target without a model price history, a dated change that is not strictly later than the preceding dated change, or an undated entry after the first proxy point.

#### Scenario: Proxy references an unknown target
- **WHEN** the catalog contains a proxy point whose target has no model price history
- **THEN** catalog loading fails with a proxy-validation error

#### Scenario: Proxy dates are unordered
- **WHEN** two dated proxy points are equal or decrease in date order
- **THEN** catalog loading fails with a proxy-validation error

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
