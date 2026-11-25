# Tasks

## 1. Scoped source evidence

- [x] 1.1 Bound flagship extraction and bind the shared input-token legend to supported tier tables. Exercise a reduced November 18 Markdown fixture retaining the intervening headings and next product section; reject misplaced, contradictory and model-qualified definitions. Document accepted source scope in `docs/model-pricing.md`.
- [x] 1.2 Retain legacy row qualifiers, Batch/Flex rows and both Ultrafast promotion/list headings in the fixture. Verify that a legacy qualifier does not block other models and that a missing end boundary does.
- [x] 1.3 Pass scoped facts from independently fetched pricing Markdown or an allowed exact raw capture into import validation. Check a legitimate round trip and an import whose row and cutoff both exist but in different sections; only the first qualifies.

## 2. Atomic onboarding

- [x] 2.1 Generate one threshold and dependent rate points per unfamiliar model using existing preconditions and groups. Verify the fixture yields separate Astra/Sol groups, no orphan writes on group failure, and continued independent eligibility. Preserve manual overrides, existing histories and standalone mode.
- [x] 2.2 Apply to an isolated home, inspect effective rates at 272000/272001 tokens and before the observation date, and repeat refresh to verify idempotence. Document the date limitation and partial coverage in `USERS.md`.
- [x] 2.3 Verify the applied cutoff has observed provenance and its URL. Exercise a model with existing short-only history to verify that adding a retroactive cutoff is refused.

## 3. Review and acceptance

- [x] 3.1 Summarize validated groups before unresolved details. Inspect human preview for rates, cutoff, date basis, source scope and blocked reasons; compare JSON to verify full excerpts, evidence references, activation and coverage remain available. Update the refresh example and unreleased changelog entry.
- [x] 3.2 Detect model rows in named but unsupported Ultrafast promotion/list tables; verify one model/tier coverage gap per affected model, linked to its otherwise eligible group in both output modes.
- [x] 3.3 Run focused fixture checks, `just check`, `git diff --check`, and strict change validation. Exercise the current source via a temporary-home dry run and record actual coverage separately from deterministic fixture results.

## Verification — November 18

- Fixture: scoped and misplaced legends, a stale group alongside an eligible group, import round trip, and short-only history protection passed. `cargo test --bin codex-cost-meter pricing` passed 53 focused tests. `just check` passed formatting, 245 Rust tests, 13 version-tool tests, and Clippy on macOS. `git diff --check` and strict OpenSpec validation passed.
- Live official Markdown, temporary home: human and JSON previews showed two eligible groups (eight Standard/Fast rate points and two cutoffs), and one unsupported Ultrafast item per model. Exit 3 indicated partial coverage. Independent import preview also returned ten eligible changes. After applying to the isolated catalog, Astra's short band ended at 272000 and long began at 272001, earlier pricing remained unknown, thresholds had observed basis/source, and a repeat preview reported unchanged.
