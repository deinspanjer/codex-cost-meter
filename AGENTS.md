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

## Program continuation

- The owner has authorized autonomous just-in-time design, planning, and execution for roadmap milestones after v0.1 under `docs/adr/0004-preserve-the-python-prototype-and-port-to-rust.md`.
- Do not add a manual owner gate merely because an agent workflow normally requests design or plan approval when the work remains within that standing authorization.
- Stop for owner review only under ADR 0004's program-stop conditions or when external authority is required. Keep dependent work stopped after an unresolved shared-foundation rejection; isolated branch investigation may continue only when it cannot compound the finding.
- Stop all autonomous work for owner review if cumulative output usage for the program's root task and descendants exceeds 1,000,000,000 tokens. This threshold includes governance overhead and cannot be bypassed with isolated branch work.
- Self-review, task review, final review, validation, documentation uplift, accounting, release-decider review, and removal of mechanical specifications and plans remain mandatory.

Future temporary Superpowers specifications, plans, SDD evidence, and accounting scratch live only under ignored `.superpowers/` paths and never under `docs/superpowers/`.
