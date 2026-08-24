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
Human and structured reports SHALL expose every target and effective-date boundary used by a model proxy. Dated `codex-auto-review` provenance SHALL identify 2026-07-30 as an announcement-date estimate and SHALL NOT describe the selected target as an observed routed model, authoritative billed model, or exact account-level cutover.

#### Scenario: Report includes Auto-review usage
- **WHEN** a report prices `codex-auto-review` usage
- **THEN** its pricing provenance shows GPT-5.4 before 2026-07-30 and GPT-5.6 Luna from 2026-07-30 with the estimator qualification

#### Scenario: Structured consumers inspect proxy metadata
- **WHEN** a structured report contains a dated proxy
- **THEN** its proxy metadata preserves target names and effective dates as separate fields rather than requiring consumers to parse a display string
