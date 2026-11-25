# API pricing refresh — November 25, 2026

The [API changelog](https://developers.openai.com/api/docs/changelog#november-2026) dates the GPT-6.2 Astra and Sol launches to November 17. A [November 18 pricing capture](https://web.archive.org/web/20261118005301id_/https://developers.openai.com/api/docs/pricing) corroborates the launch prices. November 17 is the estimator's start date, not an observed billing cutover. The [Astra](https://developers.openai.com/api/docs/models/gpt-6.2-astra) and [Sol](https://developers.openai.com/api/docs/models/gpt-6.2-sol) pages place the whole request in the long band above 272,000 gross input tokens.

The [official pricing tables](https://developers.openai.com/api/docs/pricing.md) give separate Standard, Fast, promotional Ultrafast, and list Ultrafast rows. The catalog uses 30-minute write prices. Ultrafast promotion pricing runs through February 28, 2027; the list rate starts March 1, Pacific time. Catalog dates have day precision. For a rate table at today's prices, pass today's date to `rates --as-of`. Without a date, `rates` shows the latest published point, including the announced list rate. A dated rollout uses its event date.

The November audit found no differences in the 62 comparable existing API rate rows. Two archived Priority rows fill historical gaps, in USD per million input, cached input, and output tokens:

- [`gpt-5.1-codex-max`, December 11, 2025](https://web.archive.org/web/20251211103307id_/https://platform.openai.com/docs/pricing): 2.50, 0.25, 20.
- [`gpt-5.2-codex`, January 16, 2026](https://web.archive.org/web/20260116030822id_/https://platform.openai.com/docs/pricing): 3.50, 0.35, 28.

The observations do not establish earlier start dates or cache-write rates. The historical API rates do not change which Codex releases persisted Fast-tier settings. GPT-5.6 Sol's current promotion runs through November 30; no replacement rate is assumed. Proxy routing was not independently reverified. Public-credit and quota estimates use separate data.

## Verification

The packaged macOS binary returned all six tier and context rows per model in both a clean home and a home with the November 18 observed overlay. The rows matched the official table fixture on November 17, November 25, February 28, and March 1. A clean-home `pricing refresh --dry-run --json` proposed no changes; exit 3 reported partial source coverage. The focused pricing tests, `just check`, `just version-verify`, `just package`, and strict validation of the pricing refresh specification passed.
