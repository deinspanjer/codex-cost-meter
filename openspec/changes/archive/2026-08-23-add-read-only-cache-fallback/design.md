## Context

See [proposal.md](proposal.md). `RolloutCache` currently opens lazily, configures WAL and schema state, and disables caching after any open or query failure. Reads and writes share one connection, so a write restriction prevents reuse of a valid existing cache. Cache notices are emitted after the command through the existing sanitized stderr path.

## Goals / Non-Goals

**Goals:**

- Keep valid cached discovery and analysis rows usable when only write access is blocked.
- Make connection mode explicit enough that store operations become no-ops in read-only mode.
- Preserve lazy opening, file-revision validation, refresh behavior, and single-notice reporting.

**Non-Goals:**

- Changing cache schema, versions, file location, or invalidation rules.
- Making Codex-owned databases writable or treating them as the rollout cache.
- Requiring a cache for reports or updates.
- Adding date-range candidate pruning or cache hit/miss telemetry.

## Decisions

### Track writable versus read-only connection mode

Store the live connection with a small internal access mode rather than inferring writability from later SQL errors. Cache read paths use either mode. `store_discovery` and `store_analysis` return without issuing SQL when the connection is read-only, so one ordinary cache miss cannot disable otherwise useful read-only hits.

An alternative was to let writes fail and ignore SQLite's read-only error. That would route the failure through the shared query error path, discard the connection, and lose subsequent cache hits.

### Retry read-only only for a pre-existing regular cache

Keep the existing regular-file and symlink checks. Attempt the normal writable open and initialization first. If any writable step fails for an existing file—including connection open, permission hardening, WAL configuration, or schema initialization—drop that connection and retry with SQLite read-only flags. If WAL-mode SQLite cannot perform an ordinary read-only open in a non-writable directory and no WAL or shared-memory sidecar exists, retry with an encoded immutable SQLite URI. Do not use the immutable fallback when either sidecar exists because it could omit uncheckpointed cache rows. Validate that the expected cache table is queryable before accepting either read-only mode.

When the file does not exist, retain create-on-first-use. A failed creation cannot have useful cache content, so the system warns and continues uncached instead of attempting a meaningless read-only open.

Alternatives considered:

- Opening all caches read-only first would prevent normal lazy updates.
- Matching only selected OS permission error codes would miss sandbox, ACL, filesystem, or SQLite initialization failures that still permit safe reads.

### Use mode-specific, sanitized notices

On successful read-only fallback, replace the generic unavailable notice with one warning that says the existing cache is in use read-only and write permission is required to update it. If creation of a missing cache fails, warn that creation failed and write permission is required. If both access modes fail for an existing cache, retain the generic unavailable warning. All notices continue through the existing sanitizer and stderr writer.

Do not include both the writable failure and a generic unavailable warning after read-only fallback succeeds; the mode-specific warning is sufficient and avoids misleading users into thinking the cache was discarded.

## Risks / Trade-offs

- [Read-only cache contains stale or version-mismatched rows] → Keep existing file revision and discovery/analysis version checks; misses are analyzed without being stored.
- [A nominally readable SQLite file lacks the expected schema or is corrupt] → Validate a cache query before accepting read-only mode, otherwise continue through the existing unavailable path.
- [A broad writable-initialization failure masks a non-permission defect] → Read-only mode never mutates the file and emits a visible warning; failure to query still disables the cache.
- [Concurrent writer changes an immutable cache] → Prefer ordinary SQLite read-only mode and allow immutable fallback only when no WAL/shared-memory sidecar exists; the cache remains disposable and cache misses retain source-file analysis.

## Migration Plan

No data migration is required. Existing cache files remain compatible. Rollback restores the previous behavior, which may discard cache assistance in read-only environments but does not affect Codex data or report correctness.
