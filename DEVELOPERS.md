# Developer guide

## v0.1 architecture

One Rust 2024 crate produces one read-only `codex-cost-meter` binary. `cli` parses `report`; `rollout::discovery` bounds and indexes JSONL without following directory symlinks; `rollout::analysis` attributes usage and preserves ambiguity; `pricing` embeds date-aware rates; `report` aggregates a root and descendants; and `output` renders sanitized human or structured JSON output. `data/model-prices.json` is the built-in catalog.

Exact-ID reporting reads rollout JSONL plus an optional `session_index.jsonl` name; it deliberately does not need SQLite. The [`python-prototype/`](python-prototype/) directory is historical reference only, not the active architecture.

## Build and test

Local prerequisites are macOS with Xcode Command Line Tools (`lipo` and `codesign`), Rust 1.97.1 through `rustup`, and `just`. Before `just package`, install the Intel target on the Apple Silicon build host:

```text
rustup target add x86_64-apple-darwin
```

Use focused commands while iterating:

```text
just test-filter report::tests
just test-filter output::tests
just test-version
just check
just package
```

`just check` runs formatting, tests, version-tool tests, and warnings-denied Clippy. `just package` builds both macOS slices, creates a Universal 2 binary, verifies its architectures and ad-hoc signature, and writes a deterministic archive plus checksum under `target/release/`.

Unit tests live beside the behavior they protect; `tests/report_cli.rs` covers command dispatch, home resolution, output, errors, and hardening. Keep tests focused on behavioral invariants, use temporary Codex homes, and keep input handling non-panicking and bounded.

## Release and phase gates

`Cargo.toml` is the version source. Immediately before `just bump`, keep `[Unreleased]` concise and nonempty; the command creates a new empty `[Unreleased]` while rotating its entry into a date-free release heading. `just bump major`, `minor`, or `patch` calculates the next SemVer, while an exact selector such as `just bump 1.0.0` supports an intentional major boundary. A version-changing protected-branch merge validates both macOS architectures, packages, ad-hoc-signs, checksums, tags, and publishes the archive; an unchanged version only validates.

Before a phase or release closes, require self, task, and final review; focused and full validation; durable documentation uplift; accounting; and the [release-decider rubric](docs/release/owner-approval-rubric.md). The maintainability review checks requirement traceability, focused tests, proportionate module/dependency/test growth, current consumers for abstractions, and explicit bounded follow-ups. Stop for owner review under the program-stop conditions in [ADR 0004](docs/adr/0004-preserve-the-python-prototype-and-port-to-rust.md).

## Documentation placement

| Document | Canonical content |
| --- | --- |
| `README.md` | Entry point, capability summary, and links |
| `USERS.md` | Installation, runtime behavior, privacy, and troubleshooting |
| `DEVELOPERS.md` | Architecture, tooling, tests, release process, and these rules |
| `TODO.md` | Actionable future work only |

| Change | Update |
| --- | --- |
| CLI flag, environment variable, or operator-visible error | `USERS.md` (and this guide when developer-impacting) |
| User workflow or major capability | `README.md` and `USERS.md`, plus this guide for implementation/release impact |
| Internal refactor or developer tooling/tests | This guide |
| Durable decision | `docs/adr/` and an appropriate link |
| Deferred work or design question | `TODO.md` |
