# Credit token-rate evidence

Retrieved 2026-11-19. This is independent of API prices and the frozen quota
calibration. Rates are credits per million **uncached input / cached input /
output** tokens. Gross rollout input includes cached input; subtract the cached
subset before applying the ordinary input rate. Cache writes have no additional
charge. Reasoning remains within output.

## Retrieved official captures

These are retrieved contents, not merely CDX index entries. Capture timestamps
are UTC observations, not explicit provider effective dates.

| Capture | Contents relevant to implementation |
| --- | --- |
| [20260709184412](https://web.archive.org/web/20260709184412id_/https://learn.chatgpt.com/docs/pricing) | 5.6 Sol 250/25/1500; Terra 125/12.5/125; Luna 50/5/300. 5.5 125/12.5/750, 5.4 62.5/6.25/375, 5.4 mini 18.75/1.875/113. No numerical Fast rule. |
| [20260801125602](https://web.archive.org/web/20260801125602id_/https://learn.chatgpt.com/docs/pricing) | 5.6 Sol 125/12.5/750; Terra 50/5/300; Luna 5/0.5/30. Same retired-model rows. |
| [20260902013718](https://web.archive.org/web/20260902013718id_/https://learn.chatgpt.com/docs/pricing) | 5.6 Sol 100/10/500 promotion, Terra 50/5/300, Luna 5/0.5/30. Same retired-model rows. |
| [20260906071102](https://web.archive.org/web/20260906071102id_/https://help.openai.com/en/articles/11481834-chatgpt-rate-card-business-enterpriseedu-credit-based-pricing) | Astra 250/25/1250; 5.6 family matches September 2; 5.5/5.4/mini as above; 5.3-Codex and 5.2 each 43.75/4.375/350. Astra Fast explicitly 2.5×. Auto review uses 5.4 and is included in charged activities here. |
| [20261015145421](https://web.archive.org/web/20261015145421id_/https://help.openai.com/en/articles/11481834-chatgpt-rate-card-business-enterpriseedu-credit-based-pricing) | 6.1 Sol 50/2.5/250; Astra 250/25/1250; 6 Sol 50/5/250; 6 Luna 2.5/0.25/12.5; 5.6 family unchanged. Fast 2× for these four 6-family models; Ultrafast 6× only Astra and 6.1 Sol. Signed-in auto-review safety checks explicitly free. |
| [20261101190956](https://web.archive.org/web/20261101190956id_/https://learn.chatgpt.com/docs/pricing) | Adds 6.1 Terra 25/1.25/125; otherwise corroborates the October general-purpose card. Fast 2×, Astra/6.1 Sol Ultrafast 6×. No separate cache-write charge. Subscription-only 6.1 Sol no-context-surcharge statement. |

The July Terra output row is anomalous relative to later cards; do not silently
“correct” it using API prices. Its applicability and the exact intervening
cutovers are unresolved. Archived observations establish that earlier rates
existed, not their launch dates or exact expiry instants. Before-card usage,
unresolved cutover intervals, unidentified aliases, and speed combinations
without dated numerical evidence must remain incomplete. In particular,
historical GPT-5.x-Codex IDs cannot all inherit the 5.3-Codex row.

## Current official observations

- [Pricing](https://learn.chatgpt.com/docs/pricing), observed 2026-11-19,
  now redirects to the developer documentation. The eight frozen-study model
  credit triples agree with the November capture. Additional 6.2 Astra is
  200/10/1000 and 6.2 Sol 50/2.5/250; these are not evidence for earlier dates.
- [Article 11481834](https://help.openai.com/en/articles/11481834),
  observed 2026-11-19, agrees on those general-purpose token rates. It also
  lists specialized Law, Cyber, Rosalind and image rows; exact telemetry and
  applicable dated coverage must be established before supporting those IDs.
- The current article explicitly exempts **Enterprise Codex with ChatGPT
  sign-in** from the 272K context threshold. This is narrower than a universal
  credit exemption. Neither an unidentified account nor the subscription-only
  6.1 Sol statement establishes this credit rule. Above 272000 gross input,
  missing applicable evidence means no complete price and no invented base-rate
  lower bound. API context surcharges are not credit evidence.
- Current Fast is 2× where supported. Current Astra and 6.1 Sol Ultrafast is
  6×. The 6.2 Ultrafast promotion has its own 3×/6× rule and must not inherit
  the older models' 6× rule indiscriminately.
- Auto-review's signed-in free policy has credit-source evidence in the
  October capture, unlike the September card. It must not be backdated or
  inferred merely from the API catalog's policy or an ordinary review title.

GPT-5.6 Sol promotional pricing is available **at least through November 21,
2026**. This does not prove an expiry or a November 22 rate. No such automatic
cutoff is justified.

## Scope

Token-based estimates exclude legacy Enterprise per-message/per-task cards,
Enterprise agreements billed in USD, discounts, purchase prices, commitments,
actual balances and deductions. Tool/image/voice charges cannot be inferred
from ordinary input/cache/output counters. The sources themselves distinguish
credit billing from included subscription usage and API-key pricing.

## Research status

The initial embedded catalog deliberately covers the eight general-purpose
study model IDs only. Seven begin at the October 15 observation, 6.1 Terra
Standard at the November 1 observation, and Terra Fast at the current
November 19 observation. Each row retains the capture supporting it. These are
observation-boundary estimates with disclosed carry-forward assumptions, not
claims about actual billing cutovers. Earlier cards document real changes but
do not establish their exact effective intervals; they are not silently
extended backwards or used to interpolate between conflicting observations.
Recovered retired-model rows are preserved here for future dated research,
not presented as complete historical coverage. All initial token coverage is
conservatively bounded to gross input <=272000; this is a catalog coverage cap,
not a claim that every model has that context-pricing threshold.

Every shipped triple was checked against its cited capture above. November's
general-purpose triples match October where both exist. The November capture's
general Fast wording alone was not treated as evidence of Terra Fast
availability; the current article explicitly lists it. Auto-review/guardian
credits remain unsupported because these normalized records do not establish
the signed-in authentication condition for the free policy. No API proxy,
long-context multiplier or zero-price policy is imported.

Normalization metadata was checked against the public Codex
[protocol](https://github.com/openai/codex/blob/main/codex-rs/protocol/src/protocol.rs)
and [response metadata type](https://github.com/openai/codex/blob/main/codex-rs/protocol/src/response_usage.rs)
on 2026-11-19. Recorded `thread_source = "guardian_review"` is distinct from
ordinary review. `creator_account_id` never changes on resume and is therefore
only a disclosed account assumption. A flat string `account_id` in an identical
`raw_response_completed.usage_metadata.metadata` can identify that response;
unrecognized/missing metadata is not guessed. Completion token counters are
never added to the following normalized token_count counters.
