# Architecture and tests

[Developer guide](../DEVELOPERS.md)

## Runtime and storage

One Rust 2024 crate produces one `codex-cost-meter` binary. `cli` parses cross-platform `report`, `update`, and scheduling commands on the supported scheduler targets; `rollout::discovery` bounds and indexes JSONL without following directory symlinks; `rollout::analysis` attributes usage and preserves ambiguity; `cache` owns the disposable app SQLite cache; `pricing` merges date-aware embedded rates with a user-owned catalog; `report` reuses one selected rollout/catalog context; `session_index` owns bounded snapshots and durable index appends; `title` owns pure metric parsing and bounded composition; `update` owns SQLite selection, the process lock, and the SQLite-then-JSONL recovery sequence; `schedule` owns bounded status, result transitions, and scheduled-run orchestration; `schedule::linux` owns the fixed current-user systemd service/timer lifecycle; `schedule::macos` owns the current-user LaunchAgent lifecycle; `schedule::windows` owns the fixed current-user Task Scheduler lifecycle and deferred self-delete; and `output` renders sanitized report output. Compile-time target gates select the native scheduler module without a cross-platform scheduler abstraction. Linux keeps its service and timer under XDG user configuration and its bounded status under XDG state; macOS and Windows retain their native paths. `data/model-prices.json` is the built-in catalog.

`thread_name` shares Codex history-mode naming rules with the updater and enriches reports through optional read-only SQLite lookups and session-index fallback.

Exact and project reporting may use Codex's SQLite projection to select rollout paths, but rollout JSONL remains authoritative and an incompatible or incomplete projection falls back to file discovery. `codex-cost-meter.sqlite` stores versioned discovery and parsed-analysis JSON keyed by rollout path; analysis reuse requires matching nanosecond modification time and size, and cache failures disable caching for the rest of that command. Pricing and report aggregation are deliberately not cached. Title updates additionally read the supported `state_5.sqlite` `threads` contract documented in the user guide. `rusqlite` uses its bundled SQLite build so the Universal 2 executable has no separately installed SQLite dependency; the standard-library file lock avoids another locking dependency. The scheduler writes one bounded, synchronized replacement status record with allowlisted remediation only; it pauses after three ordinary failures or immediately for disk-full, schema, and permission failures. macOS and Windows lifecycle modules use fixed native tool paths and private fake-runner seams, so tests cover command plans without registering a real job. Windows writes Task Scheduler XML only as a synchronized temporary file, queries the fixed task with HRESULT status, and keeps scheduler state under `LOCALAPPDATA`; its native cleanup script receives the executable as an argument rather than interpolating it. The [`python-prototype/`](../python-prototype/) directory is historical reference only, not the active architecture.

Rollout discovery accepts `.jsonl` and `.jsonl.zst` files; the bundled Zstandard decoder streams decompressed bytes through the same bounded-record reader. SQLite paths that still name a compressed file's former `.jsonl` name resolve to its `.jsonl.zst` sibling. Native page headers identify pages by ordinal and timestamp within a session. Copies of a page retain the existing modification-time/path tie-break; distinct pages are read in timestamp order as one logical rollout. Repeated page headers are omitted, and exact overlapping records are deduplicated while distinct records with reused ordinals survive. Normalization, model/tier attribution, turn lifecycles, and legacy fork replay operate over the combined stream. Multi-page analysis bypasses single-file cache entries so changes to any page are immediately visible; discovery and single-file analysis remain cached.

Bump `DISCOVERY_VERSION` when session-metadata interpretation or cached linkage changes, and bump `ANALYSIS_VERSION` when parsed `RolloutStats` semantics change. Pricing or report-only changes need neither bump because those calculations remain live. Per-thread report rows are transient views over existing usage facts; exact-thread date bounds reuse the same scoped event aggregation as project/corpus reports.

## Desktop title updates

The [Codex app-server API](https://learn.chatgpt.com/docs/app-server#api-overview) documents `thread/name/set` and its `thread/name/updated` notification. That renames a thread through a connected server. It is not a reload signal for direct storage edits. The public docs do not describe a way for an external tool to attach to the server of an already-running local desktop app or add an action to its thread context menu.

Keep scheduled updates as the default. If the delay becomes a problem, [Codex lifecycle hooks](https://learn.chatgpt.com/docs/hooks#stop) offer `UserPromptSubmit` and `Stop`, and the [notification setting](https://learn.chatgpt.com/docs/config-file/config-advanced#notifications) documents `agent-turn-complete`. Before using a turn-end trigger, verify that the rollout contains final usage and Codex has settled on a usable title. None of those triggers is a general idle event.

## Usage exports

`usage_export` writes schema-1 metadata, normalized usage rows and a terminal summary. Project selection resolves to a `ProjectScope` shared with ordinary reporting. The located reader preserves physical decompressed line numbers across page overlap and replay suppression. Export always reparses; locator/turn/ordinal metadata remains transient and no export files are retained by the app. Shared normalized identity/workload facts use the analysis version described below. Read/write failures return nonzero without a success summary. Stream success and per-metric completeness remain separate.

The reporting CLI tests verify dated self/tree separation, self-only per-thread reconciliation, physical locators across compressed pages and reused ordinals, counter resets, legacy replay, selection parity, unknown/untimestamped data, and fresh/cached export equality. A failed-writer test checks that a partial stream cannot end with a successful summary.

## Independent cost metrics

`costs` holds ordered selection, per-metric ledgers and unit-bearing output.
`credits` reads an independent embedded observation-dated card; `quota` uses
typed, validated TOML and the frozen opt-in preset. Neither inherits API model
proxies, zero-price policies, context surcharges or mutable API rates.
The invocation's existing catalog snapshot carries the independent cost context
so all report/export/title paths receive one selection and one quota snapshot.
FNV-1a-64 fingerprints identify file contents, not security signatures.

Analysis-cache version 8 adds timestamp-source, recorded guardian classification
and account-identity facts to normalized events. Completion metadata attaches
only to identical-category normalized token_count usage; completion counters
are never charged again. Creator account identity is a disclosed resume-time
assumption, not verified billing identity. Locator/ordinal export data remains
transient. Discovery semantics are unchanged. Valuations/configs are not cached.

Per-metric ledgers are accumulated before rounding and carried through scope,
model, tier and date/group aggregation. Legacy USD completeness and amounts
retain their meaning; each extra metric has its own unsupported/assumed counts.
Quota provenance retains policy periods and contributions, selected window,
calibration bounds, assumptions and extrapolation. Known mixed accounts cannot
be assigned one quota subtotal.

Native jobs persist validated expanded recipes and global valuation arguments;
status reads bounded native definitions, returning indeterminate rather than
guessing unsupported syntax. Manual warnings compare effective titles, not
unrelated report selection, and write only stderr. No sidecar settings store,
runtime network dependency or automatic user-policy replacement is introduced.
Focused coverage lives in `credits`, `quota`, `costs`, `title`, `cli`, native
scheduler tests and `tests/cost_metrics_cli.rs`. The latter reconciles cached
reports, grouped totals, schema-1 export and immutable title preview across a
policy boundary and unsupported speed.

## Rate table

The cross-platform `rates` command reads the same offline effective pricing snapshot as valuation. Its sorted snapshot and cost estimation share effective-date/proxy lookup; rendering preserves separate context bands, tiers, and unknown rate cells. It does not initialize session discovery, storage, or cache. `tests/rates_cli.rs` covers the CLI's exact components, historical and proxy boundaries, unknown cells, invalid selections, and storage independence.

## Shell completions

`completions bash|zsh` uses `clap_complete` to generate scripts from the same Clap command definition as argument parsing.

Homebrew installation is covered in the [release guide](releases.md#homebrew).

## Tests

Unit tests live beside the behavior they protect; `tests/report_cli.rs` covers reporting dispatch, home resolution, output, errors, and hardening, while `tests/update_cli.rs`, `tests/schedule_cli.rs`, `tests/linux_schedule_cli.rs`, and `tests/windows_schedule_cli.rs` cover update and native scheduling command contracts. Title, session-index, update, and scheduling tests use temporary homes and cover metric boundaries, root-only selection, dry-run immutability, dual-store apply/recovery, schema compatibility, lock/error handling, circuit-breaker transitions, bounded status, Linux systemd unit/command plans, Windows argument quoting, task lifecycle, and deferred cleanup. Keep tests focused on behavioral invariants, parallel-safe where possible, and input handling non-panicking and bounded.


Local pricing uses `pricing::catalog` for sparse overlay/standalone schemas and merging, `pricing_store` for bounded user-home reads and compare-before-rename/reset, `pricing::proposal` for typed atomic operations and history-preserving pruning, `pricing_evidence` for bounded official-source acquisition/verification, and `pricing_command` for review, JSON outcomes, offline research prompts and activation. Imported proposals and deterministic observations share one validator. No source evidence or old catalog revisions persist; only the active overlay and bounded-lifetime temp files are app-managed. The one invocation snapshot is explicitly supplied to report, export, title-update and scheduled-title-update valuation. The parser cache continues to store usage facts, not prices.
