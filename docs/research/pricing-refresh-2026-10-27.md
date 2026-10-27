# Pricing refresh — October 27, 2026

This refresh follows [Updating model pricing](../model-pricing.md). Official sources were retrieved October 27, 2026. Rates estimate API token list cost, not subscription charges.

## GPT-6.1 Terra

The [API changelog](https://developers.openai.com/api/docs/changelog#october-2026) dates the launch of `gpt-6.1-terra` to October 26. The [model page](https://developers.openai.com/api/docs/models/gpt-6.1-terra) confirms the exact identity, 5% cache-read rate, 30-minute write price, and whole-request long-context adjustments above 272,000 input tokens. The labeled [pricing tables](https://developers.openai.com/api/docs/pricing) publish these USD-per-million-token cells:

| Tier | Gross input | Input | Cache read | Cache write (30m) | Output |
| --- | --- | --- | --- | --- | --- |
| Standard | ≤272,000 | 1 | 0.05 | 1.25 | 5 |
| Standard | >272,000 | 2 | 0.10 | 2.50 | 7.50 |
| Fast | ≤272,000 | 2 | 0.10 | 2.50 | 10 |
| Fast | >272,000 | 4 | 0.20 | 5 | 15 |

The launch announcement plus matching current model and pricing documentation support October 26 as a day-granular estimator boundary. This is an inference about launch pricing, not an archived launch-day capture or an exact account billing cutover. Earlier usage remains unpriced. Terra is a separate model, not a proxy for GPT-5.6 Terra or GPT-6.1 Sol. No Terra Ultrafast prices are published, so that combination remains unpriced. The [Codex model guide](https://learn.chatgpt.com/docs/models) confirms Terra's availability in Codex; no other new relevant general-purpose Codex model was identified.

## Existing-model audit

A one-off component comparison against the fetched labeled pricing Markdown checked 54 unique Standard/Fast/Ultrafast context-and-tier rate sets across 17 flagship models, including the four new Terra sets. All matched. Each comparison included input, cache read, default 30-minute write, and output; unpublished cells remained unknown. The separate specialized table confirms GPT-5.3 Codex Standard (1.75 / 0.175 / unknown / 14) and Fast (3.50 / 0.35 / unknown / 28) input/read/write/output cells.

The exact HTML pages for [GPT-5.1 Codex](https://developers.openai.com/api/docs/models/gpt-5.1-codex), [Mini](https://developers.openai.com/api/docs/models/gpt-5.1-codex-mini), [Max](https://developers.openai.com/api/docs/models/gpt-5.1-codex-max), and [GPT-5.2 Codex](https://developers.openai.com/api/docs/models/gpt-5.2-codex) freshly confirm their existing Standard input/read/output prices. All ordinary catalog models therefore had their published Standard prices checked. GPT-5.1 Codex Fast prices are retained without fresh independent verification: the exact page, current pricing tables, Fast guide, and targeted official documentation search did not supply those cells. Absence from today's tables does not establish a historical price change. Its unpublished write price remains unknown. No existing history, threshold, or proxy was changed.

The [Fast guide](https://developers.openai.com/api/docs/guides/fast-mode) still describes GPT-5.6 Sol's promotion through at least November 21; no future price increase is inferred. Auto-review's separately documented authentication assumption and zero-price policy history are retained without a new policy audit. Specialized cyber, life-sciences, image, audio, tool fees, Batch/Flex, regional premiums, and subscription billing remain outside this Codex token catalog refresh.

The existing 30-minute write assumption remains a material limit: Codex's flat write counter does not distinguish 12-hour writes, so those estimates can understate cost. Terra's published 12-hour write prices are twice the corresponding 30-minute cells, but applying them globally would overprice ordinary writes. Applied tier settings also do not prove the tier actually served. These limitations are unchanged from the [October 25 review](pricing-refresh-2026-10-25.md).

## Verification

Extended the existing behavioral pricing regression with Terra's launch boundary, undated lookup, 272,000/272,001 gross input, distinct cache-read rate, all four cost components, Standard/Fast, and unsupported Ultrafast. The independently calculated Standard totals for 10,000 reads, 10,000 writes, and 15,000 output tokens are $0.340000 and $0.642502; Fast totals are $0.680000 and $1.285004. This protects against leaving Terra unknown, borrowing its predecessor's prices, or charging short-context rates above the threshold. The catalog is resolved at report time, so no analysis-cache version bump is needed.

`just test-filter pricing::tests` passed (14 tests). `just check` passed (189 Rust tests, 13 version-tool tests, formatting, and warnings-denied Clippy) on macOS. `cargo build --locked --release` passed. A synthetic release-binary CLI report produced $1.285004 for Terra Fast at 272,001 input tokens, displayed $1.29 with the Fast label and October 27 provenance, and returned identical JSON after cache reuse. The initial smoke-script assertion expected unrounded cost in human output; correcting it to the documented currency display passed without an application change. `git diff --check` passed. Self-review and final diff review found no material issue; the evidence and telemetry limitations above remain explicit. This change does not publish a release, install the binary, or reprice existing titles.
