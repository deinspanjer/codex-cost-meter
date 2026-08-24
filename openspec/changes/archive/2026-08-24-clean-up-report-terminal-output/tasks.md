## 1. Stabilize Terminal Progress

- [x] 1.1 Add focused progress regressions covering the indexing-to-analysis transition, `1 file` pluralization, final newline, and redirected forced progress; verify they fail against the current carriage-return-only renderer
- [x] 1.2 Erase only the active terminal progress line and correct file-count wording while leaving non-terminal output newline-delimited; verify the focused progress tests pass

## 2. Condense and Align Service Modes

- [x] 2.1 Add focused output regressions for Standard-only, assumed-Standard-only, combined Standard/assumed, Fast-only, mixed Standard/Fast, unavailable-tier mixtures, absence of `⚡`, aligned numeric columns, bounded pricing provenance, and unchanged separate JSON tier keys
- [x] 2.2 Project structured tiers into human modes, annotate single-mode aggregate rows without child rows, render concise child rows only for mixed modes, emit one `Standard*` explanation, and bound long human pricing fields; verify the focused human and JSON tests pass
- [x] 2.3 Add and implement the explicit `No model usage.` state, removing the empty model table while retaining lifetime totals; verify task and project empty-output tests pass

## 3. Document and Validate the Cleanup

- [x] 3.1 Update user-facing report examples and terminology for plain Fast labels, `Standard*`, single-mode aggregate annotations, wrapped pricing sources, and the empty-model state; verify repository search finds no obsolete lightning-tier example
- [x] 3.2 Build the debug binary and record named-project, current-project, and date-grouped corpus runs with Asciinema at 120 and 160 columns; run the ANSI analyzer and verify no stale progress suffix, no broad terminal-control sequences, aligned tables, and readable pricing fields
- [x] 3.3 Run `cargo fmt --check`, focused output/progress tests, `cargo test`, `openspec validate clean-up-report-terminal-output --strict`, and `git diff --check`; record every result before completion
