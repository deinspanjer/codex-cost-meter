## Why

Reports currently discard an existing rollout cache when the application cannot open or initialize it for writing, forcing expensive full-file analysis even though cached discovery and analysis rows may still be readable. This is especially costly for corpus reports run from read-only or sandboxed environments.

## What Changes

- Try the normal writable rollout-cache path first.
- When an existing regular cache file cannot be used for writing, retry it in SQLite read-only mode and reuse valid cached discovery and analysis rows.
- Prevent all cache mutations while operating read-only; cache misses continue through ordinary uncached analysis.
- Emit one sanitized warning that the cache is being used read-only and that write permission is required to update it.
- When no cache exists, attempt to create it; if permissions prevent creation, emit a sanitized warning that write permission is required and continue uncached.
- Preserve the existing uncached fallback when an existing cache cannot be opened safely in either mode.

## Capabilities

### New Capabilities

- `rollout-cache-access`: Defines writable, read-only fallback, warning, and unavailable behavior for the disposable rollout cache.

### Modified Capabilities

None.

## Impact

- Affects rollout-cache connection state and notices in `src/cache.rs`, plus focused cache/report tests.
- Does not change report selection, accounting, pricing, JSON schemas, rollout files, or Codex-owned SQLite state.
- Adds no dependency; `rusqlite` already supports explicit read-only open flags.
