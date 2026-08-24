## 1. Date-aware report inputs

- [x] 1.1 Add the minimal host-timezone dependency and local-calendar range/grouping parsers for `--since`, `--through`, `--group-by`, and `--include-empty`; verify parser tests reject malformed, inverted, duplicate, conflicting, and unbounded-empty dimensions and cover a DST boundary.
- [x] 1.2 Add mutually exclusive `report --all` selection and expose date/grouping arguments only for corpus and project reports; verify CLI integration tests preserve bare, exact-thread, and project selection behavior.
- [x] 1.3 Use available Codex thread created/updated timestamps to prune indexed `--all` rollout paths before file discovery or analysis; verify focused tests cover each bound independently, inclusive boundary behavior, combined bounds, metadata gaps, and prove excluded rollout files are not opened.

## 2. Timestamped metrics and aggregation

- [x] 2.1 Preserve compact timestamped turn records in rollout analysis and its cache, bump `ANALYSIS_VERSION`, and verify cached and uncached analysis produce identical date-scoped turn and duration results.
- [x] 2.2 Build the shared range-aware accumulator for usage, prices, turns, duration, model, and rollout type; verify root and non-root corpus rollouts each count once and a cross-boundary turn is attributed to its start day.
- [x] 2.3 Propagate unknown timestamps as excluded incomplete input only for filtered reports; verify unfiltered lifetime reports preserve current totals and filtered JSON/human output gives a sanitized qualification.

## 3. Grouped report outputs

- [x] 3.1 Add overall plus composable day/week/month, rollout-type, and model grouped JSON report output; verify Monday-start weeks, local month/day boundaries, and aggregate reconciliation to grouped nonempty rows.
- [x] 3.2 Render grouped human output and `--include-empty` rows; verify default sparse output, zero-valued time-only empty rows, and no synthetic model/type values.
- [x] 3.3 Apply the same filtering and grouping pipeline to project reports; verify project selection accounting remains correct while selected metrics and buckets observe the date range.

## 4. Documentation and validation

- [x] 4.1 Update `USERS.md`, `README.md`, and the ccusage comparison follow-up with the new commands, local-calendar semantics, incompleteness behavior, and exclusions; verify examples match `--help` and JSON output.
- [x] 4.2 Run formatting, focused analysis/report/output/CLI tests, `just check`, and `openspec validate add-corpus-date-reports --strict`; record the commands and results in the implementation handoff.
- [x] 4.3 Document metadata-backed `--all` candidate pruning and its fallback behavior, then rerun focused report CLI tests, `just check`, and strict OpenSpec validation.
