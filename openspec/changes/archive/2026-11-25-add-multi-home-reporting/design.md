# Design

## Context

Every report path in the current code takes exactly one `codex_home: &Path`:

- `ReportArgs::codex_home()` (`src/cli.rs`) resolves `--codex-home`, then `CODEX_HOME`, then `~/.codex`.
- `RolloutIndex` (`src/rollout/discovery.rs`) scans `<home>/sessions` and `<home>/archived_sessions`, or narrows the scan with `<home>/state_5.sqlite` in two ways: targeted paths for exact and project reports, and date pruning for the corpus. Every path collects `(RolloutRecord, mtime)` candidates and calls `resolve_duplicates` once. For each `(id, page_start)`, that function keeps the newest modification time with ties broken by path, and then merges pages per ID.
- `RolloutCache` lives at `<home>/<CACHE_FILENAME>` and is keyed by absolute rollout path plus a file fingerprint. Discovery and analysis both use it. Its access notices (read-only or unavailable) are written to stderr in `main.rs`.
- `Names` reads one home's `state_5.sqlite` and `session_index.jsonl`.
- `project.rs` reads one home's `.codex-global-state.json`. It resolves a reference to one Desktop Project (by ID) or to one path. It classifies roots by assignment, projectless marker, and working-directory containment, and then builds a targeted `ReportContext`.
- Replay suppression (`replay_prefix_len`) looks up the parent in the same index, so a combined index makes parents from other homes available automatically.
- The JSON and JSONL outputs contain no home paths. The only place a home path appears is the `RolloutNotFound` error.

The version is 1.9.0, published through GitHub releases and Homebrew. The handoff's decision 6 ("rename the report flag without a shim, pre-1.0") assumed a version that is now outdated.

## Goals / Non-Goals

**Goals:**
- Use one report code path for one or many homes. The single-home report is the one-element case, so existing report tests continue to guard single-home behavior.
- Keep discovery, deduplication, replay, and pricing rules unchanged. Only their input broadens.

**Non-Goals:**
- A per-home breakdown, or listing homes in JSON or JSONL output.
- Detecting copies of one rollout ID that diverged after each home resumed it separately (see Risks).
- Comma lists in `CODEX_HOME`, and multi-home `update` or `schedule`.

## Decisions

### 1. Keep `report --codex-home`; add `--codex-homes`; release as 1.10.0

`ReportArgs` keeps `codex_home: Option<PathBuf>` unchanged and adds `codex_homes: Vec<PathBuf>` with `value_delimiter = ','`, a value parser that rejects an empty element, and `conflicts_with = "codex_home"`. `UpdateArgs` and `ScheduledUpdateArgs` do not change, so clap rejects `--codex-homes` on those commands.

Alternatives considered:
- **Rename without an alias** (handoff decision 6). This breaks every existing script and Homebrew user that runs `report --codex-home`, and it requires 2.0.0 for a feature that only adds behavior. It also makes `report` inconsistent with `update` and `schedule`, which keep `--codex-home`.
- **Make `--codex-home` an alias of `--codex-homes`**. This silently splits an existing single path that contains a comma, and it gives one spelling two meanings.

Keeping both options is not a compatibility shim. `--codex-home` remains the single-home option on every command, and `--codex-homes` is the report-only extension. The release-bound PR runs `just bump minor`.

### 2. An ordered, de-duplicated home set resolved at the CLI boundary

`ReportArgs::codex_homes() -> Result<Vec<PathBuf>, CliError>` returns a non-empty ordered list:

- `--codex-homes` given: validate each entry with `fs::metadata`, which follows a symlink at the home path itself. Fail with a new `CliError::HomeUnavailable { entry, reason }` when an entry is missing or is not a directory. Remove entries whose canonical paths are equal, keeping the first. Keep the path as the user spelled it for display and scanning.
- Otherwise, build a one-element list from the existing `resolve_codex_home`, with no existence check, so behavior is unchanged.

Strict validation applies only to the new option. A mistyped entry in a list would otherwise drop out silently and make a partial total look complete. The single-home path keeps its current lenient behavior: a missing home produces an empty or not-found report.

### 3. Records carry their source home; the index records which homes hold each ID

`RolloutRecord` gains an internal `home: HomeId`, a small index into the home set. It is set by the scanner, is not serialized, and is not stored in the discovery cache: each cache belongs to one home, so a cache hit sets it. `RolloutIndex` gains `holders: HashMap<String, Vec<HomeId>>`, which records every home where any candidate for the ID appeared before deduplication. Project classification reads `holders`, and analysis and name lookup read `record.home`.

`resolve_duplicates` does not change. Rollout paths in different homes differ, so the existing path tie-break is total and independent of argument order.

### 4. Discovery over several homes

- **Full scan:** scan both roots of every home into one candidate list, then call `resolve_duplicates` once.
- **Corpus pruning:** compute `corpus_paths` for each home. A home whose metadata is unusable falls back to a full scan of that home only. Concatenate the results, then deduplicate once.
- **Exact task and project reports with more than one home:** use the combined full scan. With the discovery cache, a full scan is a directory walk plus cache hits, and project reports already run one (`state_roots`). A cross-home targeted lookup would need repeated passes, because `related_discovery_paths` closes descendants within one cache only. The full scan avoids that, and it also makes parents in other homes available for replay.
- **Exact task and project reports with one home:** keep today's targeted lookup, with one correction. `retain_workset` also keeps the parent record of each requested ID, for analysis only. If a requested record has a parent that the targeted pass did not find, fall back to the full scan. Without this, `report <fork-child>` loses its parent and charges the copied history to the child. This is an existing single-home bug, and the full-scan fallback fixes it. The parent is never added to the reported tree, because aggregation still iterates the requested ID and its descendants.
- **Containment:** a path taken from a home's SQLite `rollout_path` is used for that home only if it lies under that home's `sessions` or `archived_sessions` directory. Compare against both the given spelling and the canonical home path. A foreign path is treated as unindexed. For corpus pruning, this means the home's own files are found by the unindexed walk and kept. For targeted lookup, the paths count as unresolved and that home falls back to a full scan. This matters for copied homes: their SQLite keeps absolute paths that point into the original home, so without this check those files would be read and attributed to the wrong home and cache. The single-home case gets the same correction. This is the one intended change to single-home results, and the spec states it as an exception.

### 5. One analysis cache per home, chosen by record

A new `Homes` type (`src/homes.rs`) holds the ordered home paths and one `Rc<RolloutCache>` per home, opened with the same `refresh` flag. `ReportContext` holds `Rc<Homes>` in place of its single path and cache; `update` and `schedule` wrap their one home with `Homes::single`. Discovery of a path uses its home's cache. Analysis uses `record.home`. A multi-page record whose pages span homes already avoids single-file cache reuse. Cache notices already name the cache file, so `main.rs` writes every cache's notices without an extra label.

Rejected alternative: a single cache in the first home. That writes analysis of home B's files into home A, so later single-home reports of B miss the cache, and the cache would depend on argument order.

### 6. Names from the holding home first

`ReportContext` holds one `Names` per home. `name(id)` checks the home of the selected record first, then the other holding homes in canonical-path order, so a title match never depends on argument order. `ReportContext::session_index()` is used only by the single-home `update` path. It stays defined for a one-element home set and asserts that the set has one element.

### 7. Projects: resolve the target once, then classify within each holding home

`project.rs` loads `DesktopState` for each home and resolves the reference into a `Target` holding the selected project ID for each home, the fallback paths, the label, and the resolver:

- **Name and fuzzy name:** collect matching projects from every home and group them by lowercase name. More than one project in a single home, or more than one distinct group, keeps the existing ambiguity errors. Exactly one group gives a project target whose `fallback_paths` are the union of root paths across the group.
- **Path or current directory:** owners are projects in any home that have a source root whose canonical path equals the path, as today, grouped by name. Containment is used only to classify threads. More than one distinct name gives `AmbiguousSourceRoot`.
- **Thread-derived:** use the home of the selected root record. An assignment in that home gives a project target with that project's name, which then applies to all homes. No assignment gives the existing `thread_cwd` target. If that home has no metadata, return `MetadataUnavailable`.
- **Historical CWD fuzzy match:** over the combined roots, unchanged.
- Return not found only when no home matches.

Classification: for each unique root ID and each home in `holders[id]`, apply that home's state. The home's project ID is the one whose name matches the target. Combine with inclusion precedence: direct, then workspace fallback, then projectless thread. A root that no holding home included is counted once as an exclusion, but only when a holding home rejected it within the candidate scope: as projectless, or as assigned to another project. Otherwise it is ignored, as today. The root `cwd` comes from the selected record, and is filled from the SQLite of its holding homes when the record lacks one.

Rejected alternative: merge Desktop state by project ID. Separate account homes create the same project under different IDs, so a name lookup would hit `AmbiguousProjectName` in the main use case.

Rejected alternative: let the home that holds the selected copy alone decide a copied root. That makes a root's inclusion depend on file modification times, which have nothing to do with project metadata. Inclusion precedence avoids an unexplained undercount and is independent of argument order.

### 8. Errors and output

`ReportError::RolloutNotFound.home` becomes the comma-joined display of the home set. The `usage_export` placeholder changes to "selected Codex homes" only when the set has more than one element. The JSON and JSONL schemas do not change.

## Risks / Trade-offs

- **Diverged copies** → One rollout ID resumed independently in two homes after a copy contains real usage in both, but the ID-once rule counts only the newest copy. Document this in `USERS.md`. Detecting it would require comparing content, which the existing duplicate rule does not do. Copies made with fresh modification times are safe unless the original was appended to afterwards. In that case the original is newer and wins.
- **Multi-home sums differ from the sum of single-home reports** → This is intended: copies are deduplicated, and replay suppression across homes removes duplicate tokens that a single-home report of the child's home keeps. Document it.
- **An unrelated unreadable home marks the whole report incomplete** → This matches the existing single-home rule that any scan warning makes the report incomplete. It is conservative, and the warning names the home.
- **Containment changes single-home behavior for copied homes** → Before this change, a copied home whose SQLite points at the original location read the original's files. Now it reads its own files. This is a correctness fix. A home reached through a symlink that differs from both the given and canonical spellings recorded in SQLite falls back to slower scans but stays correct.
- **Exact reports over several homes scan every home** → They are slower than a targeted single-home report, but they run on the discovery cache, the same as project and `--all` reports already do. Revisit only if users report it.
- **Cache files written into every listed home** → Unchanged per-home behavior. Read-only homes use the existing read-only or unavailable fallback.

## Migration Plan

This is an additive minor release. Existing invocations are unchanged. Rollback is reverting the release, because no data format changes.
