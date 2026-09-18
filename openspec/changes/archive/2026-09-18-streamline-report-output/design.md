## Context

See `proposal.md` for motivation and the delta specs for observable behavior. Human exact-rollout and aggregate reports are assembled in `src/output.rs` from `Report` and `ProjectReport`; those structures already contain the counts, token components, turn durations, model/tier breakdowns, pricing date, completeness signals, and selection diagnostics needed for the new presentation. The current majority model/reasoning fields are selected by turn frequency. `src/title.rs` already recognizes and removes canonical meter-generated title suffix segments while composing updated titles.

The formatter currently uses one wide stats-table shape for scope, model, rollout-type, and group rows, then appends full pricing provenance and static notes. Progress owns one terminal line but deliberately leaves a completed analysis line before output.

## Goals / Non-Goals

**Goals:**

- Derive a compact human projection from existing structured data without changing accounting or serialization.
- Make ordinary small reports short while keeping exceptional tier, completeness, and partial-cost evidence visible.
- Use labels that describe the actual aggregation: turn-frequency model/effort and summed recorded turn durations.
- Keep exact, project, corpus, grouped, and non-root output visually related without forcing root-only concepts into aggregate reports.

**Non-Goals:**

- Adding report verbosity flags, terminal-width detection, or a responsive terminal UI.
- Changing JSON fields, cache schemas, rollout parsing, pricing calculations, or title-update behavior.
- Inferring user-input latency, wall-clock session span, idle gaps, or disjoint activity ranges; that investigation remains in `TODO.md`.
- Showing the full project path, rollout identifier, selection diagnostics, pricing sources, or proxy histories in default human output when JSON already preserves them.

## Decisions

### Project existing report data into two table shapes

Keep the current structured reports unchanged and build presentation rows at render time. Scope, rollout-type, and group tables use the compact nested-metric shape:

`label | turns | input (cached) | output (reasoning) | turn time | estimated cost`

Model tables use:

`model | turns | input | output | turn time | estimated cost`

This preserves the useful aggregate components while avoiding separate cache and reasoning columns in every model row. Reusing one wide table everywhere was rejected because it gives low-value components the same visual weight as totals and causes most of the horizontal clutter.

### Make duplication predicates explicit

Render All agents only when the exact report includes descendants. Render a model section only when there is more than one model or tier evidence must remain visible, and never append a model Total row. An empty aggregate renders one empty-state sentence and stops before usage and pricing sections.

The single-model omission predicate must consider service modes before hiding the section: Fast-only, mixed Standard/Fast, and unavailable-tier evidence remain visible. Assumed Standard alone may omit the table because its report-level warning preserves the material qualification.

Always rendering structurally identical sections was rejected because the pressure-test corpus showed the smallest and largest isolated roots commonly contain one rollout and one model, making three copies of the same values.

### Reuse canonical title parsing and existing majority selection

Expose the existing canonical suffix stripping operation from `src/title.rs` for report display rather than implementing another metric parser. Human identity uses the first eight identifier characters and the final project path component, with safe fallbacks for short identifiers and unavailable project/name fields. JSON retains the original values.

Label the existing root `majority_turn_model` and `majority_reasoning_level` pair as `Most root turns`; for a selected non-root rollout use `Most turns`. Do not recompute the pair by cost, duration, or tokens. Aggregate project and corpus reports omit it because one pair across unrelated roots is not useful context.

A generic `Primary` label was rejected because it does not disclose the selection measure. Adding per-effort report structures was rejected because the agreed presentation needs only the already-computed most-frequent pair.

### Preserve duration calculations and correct only their presentation

Root turn time remains the sum of recorded lifecycle start-to-complete-or-abort intervals in the selected root rollout. All agents remains the sum across root and descendant rollouts, even when those intervals overlap. Render the overlap note only when an All agents row is present. Project, corpus, rollout-type, group, and model rows use the neutral `Turn time` heading.

Do not call these values wall-clock session duration or user-wait latency: the analyzer does not identify user initiation, and automatic continuations are separate only when Codex records a separate lifecycle interval.

### Split dynamic report qualifications from static help

Default human footers retain only:

- the API-list estimate label and catalog date;
- a nonzero assumed-Standard token warning;
- partial-cost meaning when `+` is displayed;
- nonzero incomplete or selection qualifications;
- exceptional tier detail that affects interpretation.

Move stable explanations of nested columns, overlap, applied-versus-served tier limits, proxy estimation, sources, and JSON provenance to the report command's long help. Keep exact sources, proxy histories, and tier maps in JSON. Help describes the method rather than duplicating catalog-effective dates that can change independently.

Full provenance in every report was rejected because it dominated small output and gave a few cents of proxied Auto-review usage more space than the entire result.

### Clear only the owned interactive progress line

When terminal progress completes, erase its current line without emitting a completed analysis line before the report. Preserve newline-delimited forced progress for redirected stderr. Do not clear the screen or other terminal content.

## Risks / Trade-offs

- [Canonical-looking user text could be mistaken for a generated title suffix] → Reuse the existing strict canonical metric recognizer and strip only trailing recognized segments.
- [Conditional sections can hide exceptional pricing context] → Centralize and test the model-section predicate against ordinary Standard, assumed Standard, Fast-only, mixed, and unavailable tiers.
- [Nested values can still create wide rows for long group or model labels] → Keep compact numeric notation, retain the existing bounded target widths, and verify representative 120- and 160-column casts without adding terminal-width dependencies.
- [Static help can drift from estimator behavior] → Describe durable semantics in help and point exact dates, sources, mappings, and fields to JSON rather than duplicating mutable catalog content.
- [Project task count could disagree with selection diagnostics] → Derive the displayed task count from included root rollout totals, while JSON remains authoritative for resolver-specific assignment and exclusion counts.

## Migration Plan

1. Add focused formatting helpers and reuse canonical title stripping without changing report structures.
2. Replace exact and aggregate human layouts, then simplify pricing and tier footers.
3. Expand report long help and clear interactive progress at handoff to final output.
4. Update documentation examples and verify representative small, medium, large, isolated, clustered, project, corpus, grouped, empty, incomplete, mixed-tier, and non-root reports.

Rollback restores the prior human renderer, help text, and final progress newline. No data or cache migration is required.
