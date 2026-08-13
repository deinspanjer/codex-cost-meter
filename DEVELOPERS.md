# Developer guide

## Status

The active Rust implementation has not been scaffolded. The existing behavior is preserved under [`python-prototype/`](python-prototype/) as a reference, not as the module structure the port must copy.

See the [Python prototype developer guide](python-prototype/DEVELOPERS.md) for its architecture, tests, and implementation lessons.

## Rust direction

Start with one Rust 2024 crate and one run-once executable. Split crates only when a demonstrated ownership, reuse, or build boundary makes the project simpler.

Initial targets are:

- macOS arm64
- macOS x86_64
- Windows x64

The operating system owns scheduling. LaunchAgent/Jamf on macOS and Task Scheduler/Intune or MECM on Windows are packaging concerns, not daemon behavior to embed in the executable.

Keep platform-specific code limited to storage locations, file locking, and filesystem replacement semantics. Rollout parsing, pricing, selection, high-water tracking, and title formatting should remain shared.

## Compatibility decisions

The port begins with the behavior recorded in the [architectural decision index](docs/adr/README.md):

1. Read task metadata from SQLite and usage from rollout JSONL.
2. Persist title changes to both SQLite and `session_index.jsonl`.
3. Use session-index `updated_at` as the update and repricing high-water mark.
4. Preserve the prototype and move the active implementation to Rust.

These are internal Codex storage formats, not a supported extension API. Verify behavior against the installed Codex version before changing the compatibility contract.

## Development workflow

Build, test, lint, and release commands will be added when the Rust crate exists. Until then, the only executable checks belong to the [Python prototype](python-prototype/DEVELOPERS.md#verification).

## Documentation placement rules

- `README.md` is the short entry point: purpose, status, capabilities, and links.
- `USERS.md` contains Rust operator workflows, commands, safety, and troubleshooting.
- `DEVELOPERS.md` contains Rust architecture, setup, implementation standards, verification, and contributor guidance.
- `TODO.md` contains future work only; completed behavior belongs in the other documents.
- Python-specific guidance belongs under `python-prototype/` and is linked from the corresponding root audience document.
- Focused decisions live under `docs/adr/` and are linked from the audience document that needs them.
- `AGENTS.md` contains agent instructions, not project documentation.

### Change matrix

| Change | Update |
| --- | --- |
| User-visible behavior or commands | `USERS.md` |
| Architecture, storage contract, or contributor workflow | `DEVELOPERS.md` |
| Project purpose, headline status, or navigation | `README.md` |
| New or removed future work | `TODO.md` |
| Durable decision with meaningful alternatives or consequences | `docs/adr/` and a link from `DEVELOPERS.md` |
| Python prototype behavior | Corresponding document under `python-prototype/` |
| Agent-only workflow instruction | `AGENTS.md` |
