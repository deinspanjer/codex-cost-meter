## Context

See `proposal.md`, `specs/terminal-report-presentation/spec.md`, and `specs/tier-aware-pricing/spec.md` for motivation and observable behavior.

Interactive progress currently writes `\r` followed by the next message. The indexing message is longer than the initial analysis message, so the suffix remains on screen. Asciinema 3.2.1 casts at 120x32 and 160x36 confirm the exact transition and contain no erase-line, clear-screen, scroll-region, cursor-home, cursor-visibility, or alternate-screen sequences.

Model service-tier rows are produced directly from the structured `standard`, `assumed_standard`, `fast`, and unavailable entries, including a special display-width exception for `⚡`.

## Goals / Non-Goals

**Goals:**

- Make interactive progress replacement deterministic without clearing unrelated terminal content.
- Minimize service-mode rows while preserving every distinction needed to interpret cost completeness.
- Keep JSON compatibility and exact tier attribution.
- Make empty and narrow-terminal output deliberate and testable.

**Non-Goals:**

- Changing pricing, token attribution, report selection, cache contents, or JSON tier keys.
- Building a responsive terminal UI or adding a terminal-layout dependency.
- Reformatting machine-readable JSON or changing title output.
- Changing effective-dated model-proxy pricing or provenance.

## Decisions

### Clear only the active progress line

For terminal progress, return to column zero, erase the current line, and write the new message. Keep redirected forced progress newline-delimited and keep the final terminal newline. Use targeted erase-line control rather than screen clearing, scroll regions, cursor positioning, or padding based on the previous message length.

Track the ordinary file-count noun so `1 file` and all other counts use `files`. Padding was rejected because it depends on remembered display width and still leaves cursor placement ambiguous. Full-screen clearing was rejected because progress owns only its current line.

### Bound long human provenance fields

Keep structured pricing metadata unchanged. Render each comma-separated pricing source on its own indented line and word-wrap long prose fields to a fixed bounded content width suitable for the recorded 120- and 160-column terminals, with continuation indentation. This avoids a terminal-size dependency while keeping unbreakable values intact.

### Project structured tiers into fewer human modes

Keep structured tier maps untouched. Before rendering, project them into human modes:

- Sum explicit `standard` and `assumed_standard` values into one Standard mode.
- Mark it `Standard*` when assumed usage contributes any tokens or known cost.
- Render `Fast` as plain text.
- Preserve each unavailable tier as its own mode.

If projection produces one mode, append that mode to the aggregate model label in brackets and emit no child rows, for example `gpt-5.5 [Standard*]` or `gpt-5.6-sol [Fast]`. If it produces multiple modes, keep the aggregate label unchanged and emit one child row per projected mode. The report-level assumption sentence becomes the asterisk explanation and retains the assumed-token total.

A separate service-mode column was rejected because it makes already-wide tables wider. Omitting all tier detail was rejected because mixed Fast and unavailable usage materially affects interpretation. Combining the structured JSON tiers was rejected because consumers may rely on the evidence distinction.

### Remove the glyph-specific width rule

Once `⚡` is removed, table width uses the existing character-count behavior for the remaining report character set. Focused tests compare visible column starts for aggregate and child rows. Adding a Unicode-width dependency is deferred unless casts prove another supported glyph has a different terminal width.

### Render an explicit empty Models state

When the model map is empty, render the Models heading followed by `No model usage.` and do not construct a model table. Lifetime totals remain visible in the scope table.

## Risks / Trade-offs

- [A fixed prose wrap width cannot fit every terminal] → Target the recorded practical widths, preserve words and URLs, and avoid claiming responsive layout.
- [Collapsing Standard evidence removes its split from human rows] → Preserve the exact split in JSON and mark any combined assumed usage with `*` plus its token count.
- [Single-mode annotation lengthens a model label] → Bracketed text is shorter than a child row and avoids adding a table column.
- [Targeted ANSI erase may be visible in captured raw output] → Emit it only when output is already identified as a terminal; redirected progress remains plain lines.

## Migration Plan

1. Correct progress rendering and verify the transition in a focused test and replacement cast.
2. Add the human-mode projection, single-mode collapse, bounded provenance fields, and empty-model state while retaining JSON structure.
3. Update documentation examples and record the same named-project, current-project, and grouped-corpus modes at 120 and 160 columns.

Rollback restores the prior human renderer and progress line behavior. No data or cache migration is required.
