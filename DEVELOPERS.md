# Developer guide

Start here for local development; use the linked guides when changing pricing, storage behavior, or releases.

## Build and test

Install Rust 1.97.1 through `rustup` and `just`, then run:

```text
cargo build --locked
just test-filter report::tests
just test-version
just check
```

Use `just test-filter <FILTER>` for the behavior you are changing or `cargo test --test <NAME>` for one CLI integration suite. `just check` runs formatting, Rust tests, version-tool tests, and warnings-denied Clippy. See the [test map](docs/architecture.md#tests) for coverage and the [packaging guide](docs/releases.md#packaging-and-platform-checks) for native builds and `just package`.

## Architecture

One Rust 2024 crate produces one `codex-cost-meter` binary. Rollout JSONL is authoritative for usage; SQLite supports discovery, caching, and title updates. Platform-specific scheduler modules handle the native lifecycle. The `python-prototype/` directory is historical reference.

| Area | Modules |
| --- | --- |
| Commands and presentation | `cli`, `output`, `progress` |
| Rollout discovery and accounting | `rollout`, `report`, `project`, `date_filter` |
| Cached usage and model prices | `cache`, `pricing` |
| Title updates and recovery | `title`, `update`, `session_index` |
| Native scheduling | `schedule`, `schedule::{linux,macos,windows}` |

See [architecture and tests](docs/architecture.md) for storage invariants, cache-version rules, scheduling behavior, and completion generation.

## Maintainer guides

| Task | Guide |
| --- | --- |
| Refresh model rates or historical pricing | [Updating model pricing](docs/model-pricing.md) |
| Package, validate Homebrew, or publish a release | [Packaging and releases](docs/releases.md) |
| Understand earlier design decisions | [Architecture decisions](docs/adr/README.md) |

## Documentation Placement Rules

| Document | Canonical content |
| --- | --- |
| `README.md` | Entry point, capability summary, and links |
| `USERS.md` | Installation, runtime behavior, privacy, and troubleshooting |
| `DEVELOPERS.md` | Development quick start, architecture map, guide index, and these rules |
| `docs/architecture.md` | Runtime and storage invariants, completion generation, and test coverage |
| `docs/model-pricing.md` | Pricing evidence standards and maintenance procedure |
| `docs/releases.md` | Packaging, Homebrew, publication, signing, and release gates |
| `TODO.md` | Actionable future work only |

## Change-Driven Update Matrix

| Change | Update |
| --- | --- |
| CLI flag, environment variable, or operator-visible error | `USERS.md` (and the relevant developer guide when developer-impacting) |
| User workflow or major capability | `README.md` and `USERS.md`, plus the relevant developer guide for implementation/release impact |
| Internal refactor or developer tooling/tests | The relevant developer guide; update this index when navigation changes |
| Durable decision | `docs/adr/` and an appropriate link |
| Deferred work or design question | `TODO.md` |
