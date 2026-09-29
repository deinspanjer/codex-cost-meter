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

## Updating model pricing

Pricing is an offline, reproducible estimate of API token list cost, not a Codex subscription bill. The executable embeds `data/model-prices.json`; editing that file affects newly built binaries. Reports recompute prices from cached usage facts, so a catalog-only update needs no cache-version bump or cache deletion.

### Evidence and historical meaning

Use official [API pricing](https://developers.openai.com/api/docs/pricing), the exact model's documentation, the [API changelog](https://developers.openai.com/api/docs/changelog), dated [OpenAI launch announcements](https://openai.com/news/), archived copies of those official pages, and the [Fast-mode guide](https://developers.openai.com/api/docs/guides/fast-mode). Previous sessions and research notes explain decisions and point to sources; reopen those sources before treating them as current evidence. Third-party catalogs and search snippets are discovery aids, not pricing authority.

Rates are USD per million tokens. Each dated price point holds uncached input, cached input, cache-write input, and output rates, with separate Standard/Fast and short/long-context cells. Gross input determines the context band; the chosen band prices the whole request. Cache reads and writes are subtracted from gross input before charging uncached input. Reasoning is already included in output. Never infer a global Fast multiplier or cache discount from another model.

`as_of` records when the catalog was reviewed; `effective_from` records when a rate applies. Preserve older points when prices change. Correct an erroneous historical point only with evidence supporting the correction, documenting the previous assumption. For a new model, a dated launch announcement plus matching model pricing can support a launch-date estimator boundary; record that basis. Before falling back to an observation date, search dated launch announcements, the API changelog, and archived captures of the official pricing/model pages around launch and suspected changes. Retrieve the archived content, not just the archive index. Distinguish the announcement date, snapshot identifier date, API availability date, capture date, and any explicit rate-change date. A near-launch capture can corroborate a launch-date estimator boundary; disclose that inference rather than claiming an exact billing cutover. Only if this historical search leaves the start unsupported should an older model's newly discovered rate use the observation date, with the search and remaining gap recorded. Do not invent future price changes from a promotion's minimum duration. Dates are day-granular estimator boundaries, not proof of an account's exact billing cutover.

A missing rate is unknown, not zero. Use `null`/absent optional cells for unavailable components or tiers; retain known cost and incomplete-result behavior. Keep exact model IDs and explicitly evidenced proxy histories. A successor model is not an alias for its predecessor. See the [Auto-review evidence](docs/research/codex-auto-review-pricing-evidence.md) and [Fast attribution evidence](docs/research/fast-mode-attribution-evidence.md) before changing those boundaries. Requested/applied tier telemetry does not establish the actually served tier.

### Procedure for maintainers and agents

1. **Establish scope.** Inspect the working tree, catalog, pricing implementation/tests, and previous refresh evidence. Review every existing model's latest cells and new relevant Codex models; do not expand into unrelated image, audio, tool-call, or subscription billing. Record models retained without fresh verification rather than claiming a full audit.
2. **Fetch evidence before editing rates.** Open current official pages, including the changelog since the last review. Record retrieval date, exact model ID, tier, context threshold, all four token rates, effective-date evidence, and source URLs in a concise `docs/research/` refresh note. Inspect labeled Standard and Fast tables: flattened pages can mix Batch/Flex prices or omit collapsed rows. Use the official `.md` page or browser table when extraction loses labels. A row missing from the headline table is not proof of removal or a price change: check collapsed rows and the exact older model page. Resolve disagreements against exact model pages and dated announcements; leave genuinely unresolved cells unchanged/unknown and explain them.
3. **Edit the catalog.** Add explicit rate cells and increasing history dates; update `as_of` and source provenance. Preserve older models and proxies. Thresholds currently live per model, outside its history: changing a threshold affects every date. If a threshold itself changed historically, the schema needs a separately reviewed correction; do not silently rewrite history. When adding a previously missing threshold, verify that dates without long-context cells become incomplete rather than retaining a falsely complete short-context estimate.
4. **Verify behavior.** Add or extend the smallest pricing test covering the affected date boundary, threshold and threshold-plus-one, Standard/Fast, and independently calculated component costs. Include distinct cache-read rates for similarly named models. Preserve coverage for unknown rates and historical/proxy lookup. Run `just test-filter pricing::tests`, then `just check`, and `git diff --check`. Inspect a synthetic CLI report if output/provenance changes require it. Do not add tests for static documentation wording.
5. **Explain the result.** Update the refresh note with changes, unchanged/unverified coverage, unresolved evidence, and validation outcomes. Update `USERS.md` and `[Unreleased]` in `CHANGELOG.md`; link to the evidence instead of duplicating large rate tables in user guidance. Review the final diff for accidental historical repricing and unsupported assumptions, explicitly checking that new thresholds cannot silently apply short-context rates to historical long-context usage, then revise this procedure if executing it revealed a material gap.
6. **Deliver within the requested scope.** Build the updated executable (`cargo build --locked --release`). A requested release additionally follows the release/package gates below. Publishing, installing, or rewriting existing titles is a separate action unless included in the request. For an authorized title repricing campaign, use the new binary and one fixed `--reprice-before` cutoff across bounded batches, as described in [ADR 0003](docs/adr/0003-use-session-index-updated-at-as-the-high-water-mark.md); do not move the cutoff between batches.

## Release and phase gates

`Cargo.toml` is the version source. Immediately before `just bump`, keep `[Unreleased]` concise and nonempty; the command creates a new empty `[Unreleased]` while rotating its entry into a date-free release heading. `just bump major`, `minor`, or `patch` calculates the next SemVer, while an exact selector such as `just bump 1.0.0` supports an intentional major boundary. A version-changing protected-branch merge validates locked native macOS, Windows, and Linux builds, packages and checksums one macOS archive, one Windows archive, and two Linux musl archives, Developer ID-signs and notarizes the Universal 2 binary, creates GitHub provenance attestations for all eight assets, tags the merge, and publishes exactly eight assets; an unchanged version only validates.

After a release is public, the release workflow uses a repository-scoped GitHub App to calculate the tagged source SHA-256 and open a normal formula PR from an `automation/homebrew-v<VERSION>` branch. It never writes directly to protected `main` or enables auto-merge; repeat the formula validation above before merging. A release task is not complete until the GitHub release is public and this generated formula PR has passed CI and been merged. When asked to publish or ensure a release, own that full sequence unless the user limits its scope; update a local installation only when explicitly requested. The App must be installed only on this repository with `Contents: read and write` and `Pull requests: read and write`. Run `scripts/setup-homebrew-pr-app.sh` to create or rotate its credentials: the Client ID is repository variable `HOMEBREW_PR_APP_CLIENT_ID`, while the private key is stored as both a 1Password API Credential and repository secret `HOMEBREW_PR_APP_PRIVATE_KEY`. Verify an attested asset with `gh attestation verify <ASSET> --repo deinspanjer/codex-cost-meter`. The full distribution decision and clean-machine matrix are in [ADR 0008](docs/adr/0008-use-a-source-built-homebrew-tap-and-attested-release-assets.md).

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
