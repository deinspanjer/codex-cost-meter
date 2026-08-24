# Validation

## Focused regressions

- Before the progress fix, `cargo test progress::tests` produced the expected red result: the redirected case passed and the terminal case failed because output contained carriage returns without `ESC[2K` and rendered `1 files`.
- After implementation, `cargo test output::tests` passed 10 tests and `cargo test progress::tests` passed 2 tests.
- The output regressions cover Standard-only, assumed-only, combined Standard, Fast-only, mixed and unavailable modes, visible-column alignment, bounded pricing provenance, separate JSON tier keys, and empty task/project/corpus model sections.

## Asciinema evidence

Built `target/debug/codex-cost-meter` and recorded:

- `/tmp/codex-cost-meter-cleanup.H8EVxE/project-ts.cast` at 160x36 with `report --project ts --refresh`
- `/tmp/codex-cost-meter-cleanup.H8EVxE/current-project.cast` at 120x32 with `report --project . --refresh`
- `/tmp/codex-cost-meter-cleanup.H8EVxE/grouped-corpus.cast` at 160x36 with `report --all --since 2026-08-24 --through 2026-08-24 --group-by day,rollout-type --refresh`

The asciinema ANSI analyzer reported zero clear-screen, clear-to-end, scroll-region, cursor-home, cursor-visibility, and alternate-screen sequences in every cast. A raw event check found 415, 2, and 12 targeted `CR ESC[2K` updates respectively; no bare `CR Analyzing` transition, lightning glyph, or missing final newline remained. The 120-column empty report rendered `No model usage.` and bounded pricing continuations; the 160-column reports showed aligned aggregate and mixed-mode rows.

## Completion checks

- `cargo fmt --check`: passed
- `cargo test`: passed (130 unit, 26 report CLI, 5 schedule CLI, and 6 update CLI tests)
- `openspec validate clean-up-report-terminal-output --strict`: passed
- `rg` over `USERS.md`, `docs`, `src`, and the active change found no obsolete `⚡ Fast`, `Standard (assumed)`, or singular `Pricing source:` presentation example
- `git diff --check`: passed
