# Pricing refresh — September 29, 2026

This refresh follows [Updating model pricing](../../DEVELOPERS.md#updating-model-pricing). It reviews the existing catalog and adds the newly released GPT-6 Sol, Luna, and GPT-6.1 Sol identities. Prices are API token list estimates, not subscription charges. All sources below were retrieved September 29, 2026.

## Evidence and changes

The [official pricing Markdown](https://developers.openai.com/api/docs/pricing.md) preserves labeled Standard and Fast tables, including collapsed rows missing from the extracted HTML. [The API changelog](https://developers.openai.com/api/docs/changelog) dates GPT-6 Sol and Luna to September 22 and GPT-6.1 Sol to September 29. Their exact model pages confirm the component rates and a surcharge above 272,000 gross input tokens:

| Exact model | Effective date | Standard short input / cache read / cache write / output | Standard long input / cache read / cache write / output |
| --- | --- | --- | --- |
| [gpt-6-sol](https://developers.openai.com/api/docs/models/gpt-6-sol) | 2026-09-22 | 2 / 0.20 / 2.50 / 10 | 4 / 0.40 / 5 / 15 |
| [gpt-6-luna](https://developers.openai.com/api/docs/models/gpt-6-luna) | 2026-09-22 | 0.10 / 0.01 / 0.125 / 0.50 | 0.20 / 0.02 / 0.25 / 0.75 |
| [gpt-6.1-sol](https://developers.openai.com/api/docs/models/gpt-6.1-sol) | 2026-09-29 | 2 / 0.10 / 2.50 / 10 | 4 / 0.20 / 5 / 15 |

Units are USD per million tokens. Each of these three models explicitly publishes Fast at twice its corresponding Standard cells; the JSON stores those values independently. GPT-6.1 Sol's cache reads cost half GPT-6 Sol's, so these identities must not be merged. Launch dates plus matching current model documentation are the estimator's effective-date evidence; they do not establish exact intraday billing cutovers.

The audit found missing Pro long-context cells and overly late evidence boundaries for the corresponding base models. Historical follow-up retrieved the official pricing page from the Internet Archive:

| Models | Catalog long-context start | Evidence |
| --- | --- | --- |
| GPT-5.4 and GPT-5.4 Pro | 2026-03-05 | [March 5 capture, 21:27:01 UTC](https://web.archive.org/web/20260305212701/https://developers.openai.com/api/docs/pricing) explicitly lists Standard >272K rows: 5 / 0.50 / 22.50 for GPT-5.4 and 60 / unavailable / 270 for Pro. The [March 5 announcement](https://openai.com/index/introducing-gpt-5-4/) confirms same-day API launch. |
| GPT-5.5 and GPT-5.5 Pro | 2026-04-24 | The [April 25 capture, 10:46:44 UTC](https://web.archive.org/web/20260425104644/https://developers.openai.com/api/docs/pricing) lists Standard long-context rows: 10 / 1 / 45 and 60 / unavailable / 270 respectively. The [launch post's April 24 update](https://openai.com/index/introducing-gpt-5-5/) and API changelog establish API availability. The [April 24 GPT-5.5 model-page capture](https://web.archive.org/web/20260424183001/https://developers.openai.com/api/docs/models/gpt-5.5) explicitly confirms the base model's >272K surcharge that day. For Pro, applying the next-day-confirmed rates from April 24 is a corroborated launch-boundary inference, not an independently timestamped billing change. |

These triples are input / cache read / output in USD per million tokens. Cache writes and Pro Fast rates remain unavailable. Existing short-context points are preserved, including April 23's announced GPT-5.5 tariff for Codex estimates; the April 24 API launch is distinct from that announcement and the model snapshot name. GPT-5.5 long-context usage before April 24 remains incomplete. Current exact-model documentation supports the 272,000 threshold. The April 23 pricing capture did not contain GPT-5.5, so it cannot establish that model's long-context tariff. Archived HTML was retrieved and decompressed before checking labeled tables; the archive index alone was not used as proof of rates.

The initial refresh used September 29 for both Pro models and retained August 22 for the base models. That was too conservative because the historical search was incomplete. The new evidence replaces those boundaries without inventing a rate change on the day this maintenance work happened. Pro cache rates remain unknown; “no cached input discount” alone is not used to invent a cache-read billing cell.

## Coverage and retained limitations

- All 28 comparable current Standard/Fast rows in the main labeled tables match the resulting catalog, including long-context cells. Existing Astra, GPT-5.1, GPT-5.2, GPT-5.2 Pro, GPT-5.4/mini/nano, GPT-5.5, and GPT-5.6 Sol/Terra/Luna rates are unchanged. The specialized table confirms GPT-5.3 Codex Standard and Fast rates.
- The exact [GPT-5.1 Codex](https://developers.openai.com/api/docs/models/gpt-5.1-codex), [Mini](https://developers.openai.com/api/docs/models/gpt-5.1-codex-mini), [Max](https://developers.openai.com/api/docs/models/gpt-5.1-codex-max), and [GPT-5.2 Codex](https://developers.openai.com/api/docs/models/gpt-5.2-codex) pages confirm their existing Standard input/cache-read/output rates. GPT-5.1 Codex's existing Fast cells were not independently reconfirmed by these pages and are retained, not newly certified. Absence from a current table is not evidence of a historical price change.
- GPT-5.6 August 22 long-context evidence boundaries and all proxy histories remain unchanged; this follow-up establishes GPT-5.4/5.5 boundaries, not a full historical audit of every model. The [Fast guide](https://developers.openai.com/api/docs/guides/fast-mode) still documents GPT-5.6 Sol's promotion through at least November 21; no future increase is assumed. No new Auto-review routing evidence was established.
- Batch, Flex, Ultrafast, regional/FedRAMP premiums, tool fees, and subscription credits are outside the current estimator. Unsupported recorded service tiers remain unpriced. Current sources do not resolve actually served tiers in local rollout telemetry.
- Earlier sessions were searched with Total Recall (`skill:d06c6623 cli:1.8.0`); project indexing was stale. Retrieved September 18 entries `f7909605` and `0746c3f2` support keeping exact proxy history in JSON and material uncertainty visible. The September 5 task authorization was found, but not its implementation conversation. Commit `258a5b4` supplies the prior update's durable record: preserve historical prices and evidence boundaries, correct dates only with evidence, and test mixed components and date/context boundaries.

## Validation and review

- `just test-filter pricing::tests`: passed, 12 tests. The first sandboxed attempt could not unpack a dependency into the Cargo cache; the rerun with cache access passed.
- `just check`: passed, 176 Rust tests and 13 version-tool tests, formatting, and warnings-denied Clippy on macOS. Native Windows/Linux execution was not performed for this platform-neutral catalog change.
- A one-off comparison of the catalog's latest cells against the fetched labeled pricing Markdown passed for 28 rows. Specialized Codex and exact legacy-model pages were checked separately as described above.
- `cargo build --locked --release`: passed; updated local executable at `target/release/codex-cost-meter`.
- `git diff --check`: passed.

Post-update review and the historical follow-up found three procedural details worth making explicit: historical source searches must precede observation-date fallback; a missing headline-table row does not establish a price change, and a new model-level threshold affects historical requests even when old rate points are untouched. All three are now in the procedure. The historical boundaries are covered by Pro and base-model regression tests and disclosed above. Self, task, and final review found no remaining material issue in this change; retained source/telemetry limitations are listed above. This is a repository update and local build; publication, installation, and title repricing are separate operations.
