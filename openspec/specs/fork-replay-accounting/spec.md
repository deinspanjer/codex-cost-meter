# Fork Replay Accounting Specification

## Purpose

Ensure task and project reports exclude usage records physically replayed into legacy Codex fork rollouts while preserving every server-reported request made by the child.

## Requirements

### Requirement: Explicit fork lineage is discoverable
The system SHALL recognize a nonempty `forked_from_id` in canonical session metadata as explicit parent lineage without weakening existing lineage recognition or rollout-ID deduplication.

#### Scenario: Explicit fork joins its parent tree
- **WHEN** a selected rollout identifies an available parent through `forked_from_id`
- **THEN** task and project reporting include that rollout as a descendant of the identified parent

#### Scenario: Missing explicit parent
- **WHEN** a rollout contains `forked_from_id` but the identified parent is unavailable
- **THEN** the system retains the child usage without attempting replay suppression

### Requirement: Deterministic legacy fork replay is excluded
The system SHALL exclude only a consecutive leading sequence of child usage records that is proven to be a copied legacy fork prefix by explicit lineage, copied-history structure, and exact equality with the available parent snapshot. Equality SHALL cover all persisted token components and SHALL ignore rewritten timestamps.

#### Scenario: Complete copied prefix
- **WHEN** a legacy explicit fork begins with the complete matching parent usage sequence from its copied snapshot
- **THEN** the system excludes every matching copied record from the child totals

#### Scenario: Concurrent parent record is absent from the snapshot
- **WHEN** the child matches a leading parent sequence but the next parent record was persisted after the fork snapshot
- **THEN** the system excludes only the matched sequence and does not require the child to match that additional parent record

#### Scenario: First mismatch ends suppression
- **WHEN** one or more leading child usage records match and a later child usage record differs
- **THEN** the system retains the differing record and every subsequent child record

#### Scenario: No deterministic prefix
- **WHEN** an explicit fork has no matching leading usage record or lacks the required copied-history structure
- **THEN** the system retains all child usage

#### Scenario: Paginated or non-fork rollout
- **WHEN** a rollout uses reference-backed paginated history or has only subagent or legacy parent metadata
- **THEN** the system does not apply legacy explicit-fork replay suppression

### Requirement: Genuine child request usage remains accounted
The system SHALL retain the normalized usage of the first and subsequent genuine child API requests, including inherited conversation input, cached input, cache-write input, output, and reasoning output.

#### Scenario: First request continues the copied cumulative baseline
- **WHEN** the first post-prefix child token record contains the copied cumulative baseline plus new server-reported usage
- **THEN** the system counts only the new request delta and preserves its token-component attribution

#### Scenario: First request supplies only last-request usage
- **WHEN** the first post-prefix child token record cannot be normalized from cumulative totals but contains valid last-request usage
- **THEN** the system retains that last-request usage under the existing fallback rules

### Requirement: Heuristics never delete usage
The system MUST NOT suppress usage solely because records are temporally close or share a timestamp, model, and token signature across sessions.

#### Scenario: Rapid genuine child requests
- **WHEN** two or more genuine child usage records occur no more than one second apart without a deterministic copied prefix
- **THEN** every request remains counted

#### Scenario: Equal cross-session signatures
- **WHEN** distinct sessions contain otherwise identical usage-event signatures
- **THEN** each session's event remains independently accounted
