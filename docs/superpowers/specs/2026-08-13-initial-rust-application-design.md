# Codex Cost Meter: Initial Rust Application Design

- Status: Approved
- Date: 2026-08-13

## Purpose

Codex Cost Meter estimates the standard API-list-price equivalent and token usage of a Codex task and its descendants. It reports those measurements directly and can append selected measurements to persisted root-task titles.

The repository preserves the existing Python tools under `python-prototype/` as an archive and behavioral reference. The active implementation will be a self-contained Rust application that is easy to distribute across CPU architectures and operating systems without requiring Python on the target machine.

The estimates are diagnostic approximations, not ChatGPT billing records.

## Goals

- Provide one `codex-cost-meter` binary with reporting, update, scheduling, and uninstall modes added incrementally.
- Preserve the Python prototype's verified storage, accounting, and recovery behavior where it remains applicable.
- Support macOS on Apple Silicon and Intel first, followed by Windows and Linux.
- Keep development cycles fast with focused, parallel tests and a small dependency set.
- Fail safely and clearly across realistic variations in Codex state, filesystem behavior, permissions, and storage formats.
- Publish complete, immediately usable releases from an intentional version-changing merge to `main`.
- Keep the final release tree focused on durable documentation rather than mechanical execution records.

## Non-goals

- A long-running cross-platform daemon.
- A graphical interface or hosted service.
- A general-purpose Codex thread inspector unrelated to usage and cost.
- Support for other agent harnesses in this application. Their storage and lifecycle mechanisms are different enough to warrant separate applications if that need becomes real.
- A multi-crate workspace before a demonstrated ownership, reuse, dependency, or build boundary makes it simpler.
- Runtime price overrides, Windows arm64, production signing, or automatic issue submission in the initial release.

## Delivery roadmap

| Version | Milestone |
| --- | --- |
| `v0.1` | macOS 14 Universal 2 reporting |
| `v0.2` | macOS title updates and formatting options |
| `v0.3` | macOS scheduling lifecycle |
| `v0.4` | Windows x64 reporting and title updates |
| `v0.5` | Windows scheduling |
| `v0.6` | Linux reporting and title updates as static musl binaries for x86_64 and aarch64 |
| `v0.7` | Linux scheduling through systemd user units |
| Later `0.x` | Full price-catalog overrides and optional sanitized issue submission |
| `v1.0` | Comfortable installation and operation with production-grade macOS and Windows signing |

Patch releases deliver coherent fixes and small completed milestones without expanding feature scope. Minor-version boundaries are goals rather than permission to ship an incomplete or unsafe feature.

## Architecture

### One crate, one binary

Use one Rust 2024 binary crate while the application remains small. Shared behavior stays independent of operating system details. CPU architecture affects compilation and packaging, not source modules.

The first release creates only the modules it uses:

```text
src/
├── main.rs       # dispatch and exit handling
├── cli.rs        # command-line contract
├── report.rs     # report orchestration and result types
├── rollout.rs    # discovery, JSONL parsing, and aggregation
├── pricing.rs    # embedded catalog and cost calculation
└── output.rs     # human and JSON rendering

data/
└── model-prices.json

tests/
└── report_cli.rs # small end-to-end command contract
```

Later releases add title, storage, and scheduling modules when their behavior exists. Scheduling uses a shared command/status layer with one narrow operating-system module at a time:

- macOS: LaunchAgent and `launchctl`
- Windows: Task Scheduler
- Linux: systemd user service and timer

Conditional compilation selects the operating-system implementation. Target-specific dependencies are declared with Cargo target conditions. Empty future platform modules and interfaces with only one speculative implementation are not created.

Record this single-crate decision in an ADR. Reconsider a workspace only when there are multiple independently useful binaries, a genuinely reusable library, dependencies that cannot coexist cleanly, distinct ownership, or measured build/test costs that a crate boundary would improve.

## v0.1 reporting

### Command line

```text
codex-cost-meter report <THREAD_ID>
codex-cost-meter report <THREAD_ID> --json
codex-cost-meter report <THREAD_ID> --codex-home <PATH>
```

`--codex-home` uses `CODEX_HOME` when set and otherwise defaults to `~/.codex`.

### Data flow

1. Recursively scan regular files under `sessions/` and `archived_sessions/` without following directory symlinks.
2. Read session metadata to index rollout IDs, file locations, sources, and parent relationships.
3. When more than one file claims the same rollout ID, use the file with the newest modification time. This tolerates manually restored or copied state even though Codex-generated UUIDv7 IDs and normal archive moves make genuine ID collisions unrealistic.
4. Traverse the selected rollout and every linked descendant with cycle protection.
5. Parse turn context, lifecycle, and token events. Attribute each usage delta to its recorded model and event timestamp.
6. Convert cumulative token counters into deltas and detect counter resets without double-counting.
7. Preserve usage from unambiguous legacy rollouts. Treat copied legacy history that cannot be separated safely as unattributed rather than guessing.
8. Apply the embedded date-aware price catalog to each attributed event.
9. Read the root's latest thread name from `session_index.jsonl`, comparing valid `updated_at` values.
10. Render the result as a human report or structured JSON.

The human report preserves the useful prototype views:

- root and whole-tree scope
- descendant count and rollout type
- turn count and summed agent-turn duration
- input, cached-input, cache-write, output, and reasoning usage by model
- known and complete estimated cost
- model proxies and incomplete-estimate explanations

Unknown models or unavailable rate categories retain a known partial cost. Human output marks it with `+`; the complete JSON estimate is `null` and the structured output identifies unpriced or unattributed usage.

Malformed JSONL records are skipped and counted. A JSONL line larger than 16 MiB is skipped and counted without allocating beyond that bound; session metadata and token events are expected to remain far below it. If a scan cannot read a nonessential file or directory, the report remains available but is explicitly incomplete, so its known cost is marked and its complete estimate is unavailable. An unreadable selected root, missing task, or other failure that prevents any meaningful report produces a concise typed error on stderr and a nonzero exit status. Human output removes control characters from local metadata. User documentation warns that both human and JSON reports may include thread names and local project paths.

v0.1 does not read SQLite, mutate titles, install scheduling, or introduce unused platform abstractions.

This is a deliberately narrower read path than the combined read model in ADR 0001. Exact-ID reporting gets rollout identity and usage from rollout JSONL and the optional display name from `session_index.jsonl`; it does not need SQLite selection, history-mode naming, or update state. ADR 0001's SQLite decision applies when v0.2 selects tasks and chooses or mutates their persisted names. Clarify ADR 0001 accordingly before the interim specification is removed.

## Embedded pricing

`data/model-prices.json` is the source of truth for built-in pricing. It contains:

- an `as_of` date and source URL;
- per-model histories ordered by effective date;
- input, cached-input, cache-write, and output prices per million tokens, with unavailable categories represented explicitly;
- model proxies; and
- proxies that intentionally use the newest known price when an event date is not meaningful.

The application embeds the file with `include_str!`. It performs no runtime pricing network request and needs no build script. CI validates that the catalog parses, rates are nonnegative, effective dates are ordered and unique, and every proxy resolves. Invalid embedded data produces a typed internal error rather than a panic.

For a dated event, choose the newest rate effective no later than the event. Unknown models or missing required categories make the complete estimate unavailable without discarding known cost. Floating-point arithmetic is sufficient for these diagnostic estimates; rounding occurs only at human and JSON output boundaries.

A later `0.x` release may add `--prices-file` and a command that exports the built-in catalog. A supplied catalog replaces the built-in catalog completely instead of applying a complex overlay.

## v0.2 title updates

### Safety and selection

```text
codex-cost-meter update [selection options]
codex-cost-meter update [selection options] --apply
```

The default is a dry run. `--apply` is required for mutation and appears explicitly in installed scheduled execution.

Preserve the prototype selection modes:

- repeatable exact `--thread-id`;
- repeatable case-insensitive `--match-title`;
- `--idle-minutes` with `--limit`;
- `--max-runtime`; and
- `--reprice-before` for bounded historical repricing.

Explicit and automatic selection paths must resolve to eligible root tasks. Descendants contribute usage to a root total but cannot have their persisted titles changed.

Preserve Codex history-mode naming behavior when choosing the base title. Paginated history prefers the SQLite `name`. Legacy history prefers an explicit SQLite title that differs from the first user message, then the latest session-index name. Only when no persisted user-facing name exists may the updater synthesize a whitespace-normalized base from the first user message, SQLite title, or `Untitled` fallback.

Locate `state_5.sqlite` in the Codex-home root or its `sqlite/` subdirectory, preserving the prototype's known layouts. Before mutation, inspect only the SQLite table and columns the updater requires. New unrelated tables, indexes, columns, or a higher database schema version do not block operation. A missing or incompatible required contract stops before a write transaction begins.

Hold one process lock across the complete SQLite and `session_index.jsonl` operation. Preserve the session-index `updated_at` high-water rules from ADR 0003. An interrupted append is separated from the next valid JSONL record, flushed, and synchronized. Because SQLite and JSONL cannot be one atomic transaction, detect and repair a commit/append interruption through high-water reconciliation rather than pretending cross-store atomicity exists.

### Title composition

```text
--max-width 65
--title-metrics cost,total-tokens
```

Available metrics are:

- `cost`
- `total-tokens`
- `input-tokens`
- `output-tokens`

At least one metric is required. Preserve requested order. `all` expands to `cost,total-tokens,input-tokens,output-tokens` and cannot be combined with named metrics.

Default rendering:

```text
Task title · $12.34 · ⇄1.4M
```

Detailed rendering:

```text
Task title · $12.34 · ⇥1.2M · ↦200K
```

The complete title, including separators and suffixes, fits within `--max-width`. Requested width and metric choices persist unchanged in scheduled execution.

Recognize and replace the application's canonical trailing metric suffix, including the Python prototype's cost-only form, so repeated updates are idempotent. Do not strip similar text from the middle of a user title. Count Unicode scalar values rather than UTF-8 bytes, matching Python's practical character-count behavior without adding a grapheme dependency. Reject a maximum width too small to contain a visible base title, separator, and requested suffix.

## v0.3 scheduling and uninstall

```text
codex-cost-meter schedule install [update options]
codex-cost-meter schedule status
codex-cost-meter schedule resume
codex-cost-meter schedule remove
codex-cost-meter uninstall
```

On macOS, installation creates a LaunchAgent that invokes the canonical absolute path of the current executable. Moving the executable invalidates that schedule. The default cadence remains the prototype's five minutes. Installation persists applicable idle-selection, runtime, width, and title-metric arguments and includes `--apply` explicitly.

Scheduled runs:

- are silent on success;
- treat lock contention with an already-running updater as a successful no-op;
- maintain one small overwrite-in-place status record;
- never record task IDs, titles, prompts, project paths, or raw error chains; and
- pause after three consecutive ordinary failures or one severe failure such as disk exhaustion, incompatible required schema, or permission denial.

There is no append-only log by default. The status record may contain only the last-run time, stable result code, consecutive-failure count, paused state, and fixed allowlisted remediation text. Failure to write status never masks the primary operation result. If the update succeeds but status cannot be written, interactive output reports that the update completed while status is unavailable.

`schedule resume` clears the paused failure state. `schedule remove` unregisters automation but retains the executable. `uninstall` unregisters automation and removes the current executable; there is no separate uninstaller before a concrete platform requirement demands one. Platform-specific deletion behavior is implemented with that platform's scheduling milestone.

## Hardening and compatibility

Hardening is added with the feature it protects rather than as unused infrastructure.

### Reporting hardening

Cover behavior classes for:

- truncated final lines, malformed JSON, unknown fields/events, wrong JSON value types, and bounded handling of oversized lines;
- known historical rollout shapes and graceful degradation when future records add unknown data;
- duplicate rollout files, cyclic parent metadata, symlinked directories, and files disappearing during a scan;
- unreadable files/directories, unusual but valid Codex-home layouts, and output sanitization; and
- the invariant that no on-disk input causes a panic.

### Mutation hardening

Cover behavior classes for:

- SQLite locks, read-only databases, corruption, unsupported database formats, and incompatible required columns;
- unrelated schema additions that must remain compatible;
- disk exhaustion and permission failures before or during mutation;
- interruption between the SQLite commit and index append; and
- concurrent updater invocations.

### Scheduling hardening

Cover behavior classes for:

- overlap prevention and successful lock-contention no-ops;
- failure counting, severe-error short-circuiting, pause, and explicit resume;
- bounded status updates and safe fixed error details; and
- status-write failure without masking the primary result.

A deferred pre-1.0 feature may add `submit-issue` to prepare and, after confirmation, post a report to the project's GitHub issue tracker. It must construct diagnostics from an allowlist rather than collecting broadly and then attempting redaction. Allowed fields are limited to application version, OS/architecture, stable error code/category, and supported-format/schema identifiers. It must never capture paths, task IDs, titles, prompts, environment variables, database contents, or arbitrary error strings, and posting requires explicit user confirmation.

## Testing strategy

Use test-driven development for feature and bug work. Test the narrowest behavioral invariant that could regress.

- Unit tests live beside the behavior they exercise.
- Compatibility cases are compact and table-driven rather than one test per fixture or hypothetical.
- Tests use isolated temporary Codex homes and run in parallel by default.
- Only tests manipulating real global scheduler state serialize.
- A few load-bearing raw-format fixtures may live under `tests/fixtures/`.
- One small CLI integration test covers dispatch, stdout/stderr, and exit codes.
- No blanket snapshot suite, custom test framework, or test for static prose/constants is added.

Focused warm tests should generally finish in under one second. The complete v0.1 suite should take only a few seconds. CI records timing for review but does not enforce flaky wall-clock thresholds.

Initial dependencies are limited to:

- `clap` for command parsing;
- `serde` and `serde_json` for Codex data and the price catalog;
- `time` for timestamp and duration handling;
- `thiserror` for typed failures; and
- `tempfile` for tests.

Filesystem traversal uses the standard library. New dependencies require a demonstrated correctness or maintenance benefit.

## Maintainability gate

Every phase and release review checks that:

- each feature and test traces to an approved requirement, observed compatibility case, or concrete failure invariant;
- hypothetical suggestions are implemented only when evidence makes them sufficiently likely or valuable, otherwise recorded as a qualified backlog item or discarded;
- tests cover behavior classes rather than implementation branches or mechanical edits;
- production modules above roughly 500 non-test lines are reviewed as a signal to split by responsibility, not failed mechanically;
- abstractions have a current consumer and do not duplicate ownership;
- new dependencies, conditional compilation, test count, suite runtime, and production/test code growth remain proportionate; and
- graceful future-format handling does not attempt to predict every possible Codex change.

Passing tests does not compensate for plan divergence, incorrect ownership, missing requirements, or unmaintainable design.

## CI, versioning, and releases

### Version source and changelog

`Cargo.toml` is the version source of truth. `Cargo.lock`, the current changelog release section, and release tag must agree.

```text
just bump patch
just bump minor
just bump major
just bump 1.0.0
```

The bump command updates Cargo metadata and converts a nonempty `[Unreleased]` section in `CHANGELOG.md` into a date-free version section, then creates a new empty `[Unreleased]`. It does not commit, tag, or push. The GitHub Release timestamp is the authoritative publication date.

### Validation

Pull requests and ordinary `main` merges run:

- formatting checks;
- Clippy with warnings denied;
- tests; and
- builds for every platform/architecture supported by the current milestone.

For v0.1, CI builds the Apple Silicon and Intel release slices, combines them with `lipo`, verifies both architectures, and ad-hoc signs the final Universal 2 binary. Ad-hoc signing provides integrity but not public publisher identity. A local self-signed certificate is not used because it would require users to install and trust it manually.

Production macOS distribution later uses Developer ID signing, hardened runtime, secure timestamping, and notarization. Windows begins with an acceptable self-signing approach and gains production signing by v1.0. Linux releases use checksums.

### Automatic publication

An ordinary merge without a version change runs validation and publishes nothing. A version-changing merge to protected `main` starts one serialized release workflow that:

1. verifies the Cargo, changelog, and prospective tag versions;
2. builds and validates every artifact required by that version;
3. signs at the milestone's approved signing level;
4. packages archives and generates checksums;
5. creates the immutable tag only after every artifact succeeds; and
6. immediately publishes one GitHub Release using the matching `CHANGELOG.md` section verbatim.

Only the final publication job receives `contents: write`. A failure creates neither a tag nor a partial release. Published mistakes are corrected with a patch release rather than replacing artifacts.

v0.1.0 contains one `.tar.gz` with a separate SHA-256 checksum. The archive contains:

- the ad-hoc-signed Universal 2 `codex-cost-meter` binary;
- `README.md`; and
- `LICENSE`.

The README recommends `~/.codex/codex-cost-meter` when the user has no preferred location. Users who want direct shell invocation can instead use an existing directory on `PATH`, such as `~/.local/bin`. The project does not create `~/.codex/bin` merely to hold one executable.

## Milestone wrap-up

Each milestone is prepared on a branch. Before release:

1. Review requirements, design, plan, implementation, validation, module growth, dependencies, and test economy.
2. Uplift durable decisions and operating knowledge into README, USERS, DEVELOPERS, ADRs, TODO, and CHANGELOG as appropriate.
3. Uplift anything still needed by queued implementation, then delete mechanical specifications and plans from `docs/superpowers`. They may exist in branch history but must be absent from every final release tree.
4. Shape branch history into a small meaningful set of commits through fixups or squash when practical.
5. Run the complete supported-platform validation matrix.
6. Finalize the changelog and version with the appropriate `just bump` command.
7. Run the owner-approval release judge described below.
8. Merge to protected `main` only after the candidate passes every gate.

## Owner-approval release judge

Before every milestone release, spawn a fresh read-only subagent with a sanitized persona rubric derived from the owner's prior feedback. The public rubric records preferences and rejection triggers without private quotations, task identifiers, or transcript provenance.

The judge evaluates a candidate implementation against an already owner-approved design and plan. It is not a substitute for brainstorming, design approval, or plan approval, and it does not reopen an approved product decision merely because older durable documentation has not yet been uplifted. Apparent conflicts must be classified as candidate divergence, an approved decision awaiting documentation uplift, or genuine unresolved ambiguity before choosing a verdict.

The judge receives the approved requirements, durable decisions, implementation plan, candidate diff, validation evidence, documentation, and maintainability measurements. It must cite concrete candidate evidence and return exactly one verdict:

- `APPROVE`: likely to be approved without material changes.
- `ACCEPTABLE_WITH_FOLLOW_UP`: likely acceptable with bounded, non-structural follow-up work.
- `REJECT`: likely to provoke significant questions or concerns.

Missing milestone requirements, security or data-integrity risks, plan divergence, duplicated ownership, speculative architecture, or suspicious code/test growth require rejection. `ACCEPTABLE_WITH_FOLLOW_UP` cannot waive those concerns.

A rejection prevents merge, tag, and release. The implementation may be corrected and judged again. If rejection cannot be resolved confidently, later work may continue without releases only when it is isolated on separate branches and does not depend on or compound the rejected design. Structural or shared-foundation concerns stop dependent downstream work.

Persona construction, material persona refreshes, and judge execution are governance overhead and do not count toward implementation estimate-versus-actual comparisons. Fixes prompted by the judge do count as implementation work.

## Estimate and actual accounting

ADR 0004 remains the durable estimate ledger.

After written design approval:

1. Record this design task's tokens, agent turns, and summed agent-turn duration through the approval boundary. This background/design cost is excluded from estimate-versus-actual comparisons.
2. Record a post-design forecast before implementation-plan construction.
3. After the plan is finalized, record a post-plan forecast.
4. After every milestone, record milestone and cumulative actuals against each applicable estimate.

Use cumulative snapshots from the preserved Python reporter so the measurement method remains independent of the Rust implementation being evaluated. Include descendant agents. Public documentation records totals, not task identifiers. Summed agent-turn duration can overlap when agents run concurrently and is not wall-clock duration.

For this ledger, one agent turn is one recorded task/turn lifecycle. Actual input and output are recorded separately; cached input is already included in input, and reasoning is already included in output. Forecasts estimate output only because future input depends on context growth, cache behavior, and harness mechanics. Input remains visible in actuals but is not forced into a speculative estimate-versus-actual comparison.

Comparison boundaries are explicit:

- The original and post-design forecasts are compared with delivery work after design approval, including implementation-plan construction.
- The post-plan forecast is compared with work after plan finalization.
- Implementation, ordinary review, tests, validation, durable documentation, release preparation, and corrective rework count.
- Re-estimation, actual capture, persona construction or refresh, and release-judge execution are measured in a separate governance-overhead table and excluded.

Run estimation and accounting as distinct turns or otherwise bracket them with snapshots so their excluded cost can be identified. Record ranges and assumptions rather than false precision.

## Durable documentation resulting from this design

Before the interim specification is removed, uplift at least:

- the exact-ID reporting exception into ADR 0001 so its combined SQLite/rollout decision remains precise;
- the expanded single-crate/platform-module and platform-roadmap decision into ADR 0004 rather than creating a duplicate ADR owner;
- architecture, testing, maintainability, release-judge, and milestone-wrap-up guidance into `DEVELOPERS.md`;
- user commands, installation, privacy warnings, scheduling status, and troubleshooting into `USERS.md` as each feature ships;
- the release roadmap, price overrides, Windows arm64 decision, and sanitized issue submission into `TODO.md` while still pending;
- user-visible changes into `CHANGELOG.md`; and
- estimate snapshots, forecasts, actuals, and excluded governance overhead into ADR 0004.
