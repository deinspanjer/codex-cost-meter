## Why

The embedded catalog currently prices every `codex-auto-review` event through GPT-5.6 Luna, including reviewer usage that predates OpenAI's July 30 migration announcement. That materially underprices historical Auto-review usage and presents a timeless alias where the evidence supports a dated model transition.

## What Changes

- Represent model proxies as effective-dated target histories instead of one target plus an undated exception.
- Price `codex-auto-review` through GPT-5.4 before 2026-07-30 and GPT-5.6 Luna on and after that date.
- Use the latest proxy target when an event timestamp is unavailable, matching the catalog's existing latest-rate fallback.
- Expose dated proxy provenance and state that the boundary is an announcement-date estimate, not proof of an account-level serving or billing cutover.
- Replace user documentation that implies `codex-auto-review` has always mapped to Luna.

## Capabilities

### New Capabilities

- `model-proxy-pricing`: Effective-dated model-proxy selection, validation, fallback, and report provenance.

### Modified Capabilities

None.

## Impact

The embedded catalog schema, pricing lookup and validation, structured report metadata, human rendering, focused pricing/output tests, and user/research documentation change. Rollout parsing, token attribution, model rate histories, service-tier pricing, and external dependencies do not change.
