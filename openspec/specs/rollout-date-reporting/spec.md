# Rollout Date Reporting Specification

## Purpose

Provide trustworthy local-calendar reporting across all discovered Codex rollouts or a selected project, without losing the report's existing completeness signals.

## Requirements

### Requirement: Corpus report selection
The system SHALL provide `report --all` to aggregate every discovered rollout exactly once, irrespective of rollout type, thread identifier, or project association. `--all` MUST be mutually exclusive with an exact thread selector and `--project`; existing report selection behavior without `--all` MUST remain unchanged.

#### Scenario: Corpus includes all rollout kinds
- **WHEN** the local store contains root and non-root rollouts from several projects
- **THEN** `report --all` aggregates each discovered rollout once and reports rollout-type breakdowns without requiring a project or thread identifier

#### Scenario: Invalid mixed scope is rejected
- **WHEN** a user supplies `--all` together with a thread identifier or `--project`
- **THEN** the command fails before analyzing rollouts and identifies the conflicting selectors

### Requirement: Inclusive local-calendar ranges
The system SHALL accept optional `--since YYYY-MM-DD` and `--through YYYY-MM-DD` filters for corpus and project reports. Each bound MUST use the host OS local timezone; `--since` includes its local calendar day and `--through` includes its entire local calendar day. The command MUST reject malformed dates and inverted ranges.

#### Scenario: Range includes both boundary days
- **WHEN** a report uses `--since 2026-08-01 --through 2026-08-02`
- **THEN** data attributed to either local calendar day is included and data starting on 2026-08-03 is excluded

#### Scenario: Daylight-saving boundary is respected
- **WHEN** a local-calendar range crosses a host-local daylight-saving transition
- **THEN** each calendar boundary uses that date's local midnight rather than a fixed UTC offset

### Requirement: Metadata-backed corpus pruning
For `report --all`, the system SHALL use available Codex thread metadata to exclude clearly out-of-range rollout candidates before opening their rollout files. When `--since` is present, a metadata-backed rollout whose updated timestamp is earlier than the bound's local midnight MUST be excluded. When `--through` is present, a metadata-backed rollout whose created timestamp is at or after the next local midnight MUST be excluded. Each rule SHALL apply independently. Missing, unavailable, or unusable thread metadata MUST NOT exclude a rollout candidate.

#### Scenario: Since excludes rollouts last updated before the range
- **WHEN** `report --all --since 2026-08-01` encounters a metadata-backed rollout updated before local midnight on 2026-08-01
- **THEN** the rollout is excluded before its rollout file is opened or analyzed

#### Scenario: Through excludes rollouts created after the range
- **WHEN** `report --all --through 2026-08-31` encounters a metadata-backed rollout created at or after local midnight on 2026-09-01
- **THEN** the rollout is excluded before its rollout file is opened or analyzed

#### Scenario: Metadata gaps retain candidates
- **WHEN** a discovered rollout is absent from thread metadata or has an unusable created or updated timestamp needed by the supplied bound
- **THEN** the rollout remains a candidate and the ordinary date-scoped metric filtering determines its included data

### Requirement: Date-scoped metric attribution
For a date-filtered report, the system SHALL include usage and cost by token-event timestamp and SHALL include turns and durations by turn-start timestamp. Usage or turns lacking a usable timestamp MUST be excluded from the filtered result and MUST make the result visibly incomplete; unfiltered lifetime reports MUST retain existing accounting behavior.

#### Scenario: Turn-start attribution
- **WHEN** a turn starts within the selected range and completes after the range ends
- **THEN** its turn count and duration are included in the selected range

#### Scenario: Untimestamped filtered usage is qualified
- **WHEN** a selected rollout contains otherwise attributable usage with no usable event or session timestamp
- **THEN** the filtered result excludes that usage and exposes incomplete-input metadata or a warning

### Requirement: Composable grouped reports
The system SHALL accept `--group-by` with exactly one time dimension (`day`, `week`, or `month`) and optional `rollout-type` and `model` dimensions. Day and month buckets MUST follow host-local calendar boundaries; week buckets MUST start on Monday. The report MUST include an overall aggregate for the selected scope and range in addition to grouped rows.

#### Scenario: Time and type grouping
- **WHEN** a user requests `--group-by week,rollout-type`
- **THEN** the report groups selected metrics by Monday-start local week and rollout type while retaining an overall selected-range aggregate

#### Scenario: Invalid grouping is rejected
- **WHEN** `--group-by` has no time dimension or has more than one time dimension
- **THEN** the command fails with a description of the valid grouping shape

### Requirement: Empty bucket handling
Grouped reports SHALL omit buckets with no included metrics by default. With `--include-empty`, the system SHALL emit zero-valued rows for otherwise empty time buckets inside the selected range; it MUST NOT synthesize rollout-type or model values for those rows.
The command MUST reject `--include-empty` unless both `--since` and `--through` are supplied.

#### Scenario: Default grouped output is sparse
- **WHEN** a grouped range contains an inactive calendar day
- **THEN** the default output contains no row for that day

#### Scenario: Included empty bucket has no synthetic dimension
- **WHEN** a user requests `--group-by day,model --include-empty` over a range with an inactive day
- **THEN** output contains one zero-valued day row without a model value for that day

#### Scenario: Empty buckets require a finite range
- **WHEN** a user passes `--include-empty` without both date bounds
- **THEN** the command fails before analyzing rollouts and explains that `--since` and `--through` are required
