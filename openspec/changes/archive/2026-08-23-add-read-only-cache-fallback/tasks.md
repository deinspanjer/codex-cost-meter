## 1. Cache access modes

- [x] 1.1 Add explicit writable/read-only connection state and retry a failed writable initialization of an existing regular cache with SQLite read-only flags; verify a focused cache test reuses a compatible cached analysis entry through the fallback.
- [x] 1.2 Make discovery and analysis store operations no-ops in read-only mode while retaining ordinary parsing for cache misses; verify a focused test proves a miss is analyzed without disabling later read-only hits or changing the cache file.

## 2. Notices and first-use behavior

- [x] 2.1 Emit one sanitized read-only-mode warning that recommends write permission, preserve writable creation/update behavior, and distinguish failed creation of a missing cache from an unusable existing cache; verify focused notice tests cover all three outcomes.
- [x] 2.2 Update `USERS.md` with writable, read-only, and unavailable cache behavior; verify the documented warning and recovery guidance match rendered output.

## 3. Validation

- [x] 3.1 Run formatting, focused cache and report CLI tests, `just check`, and `openspec validate add-read-only-cache-fallback --strict`; record all results in the implementation handoff.
