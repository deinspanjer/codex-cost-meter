# Design

## Context

`pricing/evidence.rs` reads flagship rate rows but its section slice only ends at “Cyber models”; the observed document instead moves into “Multimodal models”. `pricing/proposal.rs` rejects new rate points without a known threshold and gives each observed point its own group. `pricing_command.rs` prints overlapping full evidence excerpts before the overall outcome.

## Goals / Non-Goals

Use the official document's expressed relationship, not a default value learned from earlier models. Limit threshold creation to genuinely new histories: thresholds are model-wide, not dated, so retrofitting a cutoff onto existing rates could reprice the past. Keep ordinary known-model updates and existing custom, tombstone and standalone protections.

## Decisions

1. Recognize the flagship table family as a bounded document region, including the shared short/long legend that follows the Ultrafast table. Require a matching model row in a supported labeled table, known units, and a unique applicable numeric boundary with complementary inequalities in input tokens. Normalize decimal K exactly. Model-qualified prose must not become a shared legend. Unrecognized section structure or attribution fails closed; do not search the whole page for “272K”.
2. Emit a v1 threshold-add (expected absent) and supported point adds under one group ID per model. Derive observations from literal source excerpts, check local preconditions, and keep unsupported tables outside the group. Do not synthesize a threshold-only change. Keep the established observation date/gap policy and existing 30-minute cache-write convention. A repeat run should find no duplicate additions.
3. Preserve both evidence paths. Reuse the same scope check against the actual retrieved document on import, including section boundaries, row membership and cutoff value; isolated snippets selected by a proposal author cannot establish scope. Retain independently fetched citation/excerpt checks and existing supported model-specific threshold evidence. Deterministic collection needs no second fetch.
4. Render validated model groups, not the raw evidence array. Present the decision, the threshold wording and its source (URL/section), component rates with units, affected dates and limitations inline. Show unresolved reasons separately. Full excerpts remain available via existing JSON; no verbosity switch or schema bump is required.

## Risks / Trade-offs

For the November layout, delimit the family by the standalone `Flagship models` marker, its initial units declaration and the following product block with a fresh units declaration. Require positive end evidence; neither EOF nor a search across an intervening product marker qualifies. Batch/Flex and promotional headings can occur inside the family but do not supply Standard/Fast rows. A `<272K` qualifier inside a legacy model cell is not a competing shared legend.

Pass verified scoped facts derived from the fetch cache into proposal validation, alongside existing citation results; avoid a second fetch or trusting scope claimed in proposal JSON. For deterministic cutoff adds use observation date basis and persist `observed` plus URL, rather than the current unconditional `researched`. Threshold overrides retain existing precedence; do not silently replace or prune them as part of this change.

- Changed source layout → report partial coverage rather than associate a cutoff with the wrong table.
- Promotions and missing history → show them separately; “eligible” describes the supported group, not a complete audit or a verified launch date.

## Migration

No catalog migration. This change adds requirements to the capability owned by the already-merged `add-local-pricing-refresh`; archive that baseline first when reconciling specs.
