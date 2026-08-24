## Why

Current reports can describe one task or a project over its full lifetime, but cannot answer a date-bounded question across all locally stored rollouts. The ccusage comparison identified date-aware corpus reporting as the useful gap while preserving this tool's stricter completeness and pricing guarantees.

## What Changes

- Add `report --all` to aggregate every discovered rollout once, independent of thread or project assignment.
- Add inclusive local-calendar `--since` and `--through` bounds plus composable `--group-by` dimensions for day, week, month, rollout type, and model.
- Use available Codex thread metadata to exclude clearly out-of-range `--all` rollout candidates before opening and analyzing their rollout files.
- Apply date bounds consistently to usage, cost, turns, and duration; expose incomplete filtered input rather than silently placing untimestamped data in a range.
- Extend project reports with the same date and grouping options.

## Capabilities

### New Capabilities

- `rollout-date-reporting`: Date-bounded corpus and project reporting with explicit aggregation dimensions and completeness semantics.

### Modified Capabilities

- None.

## Impact

- `src/cli.rs`, `src/main.rs`, `src/report.rs`, `src/project.rs`, `src/output.rs`, rollout discovery/analysis/cache code, and read-only access to Codex thread timestamps.
- A small timezone dependency is needed for correct host-local calendar boundaries across DST.
- Report CLI, report rendering, and cached-analysis tests; user documentation and the ccusage comparison follow-up.
