## Why

Reports currently price every request at Standard API list rates even when Codex recorded Fast/Priority processing. This understates Fast usage and cannot represent GPT-5.6's request-size pricing boundary without compromising the existing incomplete-cost guarantee.

## What Changes

- Associate each normalized usage event with the chronologically applied Codex service tier.
- Price Standard and Fast usage from explicit effective-dated short- and long-context catalog rates.
- Treat usage without a recorded tier as Standard: definitive before the first public Fast-capable Codex release and explicitly assumed afterward. Keep unsupported or unknown explicit tiers and unavailable rate combinations unpriced.
- Break each model's usage and cost into Standard, assumed Standard, Fast, and unavailable-tier detail rows while retaining the unsplit model turn count and duration; mark Fast with `⚡` in human output.
- Identify the result as an applied-tier API-list estimate because Codex does not persist the tier actually served.
- Apply each model's embedded Fast premium only to explicit Fast/Priority usage at or after stable `0.144.0`, the first stable release capable of persisting that signal; all missing attribution uses Standard pricing.

## Capabilities

### New Capabilities

- `tier-aware-pricing`: Defines chronological service-tier attribution, context-sensitive price selection, incomplete-price behavior, and pricing provenance.

### Modified Capabilities

None.

## Impact

- Affects rollout analysis and its SQLite cache version, embedded model prices, report aggregation/JSON provenance and model-tier detail, human documentation, and focused pricing/parser tests.
- Existing reports containing usage without attributable tier metadata retain a usable Standard-price estimate and expose the post-release assumed token count instead of making the whole cost incomplete.
- Adds no runtime dependency, network pricing fetch, configuration, or CLI option.
