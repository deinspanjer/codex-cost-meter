## Context

See `proposal.md` for motivation and `specs/model-proxy-pricing/spec.md` for the behavior contract.

`Catalog` currently stores `proxies` as `HashMap<String, String>`. Pricing resolves that one target before selecting an event-date rate. `undated_proxies` is a special case that forces selected aliases to the target's newest rate even when the usage event is dated; `codex-auto-review` uses that path today. Reports expose only a flat alias-to-target map.

The evidence establishes a launch target and an announced migration date, but not an exact per-account serving or billing cutover. The price model therefore needs a reviewable estimator rule without turning it into billed fact.

## Goals / Non-Goals

**Goals:**

- Select proxy targets and target rates using the same usage-event date.
- Preserve simple static aliases without special-case code.
- Keep the announcement-date uncertainty visible in human and structured provenance.
- Make future proxy-target changes data-only when the evidence supplies a new boundary.

**Non-Goals:**

- Claiming the server-routed model, API invoice SKU, or ChatGPT allowance formula.
- Inferring account-specific rollout cohorts from local records.
- Fetching a mutable model catalog or pricing source at runtime.
- Changing token attribution, service-tier handling, or analysis-cache data.

## Decisions

### Store one ordered history per proxy

Change each `proxies` value from one target string to an ordered list of proxy points:

```json
{
  "proxies": {
    "gpt-5.6": [
      {"target": "gpt-5.6-sol"}
    ],
    "codex-auto-review": [
      {"target": "gpt-5.4"},
      {"effective_from": "2026-07-30", "target": "gpt-5.6-luna"}
    ]
  }
}
```

Only the first point may omit `effective_from`; it is the baseline for earlier events. Later points require strictly increasing dates. A one-point baseline replaces today's static proxy without adding a second schema or keeping `undated_proxies`.

Keeping separate static and historical proxy maps was rejected because it creates two lookup and validation paths. Copying GPT-5.4 and Luna rate cells into a synthetic `codex-auto-review` model history was rejected because duplicated rates would drift from their source histories.

### Resolve the target before the target rate

For dated usage, select the newest proxy point whose boundary is not after the event date, falling back to the undated baseline. Then run the existing target history, tier, token-component, and long-context selection with that same event time. For missing event time, select the last proxy point and the target's last rate, matching existing catalog fallback behavior.

Hard-coding July 30 in Rust was rejected because the boundary is pricing evidence and belongs beside the catalog source. Using the catalog capture date was rejected because it would knowingly apply Luna too late. Treating all historical Auto-review usage as unpriced was rejected because the launch and migration evidence support a useful bounded estimate.

### Preserve flat metadata and add structured history

Keep the existing structured `model_proxies` map as the latest/default target for compatibility, and add a structured `model_proxy_histories` map containing target and optional effective date. Human output renders static aliases once and dated histories as separate before/from lines, followed by the announcement-date qualification.

Replacing `model_proxies` with arrays was rejected because existing structured consumers can retain the current field while opting into the new history. Encoding dates inside the existing target string was rejected because downstream consumers would have to parse prose.

### Treat July 30 as a date-level estimator boundary

The current catalog supports date precision, and the public evidence supplies an announcement date rather than a UTC timestamp. Use 2026-07-30 as the first Luna date. Documentation and output must call this an announcement-date proxy; they must not imply that every surface or account changed at midnight UTC.

Adding rollout-surface or authentication dimensions was rejected because local usage events do not provide authoritative routing evidence for such distinctions.

## Risks / Trade-offs

- [A staged migration may make some July 30 events select the wrong target] → Label the boundary as an estimate and keep it data-driven so better evidence can replace it.
- [The retained flat metadata shows only the latest target] → Add the structured full history and render that history in human output.
- [A future server-side alias change may not be announced] → Preserve the existing unpriced/approximation language; update only from reviewable evidence.
- [Catalog schema changes can reject old test fixtures] → Update the small inline fixtures mechanically and retain focused validation cases.

## Migration Plan

1. Change catalog parsing and validation, then convert the two embedded proxy entries to one-point or dated histories.
2. Add boundary, missing-time, static-alias, and invalid-history checks around pricing lookup.
3. Add structured and human proxy-history provenance while retaining the flat latest-target field.
4. Update the worked example and remove the temporary documentation warning about timeless Luna pricing.
5. Run focused tests, `cargo fmt --check`, `cargo test`, strict OpenSpec validation, and `git diff --check`.

Rollback uses the previous binary and catalog together. No cache migration is required because cached analysis stores attributed usage, not calculated prices or proxy resolution.
