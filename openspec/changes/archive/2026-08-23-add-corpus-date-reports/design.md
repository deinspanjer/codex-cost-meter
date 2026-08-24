## Context

See [proposal.md](proposal.md). The current analysis retains timestamped usage events but only lifetime turn aggregates; the disposable cache serializes that analysis. Report and project aggregation already share model, pricing, completeness, and output behavior.

## Goals / Non-Goals

**Goals:**

- Reuse the existing discovery, price-at-event-time, partial-cost, and selection behavior.
- Give corpus and project reports one date/grouping contract and stable machine-readable output.
- Use correct host-local historical offsets, including DST transitions.

**Non-Goals:**

- Filter rollout types or models before aggregation.
- Add date filters to exact-thread reports, presets such as month-to-date, an explicit timezone flag, or multi-home reporting.
- Emit synthetic type/model combinations for empty buckets.

## Decisions

### Model the selected report as timestamped metric records

Extend cached rollout analysis with compact turn records carrying a start timestamp, optional duration, model, and rollout type. Retain existing timestamped usage events. A shared report accumulator will select records by the local date range, produce the overall aggregate, and then key requested grouped rows by time plus optional dimensions.

This preserves event-level effective-dated pricing and avoids filtering pre-aggregated lifetime totals. Filtering only token events was rejected because the report would mix selected cost with lifetime operational metrics.

### Use local calendar boundaries with a timezone database

Parse only ISO calendar dates. Resolve the host's IANA timezone with a small cross-platform dependency, then convert local midnights for each bound and bucket independently. Treat `--through` as the exclusive next local midnight internally; reject an end before the start. Weeks start Monday.

Using UTC or the current fixed offset was rejected because both conflict with the agreed local-calendar contract around daylight-saving transitions.

### Make grouping additive and sparse

Require one of `day`, `week`, or `month` in `--group-by`, with optional `rollout-type` and `model`; reject repeated or conflicting time dimensions. Grouped output retains the selected-range aggregate. Without `--include-empty`, create rows only for keys that have included metrics. With it, fill only missing time buckets and leave optional dimensions absent.
`--include-empty` requires both inclusive date bounds so expansion is finite.

The no-group output remains compact and keeps its existing type/model summaries. A full time × type × model zero matrix was rejected because it is both misleading and unbounded.

### Preserve filtered completeness

For a filtered range, records with no usable timestamp cannot truthfully be assigned to a bucket. Exclude them, set incomplete-input, and issue a sanitized warning. Lifetime reports retain current fallback and completeness behavior.

### Prune metadata-backed corpus candidates before rollout I/O

For `report --all`, read each indexed rollout path with its created and updated timestamps from Codex's thread metadata. Before opening a candidate rollout file, exclude it when its updated timestamp is earlier than the `--since` local-midnight instant or its created timestamp is at or after the exclusive local midnight following `--through`. Apply only the bounds that were supplied and use the same resolved local-calendar instants as metric filtering.

Treat metadata as a pruning hint rather than the complete rollout inventory. Keep unindexed rollouts and rows with missing or unusable timestamps, and fall back to ordinary discovery when the metadata database or required columns cannot be read. Retained rollouts still pass through event- and turn-level filtering, which remains authoritative for report contents.

Filtering only after rollout analysis was rejected because it still opens and parses the entire indexed store for a narrow corpus range. Treating metadata as exhaustive was rejected because state projection lag or schema gaps could hide rollout files that remain discoverable on disk.

### Evolve the disposable cache deliberately

Add the new turn records to the cached analysis representation and bump `ANALYSIS_VERSION`. Existing cache rows will be reanalyzed; no user data migration is needed. Keep cached values private and do not retain new forensic fields beyond data required to render reports.

## Risks / Trade-offs

- [Host timezone detection fails or is unavailable] → Return a clear report error rather than silently substituting UTC.
- [Large corpus or wide range] → Aggregate streaming cached records and keep `--include-empty` bounded by the explicit date range.
- [A turn crosses a date boundary] → Attribute its full duration to its start date, as specified.
- [Date-filtered omissions surprise users] → Mark incomplete input and render a sanitized explanation in human and JSON output.
- [Thread metadata is incomplete or unavailable] → Prune only candidates with usable timestamps and retain filesystem discovery as the correctness fallback.

## Migration Plan

1. Add the CLI contract and report data model behind focused tests.
2. Bump the analysis cache version so old rows are reanalyzed automatically.
3. Update user documentation and report examples; no stored-task mutation or rollback action is required.
