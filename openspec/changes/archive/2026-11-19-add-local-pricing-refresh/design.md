# Design

## Context

See proposal.md for motivation. Pricing currently embeds `data/model-prices.json`; reports recompute costs from cached usage facts. Tier interpretation and token accounting remain Rust behavior. The existing maintainer procedure in docs/model-pricing.md governs researched releases; local current-data refresh needs an explicitly disclosed observation-date policy.

## Goals / Non-Goals

**Goals:** Small user-controlled differences over embedded prices, one effective snapshot per run, one proposal-processing pipeline, and optional research through the user's preferred tool.

**Non-Goals:** App-managed AI execution, persistent evidence downloads, backups, automatic schema migration, new accounting dimensions, general price expressions, or credit/quota updates.

## Decisions

### External overlay and standalone mode

Use `~/.codex-cost-meter/model-prices.json` in the platform user's home, independent of Codex storage homes and working directory. Missing file means embedded prices. Otherwise read it once and merge onto embedded data once; all reports, rate tables, exports, and title valuation use that snapshot. Normal valuation performs no network access. The existing `rates` command exposes the effective table; do not add a duplicate rate command.

By default the file contains only differences: added/replacement history points keyed by exact model, tier, context band, and effective_from; explicit removal markers (tombstones) for embedded points; threshold and proxy overrides. Overlay values win on identical keys. A replacement point supplies its permitted rate fields as a unit: absent/null optional components remain unknown, not silently filled from the overridden point. Reject duplicate/conflicting point-and-tombstone keys. After applying overrides and removals, validate the complete effective history, thresholds, and proxy references.

Top-level `base: "none"` means the file is a standalone complete catalog and embedded data contributes nothing. This supports whole-catalog custom pricing and prevents later releases from importing official prices for new models into that custom catalog. Models the standalone file does not list stay unknown. Users can set every listed price to a penny without official evidence. Missing basis defaults to custom. This is not a wildcard rule for automatically inventing prices for unknown models.

Maintenance releases automatically change every unoverridden embedded key at the next invocation, offline. Overridden keys and tombstones continue to win. During refresh and import, a generated overlay point is redundant only when the merged effective history with and without that point is identical for every date; only such points are pruned. This includes redundant points at different dates, not just identical keys. Custom points are never pruned. Generated means basis official, observed, or researched; users who hand-edit a generated point should mark it custom if they want it retained. Do not infer edit history from prices. Do not prune standalone points against embedded data, since that would delete effective prices. Tombstones and explicit overrides survive unless an explicit operation removes them.

### Small metadata and basis

The external file has schema_version, a simple display version, optional label, optional base, and its points/removals/overrides. No separate data-version ordering is needed. For embedded + overlay output, expose the embedded and external display versions without constructing an archival identity. Sanitize labels for terminal display; preserve separate JSON fields.

Each external pricing point or override/removal stores a short basis: official, observed, researched, or custom, and a source URL where applicable. Official identifies an explicit current-source fact, observed a provisional observation-date boundary, researched an evidence-supported interpretation/historical correction, and custom user-supplied values. The app derives generated basis from the accepted proposal rather than accepting an imported verification assertion. Basis is descriptive metadata, not a signature or proof against later user editing.

Excerpts, retrieval times, and date reasoning are used during proposal verification and appear in the human change summary/JSON result as appropriate; they are not persisted in the catalog. Raw fetched sources are temporary. User-saved prompts, proposals, results, and backups remain user-managed. Ordinary footers/JSON distinguish official, observed/researched, and custom pricing without repeating evidence excerpts.

Structurally validate finite nonnegative rates, ordered effective dates, supported fields/schema, context thresholds, and valid proxy targets. Missing components remain unknown. Invalid or incompatible files cause actionable errors before valuation or title mutation. The error tells users to delete or regenerate the external file; no partial interpretation or automatic migration. Keep reset usable to remove an invalid file without parsing it as a valid catalog.

### One proposal pipeline

Define proposal format version 1 independently from catalog schema and display version. The envelope declares proposal_schema_version, catalog_schema_version, operations, evidence, and coverage/unresolved items. Each operation has an ID, typed target, kind (add, replace-value, remove, move), exact expected-old effective value or expected absence, replacement date/value as applicable, evidence references, and date basis. Related edits use atomic groups so partial application cannot split dependent operations. Reject malformed structure, unsupported versions/kinds, or invalid references before writes.

Deterministic extraction produces this exact format in-process. Its citations use the source content just fetched in the same run; verify against those bytes without re-fetch. Imported proposals use independently fetched source content. After source acquisition, the same operation validation, candidate construction, change summary, review/application modes, partial-result handling, generated-point pruning, and final write apply. Imported claims of a same-run fetch cannot bypass independent verification; the command knows whether it collected or imported the proposal.

Evaluate old-value/absence preconditions against the current merged catalog, not an earlier full-file snapshot. A changed unrelated price does not reject an entire proposal. Imported operations targeting custom data, inheriting an effective custom point, or targeting standalone catalogs are unresolved and require manual editing. An operation with a failed precondition is unresolved; independent eligible groups remain applicable with partial status. Conflicting or overlapping edits must not be silently ordered to satisfy each other. Validate a group's resulting effective history and proxy references before accepting it.

Translate accepted operations to overlay changes. Adding/replacing creates a point at the target key. Removing an embedded point requires a tombstone. Removing an overlay point deletes it; if an embedded value exists underneath, deleting the override reveals that value and the summary must show it. To remove the effective key entirely, the operation must explicitly request removal of the effective point, producing a tombstone when needed. Typed targets distinguish overlay-point removal from effective-point removal; both check the old effective value. Moving a point removes the old effective key (with a tombstone if embedded) and writes the replacement at the new key as one operation; the old key must not reappear. Explicitly dropping a tombstone reveals the underlying embedded point and is shown in the diff. Custom destination conflicts and invalid references remain unresolved. An imported point removal must accompany a supported replacement for the same model/tier/context history in its atomic group; moves already combine those halves. Independent point removals remain unresolved for manual editing. Absence from live documentation never itself authorizes deletion.

Generated-point pruning happens only during refresh/import and is included in the candidate summary. Compare merged effective histories with and without each candidate point over every date, retaining all rate components and unknown-value behavior. Evaluate removals against the resulting candidate so several individually redundant points cannot be removed together if that would change effective history. Never prune custom or standalone data. For example, embedded November 1=A with overlay November 6=A permits pruning; embedded October 1=A, overlay October 15=B, overlay November 6=A does not, because removal would reveal B.

For an official or observed point dated D that differs from the embedded effective value at D, report an unresolved conflict when the embedded catalog's existing `as_of` review date is on or after D. This comparison proposes no removal or replacement, including for observed points; resolution requires researched proposal import or manual editing. When embedded `as_of` precedes D, emit no conflict from this check, because the embedded data is older. Custom and researched points and standalone catalogs are excluded from this conflict check; researched points remain eligible for history-preserving pruning. A researched import is a durable resolution, including on subsequent runs. Use existing `as_of` only, with no additional metadata. Persist only differences and short basis metadata. Known historical corrections need supporting evidence; current evidence alone must not silently rewrite an older rate.

### Commands, review, and outcomes

- `pricing refresh` collects current data, prepares the proposal/diff, and asks whether to apply on a terminal. Without a terminal, fail before collection and explain the explicit modes.
- `--dry-run` displays validated changes without prompting or writing; `--apply` displays and applies eligible validated changes without prompting. They are mutually exclusive, with no extra confirmation flag.
- `pricing refresh --from <proposal.json>` reads a bounded proposal file once, skips current-price extraction, and enters the same pipeline after independently fetching its citations. Bare/dry-run/apply modes are unchanged.
- `pricing prompt` writes a self-contained full-check prompt to stdout offline. It includes the current effective catalog with external entries/tombstones/overrides marked, catalog path, supported schemas, and evidence rules. It does not write the catalog or invoke a tool.
- `pricing reset` uses the same three modes and removes the external file, returning to embedded data. Show that custom/standalone data will be discarded; no backup is retained. If absent, report a no-op. Reset compares the bytes/absence captured for review immediately before deletion; it has the same accepted check-to-action race as replacement.
- `--json` emits structured changes, coverage, unresolved items, and activation outcome. In interactive mode, review/prompt goes to stderr so stdout remains one JSON result.

Exit 0 means complete applied/unchanged/dry-run results or user cancellation, 1 operational failure, 2 CLI usage error, and 3 partial results (including partial application). JSON independently states whether activation occurred and coverage completeness. Missing/malformed proposal files fail with 1; usage errors need not emit JSON. Unavailable collection or unsupported well-formed evidence yields partial coverage without invented changes. A final write failure is operational failure even if the candidate had eligible changes. Never describe failed collection as a complete successful refresh.

### Current-data extraction and imported citation checks

Use bounded HTTP fetching with request/whole-run timeouts and response-size limits, through a maintained cross-platform client. Deterministic extraction reads only current official pricing/model Markdown, not HTML/changelog/Wayback research. Require labeled model/tier/context/component/unit cells. Do not infer model aliases, global speed multipliers, discounts, or a promotion's end from names or minimum durations. Model/tier/accounting dimensions the binary cannot represent remain unresolved.

When new current prices lack an explicit effective date, propose a provisional observation-date boundary using the UTC retrieval date. An existing model keeps its old price before observation; disclose the potentially mispriced gap before that date. New models stay unknown before observation. Unchanged prices do not create new date points. A differing rate at an already occupied same-day boundary is a conflict, not permission to overwrite it. Historical research can later support a move/removal through proposal import. During implementation, document this current-data policy separately from maintainer-release historical research in docs/model-pricing.md.

Independently fetch every imported citation URL, including exact captures of official pages in archives. Require cited excerpts to occur in fetched content after conservative whitespace/entity/markup normalization; preserve numbers, signs, dates, units, and relevant labels. Check redirects and archive original-page identity against the official-source policy. HTML-to-visible-text normalization verifies quotations; it does not extract new prices. A missing excerpt, failed fetch, or unsupported representation leaves affected operations unresolved/unapplied, including unattended apply. Matching text supports the citation but does not prove the interpretation. Treat source content as evidence, never executable instructions. Same-run deterministic citations are checked against already fetched bytes using the same evidence rules.

### Generated research prompt

Bundle the evidence rules and proposal schema in the binary so an installed app requires no source checkout. The prompt always requests a full check of the current effective catalog, with overlay entries marked. Include examples of each operation, exact old-value/absence preconditions, atomic groups, required URLs/retrieval times/excerpts/date bases, current-data limitations, and instructions to leave unsupported facts unresolved. Supply no session/rollout data. The user selects their preferred harness, authentication, permissions, and proposal output path.

The prompt explicitly directs the tool to write a proposal file only, not edit model-prices.json or apply changes. Direct catalog edits remain supported manual customization, outside the validated workflow. Source-format/location changes can be researched by that tool and imported without requiring the deterministic extractor to parse the new format. Prompt generation needs no prior refresh result.

Document the user-managed scheduling pattern: generate a prompt and run the chosen tool non-interactively to a fresh proposal; only after successful proposal production run `pricing refresh --from <proposal.json> --apply`. Inspect exit/JSON outcomes. The app neither orchestrates these steps nor manages the external tool's lifecycle/files. A stale operation becomes unresolved through its precondition; unrelated valid work can still apply.

### Temp-file replacement and cleanup

Write the validated external file to a unique same-directory temp file with fixed prefix `.model-prices.tmp-`, finish writing/close it, compare destination bytes (or absence) with those captured at invocation, then call `std::fs::rename`. A mismatch stops application; do not remove the destination before rename. On supported Windows filesystems rename replaces an existing file; verify actual replacement in Windows CI. Use the standard library, no extra replacement dependency. Reference: [Rust rename documentation](https://doc.rust-lang.org/std/fs/fn.rename.html).

Accepted residual: two updaters can both pass the byte check before either rename, so the last rename can overwrite the other update. Atomic replacement prevents a torn file, not lost concurrent updates. The same residual applies between reset's check and deletion. Do not promise stronger concurrency protection.

Remove the invocation's temp file on handled failure/completion. On startup, delete same-directory files with that fixed prefix older than one hour; leave recent files alone. Do not create a directory just to clean it, or delete user prompt/proposal files. No backup copies or permanent evidence downloads are retained. A crash may leave a temp file until a later startup; age-based cleanup is intentionally simple.

## Risks / Trade-offs

- Generated points can mask later embedded prices until removed through researched import or manual editing. Refresh/import prune only points whose removal preserves the effective history for every date, and surface differing-value conflicts for official/observed points only when embedded `as_of` covers the point date; an older review date leaves this masking undisclosed by the conflict check. Custom overrides remain deliberate user choices.
- Current Markdown changes → report partial coverage and let user-run research supply a proposal; unknown accounting semantics still need a binary update.
- Imported prose can misinterpret valid citations → expose basis and changes, reject unsupported evidence, and offer review without making it mandatory.
- Temp-file check/rename permits a lost-update race → accept and document it; retain Windows replacement verification.
- Standalone catalogs receive no new built-in prices → unknown models remain unknown until users add them, keeping whole-catalog custom pricing under user control.

## Delivery sequence

One change, three independently shippable stages: Stage A supplies offline loading, overlay merge, standalone behavior, label/basis, and validation; Stage B supplies extraction to proposals plus validation/review/application and reset; Stage C supplies prompt generation and external proposal import. Each stage includes its tests/documentation. Keep the change open until all stages finish; no implementation or release is authorized by this planning revision.

## Migration Plan

Existing installations without a file retain embedded behavior. Users opt into overlays or standalone files; later maintenance releases affect unoverridden embedded keys automatically. Incompatible external schemas error with delete/regenerate guidance. Reset/delete returns to the current embedded catalog. New prices are recomputed from cached usage facts; catalog operations do not rewrite titles or install schedules. Existing explicit title repricing controls remain unchanged.
