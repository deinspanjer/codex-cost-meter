# Pricing refresh — October 25, 2026

This refresh follows [Updating model pricing](../model-pricing.md). Sources were retrieved October 25, 2026. Prices estimate API token list cost, not subscription charges.

## Evidence and changes

The [API changelog](https://developers.openai.com/api/docs/changelog) records Ultrafast availability for GPT-6 Astra on September 29 and GPT-6.1 Sol on October 8. The [Ultrafast guide](https://developers.openai.com/api/docs/guides/ultrafast-mode) identifies the exact models and `service_tier: "ultrafast"`. The current [labeled pricing Markdown](https://developers.openai.com/api/docs/pricing.md) supplies these explicit USD-per-million-token cells:

| Exact model | Estimator effective date | Context | Input | Cache read | Cache write (30m) | Output |
| --- | --- | --- | --- | --- | --- | --- |
| gpt-6-astra | 2026-09-29 | ≤272,000 input | 60 | 6 | 75 | 300 |
| gpt-6-astra | 2026-09-29 | >272,000 input | 120 | 12 | 150 | 450 |
| gpt-6.1-sol | 2026-10-08 | ≤272,000 input | 12 | 0.60 | 15 | 60 |
| gpt-6.1-sol | 2026-10-08 | >272,000 input | 24 | 1.20 | 30 | 90 |

The [Astra](https://developers.openai.com/api/docs/models/gpt-6-astra) and [Sol](https://developers.openai.com/api/docs/models/gpt-6.1-sol) model pages corroborate the context threshold; Sol also explicitly describes Ultrafast as six times Standard. The JSON stores independent cells, not an inferred multiplier. Availability announcements plus matching current prices support these day-granular launch-boundary estimates; they do not establish an exact historical account billing cutover or an archived launch-day price capture. Earlier Ultrafast usage remains unpriced. Existing Standard/Fast histories, thresholds, and proxies are unchanged.

The parser now recognizes explicit applied `ultrafast` settings, reports them separately, and uses only that tier's dated prices. Unsupported model/tier/date combinations remain incomplete. Analysis cache version 6 forces reanalysis of version-5 entries that classified Ultrafast as unsupported. No cache deletion is required.

## Coverage and limitations

- A one-off component comparison checked all 28 existing comparable Standard/Fast rows in the flagship pricing tables, including long-context cells, against the resulting catalog: no differences. The specialized table also confirms GPT-5.3 Codex Standard/Fast rates. No current Standard/Fast price change was found.
- The exact [GPT-5.1 Codex](https://developers.openai.com/api/docs/models/gpt-5.1-codex), [Mini](https://developers.openai.com/api/docs/models/gpt-5.1-codex-mini), [Max](https://developers.openai.com/api/docs/models/gpt-5.1-codex-max), and [GPT-5.2 Codex](https://developers.openai.com/api/docs/models/gpt-5.2-codex) HTML pages confirm the existing Standard input/read/output cells. Their Markdown representations omit the price tables. GPT-5.1 Codex Fast cells are retained without fresh independent verification; absence from today's table is not evidence of changed historical pricing. Unpublished cache-write cells remain unknown.
- The [model catalog](https://developers.openai.com/api/docs/models) and [Codex model guidance](https://learn.chatgpt.com/docs/models) identify no additional general-purpose Codex model missing from the catalog since the preceding refresh. GPT-5.5's retirement from ChatGPT-authenticated Codex does not remove its historical prices or imply retirement from the API. Older shut-down Codex model histories are likewise preserved. Specialized cyber, life-sciences, image, audio, and tool-call billing remain outside this refresh.
- The October 20 changelog and [prompt-caching guide](https://developers.openai.com/api/docs/guides/prompt-caching) introduce 12-hour writes at a higher price than default 30-minute writes. The API exposes nonoverlapping `ttl_30m_tokens` and `ttl_12h_tokens` under `input_tokens_details.cache_write_tokens_details`; both lifetimes can occur in one request. This app parses Codex's flat `cache_write_input_tokens` counter, which has no lifetime split. Cache-write pricing continues to assume 30 minutes and can understate cost for 12-hour writes. This is a documented limitation, not evidence that all Codex writes changed price. No speculative rollout fields or global price increase are introduced.
- [Fast-mode guidance](https://developers.openai.com/api/docs/guides/fast-mode) still documents GPT-5.6 Sol's promotion through at least November 21; no future price increase is assumed. Batch/Flex, regional premiums, and subscription credits remain outside the estimator. Applied settings do not prove the tier actually served. Auto-review's existing authentication assumption and policy history are retained without fresh policy verification.

## Validation and review

`just test-filter pricing::tests` passed (14 tests). `just check` passed (187 Rust tests, 13 version-tool tests, formatting, and warnings-denied Clippy) on macOS. `git diff --check` passed. Native Windows/Linux execution is delegated to PR CI. Focused regressions cover Ultrafast date boundaries, 272,000/272,001 input, all four components, unchanged Standard/Fast prices, unsupported combinations, parser chronology, report/cache reuse, and human labeling.

`cargo build --locked --release` passed after rebasing onto current `main` (v1.3.1). A synthetic CLI report with 272,001 input tokens, 10,000 reads, 10,000 writes, and 15,000 output tokens produced $7.710024 for Sol Ultrafast, the correct human label, October 25 provenance, and matching cached JSON output. Full local checks passed again after the rebase. Self-review and final review found no remaining material issue; the lifetime and telemetry limitations above remain explicit. This PR does not publish a release, install a binary, or reprice existing titles.
