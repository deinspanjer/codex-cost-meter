## Why

Human report output currently leaves stale progress text and presents noisy, occasionally misaligned tier detail and awkward empty or wrapped states. Asciinema recordings across report modes make these inconsistencies reproducible and show that they obscure otherwise-correct analysis and pricing results.

## What Changes

- Make terminal progress replacement erase the prior line before drawing the next message, so transitions from indexing to analysis cannot leave ` 0 files` or `files` behind; use grammatically correct singular/plural file counts and avoid broad screen, scroll-region, or cursor resets.
- Simplify human service-tier detail by removing the lightning-bolt glyph, combining explicit and assumed Standard usage into a `Standard*` presentation mode, and explaining `*` once as usage that includes assumed Standard attribution. Preserve the separate `standard` and `assumed_standard` fields in JSON.
- Derive the distinct human modes for each model after that Standard merge. When exactly one mode remains, annotate the aggregate model row as `Standard`, `Standard*`, `Fast`, or unavailable and omit all child rows. When multiple modes remain, retain the aggregate row and render concise child rows for each mode so Standard/Fast and unavailable-tier mixtures stay visible.
- Clean up table alignment and long pricing-provenance wrapping at practical terminal widths without changing numeric values or machine-readable output.
- Give empty reports an intentional model-section state instead of a full `Models` table containing only `Total`.
- Add cast-backed terminal verification for named-project, current-project, and date-grouped corpus modes.

### Recorded runtime inconsistencies

- A 160x36 project cast writes `\rIndexing rollout metadata: 0 files` followed by the shorter `\rAnalyzing 0/1977 rollouts`. No erase-line sequence follows, so the prior ` 0 files` suffix remains visible. The same transition occurs in the grouped-corpus and empty current-project casts.
- Progress renders `1 files` rather than `1 file`.
- The lightning bolt requires a one-character display-width exception and tier labels create visibly uneven, repetitive model sections.
- Long pricing basis and source lines hard-wrap mid-sentence or mid-URL at 120 and 160 columns without a continuation indent.
- A zero-result project prints a `Models` heading, table header, separator, and `Total` row despite having no model data.
- The casts contain no clear-screen, clear-to-end, scroll-region, cursor-home, cursor-visibility, or alternate-screen ANSI sequences. The progress defect is incomplete carriage-return replacement, not teardown or scrolling behavior.

## Capabilities

### New Capabilities

- `terminal-report-presentation`: Stable terminal progress replacement and intentional, bounded human report layout across task, project, and corpus modes.

### Modified Capabilities

- `tier-aware-pricing`: Present structured service-tier detail concisely in human reports while retaining separate machine-readable tier keys.

## Impact

Human output and progress rendering in `src/output.rs` and `src/progress.rs`, focused output/progress tests, documentation examples, and asciinema-based validation are affected. Pricing calculations and provenance, rollout analysis, cache contents, JSON tier keys, and external pricing/catalog data do not change.
