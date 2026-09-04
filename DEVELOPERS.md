# Developer guide

## v0.8 architecture

One Rust 2024 crate produces one `codex-cost-meter` binary. `cli` parses cross-platform `report`, `update`, and scheduling commands on the supported scheduler targets; `rollout::discovery` bounds and indexes JSONL without following directory symlinks; `rollout::analysis` attributes usage and preserves ambiguity; `cache` owns the disposable app SQLite cache; `pricing` embeds date-aware rates; `report` reuses one selected rollout/catalog context; `session_index` owns bounded snapshots and durable index appends; `title` owns pure metric parsing and bounded composition; `update` owns SQLite selection, the process lock, and the SQLite-then-JSONL recovery sequence; `schedule` owns bounded status, result transitions, and scheduled-run orchestration; `schedule::linux` owns the fixed current-user systemd service/timer lifecycle; `schedule::macos` owns the current-user LaunchAgent lifecycle; `schedule::windows` owns the fixed current-user Task Scheduler lifecycle and deferred self-delete; and `output` renders sanitized report output. Compile-time target gates select the native scheduler module without a cross-platform scheduler abstraction. Linux keeps its service and timer under XDG user configuration and its bounded status under XDG state; macOS and Windows retain their native paths. `data/model-prices.json` is the built-in catalog.

Exact and project reporting may use Codex's SQLite projection to select rollout paths, but rollout JSONL remains authoritative and an incompatible or incomplete projection falls back to file discovery. `codex-cost-meter.sqlite` stores versioned discovery and parsed-analysis JSON keyed by rollout path; analysis reuse requires matching nanosecond modification time and size, and cache failures disable caching for the rest of that command. Pricing and report aggregation are deliberately not cached. Title updates additionally read the supported `state_5.sqlite` `threads` contract documented in the user guide. `rusqlite` uses its bundled SQLite build so the Universal 2 executable has no separately installed SQLite dependency; the standard-library file lock avoids another locking dependency. The scheduler writes one bounded, synchronized replacement status record with allowlisted remediation only; it pauses after three ordinary failures or immediately for disk-full, schema, and permission failures. macOS and Windows lifecycle modules use fixed native tool paths and private fake-runner seams, so tests cover command plans without registering a real job. Windows writes Task Scheduler XML only as a synchronized temporary file, queries the fixed task with HRESULT status, and keeps scheduler state under `LOCALAPPDATA`; its native cleanup script receives the executable as an argument rather than interpolating it. The [`python-prototype/`](python-prototype/) directory is historical reference only, not the active architecture.

Bump `DISCOVERY_VERSION` when session-metadata interpretation or cached linkage changes, and bump `ANALYSIS_VERSION` when parsed `RolloutStats` semantics change. Pricing or report-only changes need neither bump because those calculations remain live.

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
cargo test schedule::linux::tests
cargo test --test schedule_cli
cargo test --test windows_schedule_cli
cargo test --test linux_schedule_cli
cargo test --test update_cli
just check
just package
```

`just check` runs formatting, tests, version-tool tests, and warnings-denied Clippy. `just package` builds both macOS slices, creates a Universal 2 binary, verifies its architectures and ad-hoc signature, and writes a deterministic archive plus checksum under `target/release/`. Native Windows CI runs the corresponding checks and release build on `x86_64-pc-windows-msvc`, then creates and inspects the deterministic Windows ZIP and checksum. Native Linux CI runs the Linux lifecycle fake-runner and CLI contracts, builds static musl binaries for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, then uses `python3 scripts/version.py package-linux --architecture x86_64|aarch64` to create and inspect each deterministic archive and checksum. It deliberately does not install a real systemd unit: hosted runners can lack a user bus, while the fake-runner tests cover the exact lifecycle command plans. Before release, inspect all archive member lists and checksums, the Universal 2 architectures and strict ad-hoc signature, and the Windows x64 fixture.

The source-building Homebrew formula lives at `Formula/codex-cost-meter.rb`. It pins a tagged source archive and uses Homebrew's locked Cargo install arguments. Validate it from a temporary or installed tap with `brew style <TAP>`, `brew audit --strict <TAP>/codex-cost-meter`, `brew install --build-from-source <TAP>/codex-cost-meter`, `brew test <TAP>/codex-cost-meter`, and `brew uninstall codex-cost-meter`. Homebrew rejects formula paths outside a tap. Do not install a real schedule during the formula test.

Unit tests live beside the behavior they protect; `tests/report_cli.rs` covers reporting dispatch, home resolution, output, errors, and hardening, while `tests/update_cli.rs`, `tests/schedule_cli.rs`, `tests/linux_schedule_cli.rs`, and `tests/windows_schedule_cli.rs` cover update and native scheduling command contracts. Title, session-index, update, and scheduling tests use temporary homes and cover metric boundaries, root-only selection, dry-run immutability, dual-store apply/recovery, schema compatibility, lock/error handling, circuit-breaker transitions, bounded status, Linux systemd unit/command plans, Windows argument quoting, task lifecycle, and deferred cleanup. Keep tests focused on behavioral invariants, parallel-safe where possible, and input handling non-panicking and bounded.

## Release and phase gates

`Cargo.toml` is the version source. Immediately before `just bump`, keep `[Unreleased]` concise and nonempty; the command creates a new empty `[Unreleased]` while rotating its entry into a date-free release heading. `just bump major`, `minor`, or `patch` calculates the next SemVer, while an exact selector such as `just bump 1.0.0` supports an intentional major boundary. A version-changing protected-branch merge validates locked native macOS, Windows, and Linux builds, packages and checksums one macOS archive, one Windows archive, and two Linux musl archives, Developer ID-signs and notarizes the Universal 2 binary, creates GitHub provenance attestations for all eight assets, tags the merge, and publishes exactly eight assets; an unchanged version only validates.

After a release is public, the release workflow uses a repository-scoped GitHub App to calculate the tagged source SHA-256 and open a normal formula PR from an `automation/homebrew-v<VERSION>` branch. It never writes directly to protected `main` or enables auto-merge; repeat the formula validation above before merging. The App must be installed only on this repository with `Contents: read and write` and `Pull requests: read and write`. Run `scripts/setup-homebrew-pr-app.sh` to create or rotate its credentials: the Client ID is repository variable `HOMEBREW_PR_APP_CLIENT_ID`, while the private key is stored as both a 1Password API Credential and repository secret `HOMEBREW_PR_APP_PRIVATE_KEY`. Verify an attested asset with `gh attestation verify <ASSET> --repo deinspanjer/codex-cost-meter`. The full distribution decision and clean-machine matrix are in [ADR 0008](docs/adr/0008-use-a-source-built-homebrew-tap-and-attested-release-assets.md).

The macOS publish job imports the Developer ID identity into an ephemeral keychain, signs only after creating the Universal 2 binary, requires Apple notarization acceptance, and deletes temporary signing material even when a later step fails. It requires repository secrets `MACOS_CERTIFICATE_P12_BASE64`, `MACOS_CERTIFICATE_PASSWORD`, `APPLE_NOTARY_APPLE_ID`, and `APPLE_NOTARY_PASSWORD`, plus repository variable `APPLE_TEAM_ID`. Keep the recoverable `.p12` and its distinct export password in 1Password; never commit signing material.

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
