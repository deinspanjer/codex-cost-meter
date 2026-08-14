# Developer guide

## v0.6 architecture

One Rust 2024 crate produces one `codex-cost-meter` binary. `cli` parses cross-platform `report`, `update`, and scheduling commands on the supported scheduler targets; `rollout::discovery` bounds and indexes JSONL without following directory symlinks; `rollout::analysis` attributes usage and preserves ambiguity; `pricing` embeds date-aware rates; `report` reuses one rollout/catalog context; `session_index` owns bounded snapshots and durable index appends; `title` owns pure metric parsing and bounded composition; `update` owns SQLite selection, the process lock, and the SQLite-then-JSONL recovery sequence; `schedule` owns bounded status, result transitions, and scheduled-run orchestration; `schedule::macos` owns the current-user LaunchAgent lifecycle; `schedule::windows` owns the fixed current-user Task Scheduler lifecycle and deferred self-delete; and `output` renders sanitized report output. Compile-time target gates select the native scheduler module without a scheduler trait or Linux placeholder, so static Linux musl x86_64 and aarch64 builds support `report` and explicit `update` only. `data/model-prices.json` is the built-in catalog.

Exact-ID reporting reads rollout JSONL plus an optional `session_index.jsonl` name. Title updates additionally read the supported `state_5.sqlite` `threads` contract documented in the user guide. `rusqlite` uses its bundled SQLite build so the Universal 2 executable has no separately installed SQLite dependency; the standard-library file lock avoids another locking dependency. The scheduler writes one bounded, synchronized replacement status record with allowlisted remediation only; it pauses after three ordinary failures or immediately for disk-full, schema, and permission failures. macOS and Windows lifecycle modules use fixed native tool paths and private fake-runner seams, so tests cover command plans without registering a real job. Windows writes Task Scheduler XML only as a synchronized temporary file, queries the fixed task with HRESULT status, and keeps scheduler state under `LOCALAPPDATA`; its native cleanup script receives the executable as an argument rather than interpolating it. The [`python-prototype/`](python-prototype/) directory is historical reference only, not the active architecture.

## Build and test

Local development requires Rust 1.97.1 through `rustup`; the repository's `just` recipes require `just`. macOS packaging additionally requires Xcode Command Line Tools (`lipo` and `codesign`). Before `just package`, install both supported macOS targets:

```text
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

Use focused commands while iterating:

```text
just test-filter report::tests
just test-filter output::tests
just test-version
just test-filter title::tests
just test-filter update::tests
cargo test schedule::tests
cargo test schedule::macos::tests
cargo test schedule::windows::tests
cargo test --test schedule_cli
cargo test --test windows_schedule_cli
cargo test --test update_cli
just check
just package
```

`just check` runs formatting, tests, version-tool tests, and warnings-denied Clippy. `just package` builds both macOS slices, creates a Universal 2 binary, verifies its architectures and ad-hoc signature, and writes a deterministic archive plus checksum under `target/release/`. Native Windows CI runs the corresponding checks and release build on `x86_64-pc-windows-msvc`, then creates and inspects the deterministic Windows ZIP and checksum. Native Linux CI builds static musl binaries for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, then uses `python3 scripts/version.py package-linux --architecture x86_64|aarch64` to create and inspect each deterministic archive and checksum. Before release, inspect all archive member lists and checksums, the Universal 2 architectures and strict ad-hoc signature, and the Windows x64 fixture.

Unit tests live beside the behavior they protect; `tests/report_cli.rs` covers reporting dispatch, home resolution, output, errors, and hardening, while `tests/update_cli.rs`, `tests/schedule_cli.rs`, and `tests/windows_schedule_cli.rs` cover update and native scheduling command contracts. Title, session-index, update, and scheduling tests use temporary homes and cover metric boundaries, root-only selection, dry-run immutability, dual-store apply/recovery, schema compatibility, lock/error handling, circuit-breaker transitions, bounded status, Windows argument quoting, task lifecycle, and deferred cleanup. Keep tests focused on behavioral invariants, parallel-safe where possible, and input handling non-panicking and bounded.

## Release and phase gates

`Cargo.toml` is the version source. Immediately before `just bump`, keep `[Unreleased]` concise and nonempty; the command creates a new empty `[Unreleased]` while rotating its entry into a date-free release heading. `just bump major`, `minor`, or `patch` calculates the next SemVer, while an exact selector such as `just bump 1.0.0` supports an intentional major boundary. A version-changing protected-branch merge validates native macOS, Windows, and Linux builds, packages and checksums one macOS archive, one Windows archive, and two Linux musl archives, ad-hoc-signs the Universal 2 binary, tags the merge, and publishes exactly eight assets; an unchanged version only validates.

Before a phase or release closes, require self, task, and final review; focused and full validation; durable documentation uplift; and accounting. The maintainability review checks requirement traceability, focused tests, proportionate module/dependency/test growth, current consumers for abstractions, and explicit bounded follow-ups. Stop for owner review under the program-stop conditions in [ADR 0004](docs/adr/0004-preserve-the-python-prototype-and-port-to-rust.md).

## Documentation Placement Rules

| Document | Canonical content |
| --- | --- |
| `README.md` | Entry point, capability summary, and links |
| `USERS.md` | Installation, runtime behavior, privacy, and troubleshooting |
| `DEVELOPERS.md` | Architecture, tooling, tests, release process, and these rules |
| `TODO.md` | Actionable future work only |

## Change-Driven Update Matrix

| Change | Update |
| --- | --- |
| CLI flag, environment variable, or operator-visible error | `USERS.md` (and this guide when developer-impacting) |
| User workflow or major capability | `README.md` and `USERS.md`, plus this guide for implementation/release impact |
| Internal refactor or developer tooling/tests | This guide |
| Durable decision | `docs/adr/` and an appropriate link |
| Deferred work or design question | `TODO.md` |
