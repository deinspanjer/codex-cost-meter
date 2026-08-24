# Codex Auto-review model and pricing evidence

Research date: 2026-08-24.

## Conclusions

1. OpenAI's [April 30 Alignment publication](https://alignment.openai.com/auto-review/) explicitly identifies the launch configuration as GPT-5.4 Thinking with low reasoning. Auto-review is a separate reviewer call, so its tokens must not be attributed to the main task model.
2. On July 30, OpenAI announced a migration of Auto-review in the ChatGPT app and Codex CLI from GPT-5.4 to GPT-5.6 Luna, with an expected roughly 10x cost reduction. The original announcement is not available as a durable OpenAI documentation page; [IBTimes](https://www.ibtimes.sg/openai-says-gpt-5-6-helped-optimize-infrastructure-enabling-api-price-cuts-see-list-91205) and [Techmeme](https://www.techmeme.com/260730/p58) preserve it contemporaneously. This is strong migration evidence, but secondary preservation.
3. OpenAI's current [Codex changelog](https://learn.chatgpt.com/docs/changelog) independently records the July migration of bundled GPT-5.4 selections and internal uses to GPT-5.6 Terra and Luna variants. It does not identify the Auto-review target or an exact serving cutover.
4. Current [Auto-review documentation](https://learn.chatgpt.com/docs/sandboxing/auto-review) explains the separate reviewer and its context, but does not name the backing model, reasoning effort, or billing SKU. Current [Codex pricing documentation](https://learn.chatgpt.com/docs/pricing) describes plan and token-credit mechanics but likewise does not map `codex-auto-review` to a billable model.
5. The [Help Center credit card](https://help.openai.com/en/articles/11481834-chatgpt-rate-card-business-enterpriseedu-credit-based-pricing) still says Auto-review uses GPT-5.4. That conflicts with the newer migration evidence and should not override it for current estimates. It remains evidence of documentation drift, not evidence of a production rollback.
6. Local rollouts expose the stable `codex-auto-review` identity and token categories, not the routed model. No local field can prove the backend or billing rate for an individual request.

## Evidence timeline

| Date | Evidence | What it supports |
| --- | --- | --- |
| 2026-04-20s | The April 30 Alignment publication says Auto-review was released the preceding week. | Approximate public launch window; not an exact day. |
| 2026-04-30 | [OpenAI Alignment](https://alignment.openai.com/auto-review/) says the reviewer used GPT-5.4 Thinking with low reasoning. | Authoritative launch model and effort. |
| 2026-06-20 | A [Developer Community diagnosis](https://community.openai.com/t/5-5-xhigh-requests-going-to-5-4-solved/1384128) attributes unexpected GPT-5.4 traffic to Approve for me. | User corroboration before the migration; not pricing authority. |
| 2026-07-21 | The [Codex changelog](https://learn.chatgpt.com/docs/changelog) records migration of bundled GPT-5.4 selections and internal uses to GPT-5.6 variants. | Official corroboration of the broader internal migration. |
| 2026-07-30 | Contemporary reports preserve OpenAI's announcement that Auto-review was moving from GPT-5.4 to GPT-5.6 Luna. | Best public date for a historical estimator boundary. |
| 2026-08-24 | Current Auto-review and Codex pricing docs omit a canonical alias-to-model billing map. | The remaining ambiguity is still current. |

## Token and price semantics

The reviewer receives a compact transcript and the exact approval request. Relevant user messages, surfaced assistant updates, and tool evidence may be included; the main agent's hidden reasoning is excluded. The reviewer may rarely perform read-only checks. These are current documented product behaviors, not inferred pricing fields.

The Business, Enterprise, and Edu credit card describes Auto-review as token-metered Codex activity using actual input, cached-input, and output tokens. It documents no fixed per-review fee or separate Guardian surcharge. Current public rates make GPT-5.4 12.5 times the price of GPT-5.6 Luna for the same token mix, both in the credit table and the corresponding API list rates:

| Model | API input / 1M | API cached input / 1M | API output / 1M |
| --- | ---: | ---: | ---: |
| [GPT-5.4](https://developers.openai.com/api/docs/models/gpt-5.4) | $2.50 | $0.25 | $15.00 |
| [GPT-5.6 Luna](https://developers.openai.com/api/docs/models/gpt-5.6-luna) | $0.20 | $0.02 | $1.20 |

Those public model rates support an API-list-price estimate. They do not prove that every ChatGPT allowance decrement or API invoice for the internal alias used the corresponding public row.

## Observed, inferred, and unknown

### Observed

- Launch documentation names GPT-5.4 Thinking and low reasoning.
- A July 30 announcement names a GPT-5.4 to GPT-5.6 Luna migration for Auto-review.
- Current OpenAI-owned Codex source selects the internal reviewer from model/provider metadata and prefers low reasoning when the selected model supports it ([pinned source](https://github.com/openai/codex/blob/09609ba4148ec85d758c65b11d969ff8e0661e37/codex-rs/core/src/guardian/review.rs)).
- Rollouts and OTel can report `codex-auto-review` token usage without the physical backend. [`openai/codex` issue #20981](https://github.com/openai/codex/issues/20981) documents the resulting accounting gap.

### Estimator policy

- Treat `codex-auto-review` usage before 2026-07-30 as GPT-5.4.
- Treat usage on or after 2026-07-30 as GPT-5.6 Luna.
- When the event timestamp is unavailable, use the latest known target, GPT-5.6 Luna, consistent with the catalog's existing latest-rate fallback.
- Report the mapping as an announcement-date proxy, not an observed routed or billed model.

This date rule replaces the prior timeless Luna proxy, so known pre-migration reviewer usage is no longer repriced at the much lower post-migration rate. It is still an approximation: the public evidence does not establish a single UTC cutover or a uniform rollout across accounts, authentication methods, clients, regions, and product surfaces.

### Unknown

- The exact cutover timestamp or rollout window for each account and surface.
- Whether API-key requests used the same migration schedule and public Luna billing SKU.
- The exact formula translating reviewer tokens into Plus or Pro included-allowance percentages.
- A post-migration prose statement that explicitly combines GPT-5.6 Luna with low reasoning.
- Whether OpenAI will later route the stable alias to another model without changing local telemetry.

## Recommendation

Retain the effective-dated proxy history in the embedded catalog. Keep ordinary model rate histories unchanged, resolve the proxy target before selecting the target model's event-date rates, validate every referenced target and increasing proxy date, use the newest mapping when time is absent, and expose the proxy history in report provenance.

Do not claim billing accuracy. Billing-grade reconciliation still requires an authoritative server-side routed/billable model or an OpenAI ledger. The local estimator should make the evidence boundary visible and remain easy to update when OpenAI publishes a better date or target.
