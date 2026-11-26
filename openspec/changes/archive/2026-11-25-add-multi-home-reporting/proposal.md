# Multi-home reporting

## Why

People who run Codex under more than one storage directory (separate account homes, a copied or archived home, another machine's home) cannot get one report across them today: `report` reads exactly one Codex home. Summing separate per-home reports double-counts rollouts copied between homes and misses cross-home fork relationships, so the sum is wrong in both directions.

## What Changes

- Add `report --codex-homes HOME[,HOME...]`, which combines every listed home into one read-only report. It works with every report mode: exact task, `--match-title`, `--project`, `--all`, date filters and grouping, `--by-thread`, `--json`, and `--jsonl`.
- Count one rollout ID as one rollout across all listed homes. When copies of the same rollout exist in several homes, select one with the existing newest-file rule. The result does not depend on the order of the listed homes.
- Resolve parent and child relationships across homes. A child in one home and its parent in another form one task tree, and legacy fork-replay suppression applies when the parent is in any listed home.
- Resolve `--project` across homes. Desktop Projects in different homes that have the same name are the same project. Each home classifies only the root rollouts it holds, and a root counts once.
- Reject an empty home entry, and reject a listed home that does not exist or is not a directory, before reading any rollout. An existing home that cannot be read completely keeps the existing behavior: the report is marked incomplete and includes a warning.
- Fix two single-home bugs exposed by this work. An exact report on a fork child now uses the child's parent to suppress the copied history; before, it charged that history to the child. A copied home whose metadata points into the original home now reads only its own files.
- Keep `report --codex-home PATH` with its current single-path meaning, and make it conflict with `--codex-homes`. Keep `CODEX_HOME`, `update`, and `schedule` limited to a single home, without changes.
- Release as an additive minor version (1.10.0). This differs from the original handoff, which proposed replacing the report flag before 1.0. The project is now at 1.9.0 and distributed through Homebrew, so removing the flag would break existing users.

## Capabilities

### New Capabilities

- `multi-home-reporting`: selecting several Codex homes for one read-only report, deduplicating rollouts across homes, resolving task trees and projects across homes, the semantics for a missing or unreadable home, and keeping mutating commands limited to one home.

### Modified Capabilities

None. Existing report, date-range, fork-replay, and cache requirements apply unchanged to the combined set of homes.

## Impact

- CLI: `ReportArgs` in `src/cli.rs` gets the new option and a home-set resolver. `UpdateArgs` and `ScheduledUpdateArgs` do not change.
- Discovery: `src/rollout/discovery.rs` gets multi-root scanning, targeted and corpus path selection for each home, and a record of which homes hold each rollout ID. `resolve_duplicates` does not change.
- Report context: `src/report.rs` and `src/usage_export.rs` use names from each home, an analysis cache for each home chosen by a record's source home, and an error message that lists every home.
- Projects: `src/project.rs` merges Desktop state across homes by project name and classifies roots in each home.
- Entry point: in `src/main.rs`, the report branch builds a home set and writes cache notices for each home.
- Docs: `USERS.md`, `CHANGELOG.md`, and CLI help. The handoff's step 5 and decision 6 are marked superseded by this change.
- No new dependencies. No change to the JSON or JSONL schemas.
