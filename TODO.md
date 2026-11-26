# TODO

## Accounting

- Check whether current `thread_spawn` subagent rollouts replay parent `token_count` history, as reported in [ccusage issue #950](https://github.com/ccusage/ccusage/issues/950). v1.1.0 removes replay only from explicit legacy forks and deliberately keeps subagent-only candidates. Remove a subagent prefix only when it exactly matches the parent's usage sequence. Keep ambiguous usage, and do not use timestamp-burst or cross-session dedupe heuristics.
- Add multi-home reporting: replace the report-only `--codex-home` with `--codex-homes HOME[,HOME...]`, and deduplicate rollout IDs across homes. `update` and `schedule` keep a single `--codex-home`. See step 5 of the [handoff plan](docs/plans/ccusage-accounting-pricing-and-multi-home-handoff.md#5-add-multi-home-reporting).
- Explore reporting a session's active time ranges, separate from summed turn time. Define which idle gaps split a range, show disjoint date ranges compactly, and verify how automatic continuations and scheduled wakeups appear in rollout lifecycle events.

## Titles

- Evaluate titling threads younger than 15 minutes without racing or overriding the improved title that Codex generates after a delay.

## Diagnostics and support

- Add a `doctor` command once several concrete diagnostics justify it. Start with rollout-cache integrity, writability checks, and explicit repair.
- Add a privacy-safe issue-submission mode. It collects only allowlisted fields and requires confirmation before it sends anything.

## Installation feedback

So far, the only user-confirmed installation is on an Apple Silicon Mac. Reports of successful installs or problems are welcome.
