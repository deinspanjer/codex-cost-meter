# Agent Guidelines

## Purpose

Preserve the existing Python Codex task-cost tools for reference while developing the primary implementation as a small, multi-architecture Rust application.

## Scope

- Treat `python-prototype/` as historical reference. Do not restructure it or extend it as the primary implementation.
- Port behavior and verified invariants, not the Python module layout.
- Prefer one Rust crate while the project remains small. Add crates only when distinct ownership or build boundaries clearly reduce complexity.
- Keep the executable run-once. LaunchAgent, Task Scheduler, packaging, and fleet deployment stay outside the application unless a concrete requirement says otherwise.
- Initial release targets are macOS arm64, macOS x86_64, and Windows x64. Keep platform-specific code narrow and explicit.

## Development

- Use Rust 2024 and stable Rust.
- Search before adding helpers, dependencies, or abstractions.
- Prefer the standard library and existing dependencies; add dependencies only when they materially reduce correct code.
- Preserve Codex storage compatibility deliberately, including rollout parsing, descendant accounting, title limits, SQLite updates, and `session_index.jsonl` updates.
- Keep public APIs small and use exhaustive matches where practical.
- Do not add compatibility shims, configuration, or packaging machinery without a current requirement.

## Verification

- Test the narrowest behavioral invariant that could regress; do not test static prose or constants.
- Exercise supported platform branches in CI when cross-platform build automation is added.
- Use repository-provided `just` recipes once they exist; run focused checks before broad suites.
- Do not call work complete without reporting the validation commands and their results.
