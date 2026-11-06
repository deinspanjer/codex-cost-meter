# Proposal

## Why

Model and API-price changes currently require rebuilding the executable. Users should be able to refresh an existing installation on their own schedule while receiving maintenance-release corrections automatically for prices they have not overridden.

## What Changes

- Load one external JSON overlay onto the embedded catalog, once per invocation. Overlay points and explicit removal markers win; optional `base: "none"` selects a standalone catalog with no embedded fallback.
- Keep custom values, including penny pricing, and an optional user label. Store only a short per-point basis and source URL when applicable.
- Use one proposal pipeline: deterministic current-Markdown extraction creates proposals in-process; external research tools return the same versioned format for import. Validation, review, partial outcomes, and activation are shared.
- Generate a self-contained full-check prompt with the effective catalog and overlay entries marked. Users run it in their preferred tool, then use `pricing refresh --from <proposal.json>`.
- Check operation preconditions against current effective values. Verify imported citations by independent fetching; same-run deterministic evidence needs no second fetch.
- Support bare refresh/reset with a prompt, `--dry-run` without prompting/writing, and `--apply` without prompting. Reset removes the external file.
- Write a same-directory temporary file, check destination bytes immediately before rename, and retain no backups. Accept the concurrent-update race between check and rename.
- Deliver offline overlay loading first, deterministic extraction and application second, and prompt/import third.

## Capabilities

### New Capabilities

- `local-pricing-catalog`: Offline overlay/standalone loading, custom values, simple identity and basis, compatibility, and file lifecycle.
- `pricing-evidence-refresh`: Evidence-to-proposal conversion, explicit history operations, research handoff/import, common validation, review, and application.

### Modified Capabilities

- `tier-aware-pricing`: Use effective prices and identify embedded, embedded + overlay, or standalone data, including custom basis and optional label.

## Impact

Rust catalog loading, CLI, rates/reports/exports/title valuation, bounded HTTP fetching, proposal processing, and user/developer guidance. Keep one crate and ordinary valuation offline. No app-managed AI invocation, side-car publication, scheduling service, backups, automatic schema migration, or new accounting semantics. The separate `add-credit-cost-metric` proposal remains untouched.
