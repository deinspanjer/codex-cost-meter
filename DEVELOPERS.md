# Developer guide

## v0.2 architecture

One Rust 2024 crate produces one `codex-cost-meter` binary. `cli` parses `report` and `update`; `rollout::discovery` bounds and indexes JSONL without following directory symlinks; `rollout::analysis` attributes usage and preserves ambiguity; `pricing` embeds date-aware rates; `report` reuses one rollout/catalog context; `session_index` owns bounded snapshots and durable index appends; `title` owns pure metric parsing and bounded composition; `update` owns SQLite selection, the process lock, and the SQLite-then-JSONL recovery sequence; and `output` renders sanitized report output. `data/model-prices.json` is the built-in catalog.

Exact-ID reporting reads rollout JSONL plus an optional `session_index.jsonl` name. Title updates additionally read the supported `state_5.sqlite` `threads` contract documented in the user guide. `rusqlite` uses its bundled SQLite build so the Universal 2 executable has no separately installed SQLite dependency; the standard-library file lock avoids another locking dependency. The [`python-prototype/`](python-prototype/) directory is historical reference only, not the active architecture.

## Build and test

Local prerequisites are macOS with Xcode Command Line Tools (`lipo` and `codesign`), Rust 1.97.1 through `rustup`, and `just`. Before `just package`, install both supported targets:

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
cargo test --test update_cli
just check
just package
```

`just check` runs formatting, tests, version-tool tests, and warnings-denied Clippy. `just package` builds both macOS slices, creates a Universal 2 binary, verifies its architectures and ad-hoc signature, and writes a deterministic archive plus checksum under `target/release/`.

Unit tests live beside the behavior they protect; `tests/report_cli.rs` covers reporting dispatch, home resolution, output, errors, and hardening, while `tests/update_cli.rs` covers the update command contract. Title, session-index, and update tests use temporary Codex homes and cover metric boundaries, root-only selection, dry-run immutability, dual-store apply/recovery, schema compatibility, and lock/error handling. Keep tests focused on behavioral invariants, parallel-safe where possible, and input handling non-panicking and bounded.

## Release and phase gates

`Cargo.toml` is the version source. Immediately before `just bump`, keep `[Unreleased]` concise and nonempty; the command creates a new empty `[Unreleased]` while rotating its entry into a date-free release heading. `just bump major`, `minor`, or `patch` calculates the next SemVer, while an exact selector such as `just bump 1.0.0` supports an intentional major boundary. A version-changing protected-branch merge validates both macOS architectures, packages, ad-hoc-signs, checksums, tags, and publishes the archive; an unchanged version only validates.

Before a phase or release closes, require self, task, and final review; focused and full validation; durable documentation uplift; accounting; and the [release-decider rubric](docs/release/owner-approval-rubric.md). The maintainability review checks requirement traceability, focused tests, proportionate module/dependency/test growth, current consumers for abstractions, and explicit bounded follow-ups. Stop for owner review under the program-stop conditions in [ADR 0004](docs/adr/0004-preserve-the-python-prototype-and-port-to-rust.md).

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
