Implementation is on hold by owner instruction as of 2026-10-30. Do not start the tasks below until the owner explicitly resumes implementation. CLI spelling is confirmed as `--cost-metric api-usd|credits`.

## 1. Rate evidence and catalog

- [ ] 1.1 Capture the full applicable official credit rate card, speed/context rules, observation date, historical evidence from Wayback snapshots of learn.chatgpt.com/docs/pricing and Help Center article 11481834, and model coverage. Record exact capture URLs/dates; state expected pre-card and unsupported retired-model incompleteness and represent future evidenced GPT-5.6 Sol promotional changes with effective dates (November 21 is not an assumed cutoff). Document legacy/USD Enterprise exclusions and unsupported telemetry; do not backdate current rates.
- [ ] 1.2 Add the independent embedded credit catalog and small cost-metric enum. Keep existing API rates and meanings unchanged; justify any shared model proxies.
- [ ] 1.3 Implement selected valuation, independent completeness/provenance, and credit cache-write treatment after verifying normalized counter semantics.

## 2. CLI and report surfaces

- [ ] 2.1 Add global `--cost-metric api-usd|credits` with default `api-usd`; thread selection through existing dispatch and regenerate completion behavior from Clap.
- [ ] 2.2 Apply selection across exact/project/corpus/date/group/model/tier reports and `rates`, with unit-specific labels and fractional display.
- [ ] 2.3 Add selected-cost objects while retaining USD fields; extend JSONL metadata/event/summary additively under schema 1 and document optional fields and unknown-field tolerance.

## 3. Titles and schedules

- [ ] 3.1 Make title cost composition, canonical suffix stripping, and idle-update matching metric-aware, using the strict U+00A4 grammar (`¤12.34`, `¤0.50`, `¤1,234.56`, optional leading `<` and trailing `+`), preserving width and preview/apply safeguards. Always use two decimals; render positive sub-0.01 amounts as `<¤0.01`. Permit ASCII `c` fallback only for an observed rendering problem; never use `¢`.
- [ ] 3.2 Propagate selection through native schedule installation and run arguments on macOS, Windows, and Linux; retain default behavior for existing jobs.

- [ ] 3.3 Show the installed metric in `schedule status`; inspect it read-only before manual update preview/apply and warn on metric mismatch without blocking or changing selection. Preserve JSON stdout, handle absent/indeterminate schedules honestly, and explain repeated title replacement, update-limit consumption, and session_index appends.

## 4. Focused verification and delivery

- [ ] 4.1 Read the current testing guidelines and extend focused pricing coverage for mixed cached/write/reasoning usage, speed/context selection, historical gaps, and independent credit/USD completeness. Include 272,000/272,001 gross-input boundary cases: without an evidenced per-model credit long-context rule, the over-threshold event must have an unavailable complete estimate and preserve only evidenced known minima; a subscription-only exemption must not make credit valuation complete. These protect against double charging and invented credit costs; reuse existing fixtures rather than repeating the corpus.
- [ ] 4.2 Verify global argument placement/default/rejection and one selected-cost fixture across human report, JSON, JSONL schema 1, aggregation, and rates. Confirm unchanged USD values and cache-reuse parity; avoid redundant full report-mode matrices.
- [ ] 4.3 Verify dollar-to-credit and reverse title transitions, idle eligibility, width, and dry-run immutability. Verify strict two-decimal credit suffix recognition and replacement for `¤12.34`, `¤12.34+`, `¤0.50`, `¤1,234.56`, `<¤0.01`, and `<¤0.01+`, rejecting malformed grouping/decimals and noncanonical credit prose. Verify ASCII fallback compatibility if rendering evidence requires it. Exercise each native schedule's generated arguments through existing fake-runner seams, without installing real jobs.
- [ ] 4.4 Verify manual preview/apply mismatch warnings in both directions (including omitted/default `api-usd`), matching metrics, absent/indeterminate schedule state, installed/default metric status display, and valid structured stdout, without real job installation.
- [ ] 4.5 Update README/USERS/architecture and release notes with CLI examples, credit provenance, historical coverage, estimate limitations, and JSONL compatibility. In USERS.md explain the manual/scheduled mismatch warning and flip-flop consequences, show installed-metric status output, and show how to align the selections.
- [ ] 4.6 Run focused `just test-filter` checks first, then `just check`; use existing platform CI for affected scheduler branches. Record commands/results and complete self-review and final review. Keep planning status distinct from implemented/validated status.
