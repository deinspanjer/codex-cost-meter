# rollout-cache-access Specification

## Purpose

Keep rollout-cache reads available in restricted environments while clearly distinguishing writable, read-only, and unavailable cache operation.

## Requirements

### Requirement: Existing cache falls back to read-only access
When an existing regular rollout-cache file cannot be opened or initialized for writing, the system SHALL retry the same file in SQLite read-only mode. If the read-only cache is valid, the system SHALL reuse compatible discovery and analysis entries, SHALL analyze cache misses normally, and MUST NOT attempt cache mutations through the read-only connection.

#### Scenario: Existing cache is readable but not writable
- **WHEN** the rollout cache exists, writable initialization fails because the process lacks write permission, and SQLite can open and query the cache read-only
- **THEN** the system uses compatible cached entries, analyzes uncached or stale rollouts without storing replacements, and completes the requested operation

#### Scenario: Existing cache is writable
- **WHEN** writable cache initialization succeeds
- **THEN** the system retains normal cache reads, writes, schema initialization, and cache-update behavior without a read-only warning

#### Scenario: Existing cache is unusable in both modes
- **WHEN** writable initialization fails and the existing cache cannot be safely opened and queried read-only
- **THEN** the system emits its sanitized cache-unavailable warning and continues without cache assistance

### Requirement: Cache access degradation is visible
The system SHALL emit at most one initial cache-access notice for the selected fallback mode. A read-only fallback notice MUST state that the cache is being used read-only and that the application must be run with write permission to update the cache. A failed cache-creation notice MUST state that the cache could not be created and that write permission is required.

#### Scenario: Read-only fallback warning
- **WHEN** the system successfully falls back from writable access to an existing read-only cache
- **THEN** it emits one sanitized warning that identifies read-only cache use and recommends rerunning with write permission to update the cache

#### Scenario: Missing cache cannot be created
- **WHEN** no rollout cache exists and permissions prevent the system from creating one
- **THEN** it emits one sanitized warning that cache creation failed because write access is required and continues without cache assistance

### Requirement: Missing cache retains create-on-first-use behavior
When no rollout cache exists, the system SHALL attempt to create and initialize a writable cache. It MUST NOT attempt read-only fallback for a nonexistent cache.

#### Scenario: First use can create cache
- **WHEN** no rollout cache exists and the selected Codex home is writable
- **THEN** the system creates the private writable cache and retains the existing creation notice
