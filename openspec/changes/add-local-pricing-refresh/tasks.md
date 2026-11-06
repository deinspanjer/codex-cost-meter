# Tasks

Deliver Stage A independently before adding network or proposal application. Keep this change open until all stages are complete; each stage includes its own tests and documentation.

## 1. Stage A — offline overlay loading and validation

- [x] 1.1 Define the external schema for keyed price differences, tombstones, threshold/proxy overrides, display version, optional label, per-point basis/source, and standalone base; verify penny pricing, missing-basis custom default, unknown cells, and invalid schema/point/reference rejection.
- [x] 1.2 Implement one-time user-path loading and embedded-plus-overlay merge with overlay precedence and standalone exclusion; verify reports across multiple Codex homes share one snapshot, unoverridden prices follow maintenance updates offline, tombstones remain effective, and standalone models do not inherit new embedded prices.
- [x] 1.3 Integrate effective pricing into rates, reports, exports, and title valuation with mode/version/label/basis metadata; verify cached synthetic usage reprices without reanalysis, replacement missing cells stay unknown, and custom prices are not presented as official.
- [x] 1.4 Document overlay/standalone editing, precedence, effective rates inspection, basis, labels, and incompatible-file deletion/regeneration in USERS.md and docs/model-pricing.md; verify examples with temporary homes and confirm no network is required.
- [x] 1.5 Run focused Stage A checks, `just check`, and `git diff --check`; record results and a release-ready changelog entry without claiming later-stage commands are implemented.

## 2. Stage B — deterministic extraction to proposals and application

- [x] 2.1 Define proposal format version 1 with typed add/value/remove/move operations, exact old effective-value/absence preconditions, atomic groups, evidence/date bases, and coverage; verify valid proposals and rejection of malformed structure, unknown versions/kinds, and invalid references.
- [x] 2.2 Implement bounded current official Markdown fetching/extraction that emits the proposal format in-process; verify explicit rate extraction and partial failure on missing labels, moved pages, or network errors, using same-run evidence without a second fetch.
- [x] 2.3 Implement provisional observation-date points and gap disclosure; verify unchanged rates avoid new boundaries, new models stay unknown before observation, old rates remain before observation, and same-day differing values are unresolved.
- [x] 2.4 Implement the common operation-validation/candidate pipeline against current merged prices; verify a stale operation does not block unrelated valid work, dependent groups stay atomic, embedded removals produce tombstones, overlay removal reveals embedded values, and moves remove old effective keys without overwriting custom conflicts.
- [x] 2.5 Implement generated-point pruning only when effective history is identical for every date, plus unresolved official/observed differing-value conflicts gated by existing embedded as_of; verify November 1=A/November 6=A is pruned, October 1=A/overlay October 15=B/November 6=A retains November 6, observed November 6=B over November 1=A with older as_of remains unchanged without conflict, and covered differing values report conflict without a proposed correction. Verify combined removals preserve history, custom/standalone data remain untouched, and compact basis/source persistence excludes excerpts/retrieval times while results retain them.
- [x] 2.6 Add refresh/reset bare prompt, --dry-run, --apply, JSON output, and exits 0/1/2/3; reset removes even an invalid external file. Verify nonterminal refusal, decline, no-op reset, partial application, and uncluttered JSON stdout.
- [x] 2.7 Implement same-directory temp write, final destination byte/absence check, and std::fs::rename replacement without an extra dependency; verify detected changes and write errors preserve the destination, and Windows CI verifies replacement of an existing file. Document the accepted concurrent check-to-rename race and analogous reset race rather than asserting serialized updates.
- [x] 2.8 Add fixed-prefix temp cleanup older than one hour on startup and handled-failure cleanup; verify old files are removed while recent temps, active data, and user proposal files remain. Keep no backups or persistent evidence downloads.
- [x] 2.9 Document current-data observation policy versus researched maintainer updates, proposal operations, basis, effective-history pruning, CLI/reset modes, partial outcomes, and explicit title repricing; disclose that generated points may mask later embedded data until researched/manual removal and that official/observed conflicts are surfaced only when embedded as_of covers the point date; verify temporary-home examples and completions/help, then run `just check` and `git diff --check` with results recorded.

## 3. Stage C — generated full-check prompt and proposal import

- [x] 3.1 Generate pricing prompt from binary-owned evidence rules and the common proposal schema, including the current effective catalog with overlay entries marked; verify dynamic catalog/schema/label data and operation examples are present without session data or permission to edit the active file. Do not add static-prose tests.
- [x] 3.2 Add pricing refresh --from as a bounded input adapter to the existing pipeline, bypassing current-price extraction; verify malformed input fails without writes and proposals with some failed preconditions yield partial results rather than rejecting unrelated valid operations.
- [x] 3.3 Independently fetch imported citations under source/redirect/archive policy and verify exact excerpts after conservative normalization; verify fabricated numbers/dates and failed retrieval remain unapplied, imported same-run claims cannot bypass fetching, and a reformatted official source can support an import without extractor changes.
- [x] 3.4 Document the user-managed research/import scheduling pattern, shared proposal schema, prompt instructions, external-tool permissions/files, and successful-proposal-production gate before import; verify prompt-to-fixture-proposal-to-import in a temporary home using the existing review/application pipeline without launching an AI tool.

## 4. Integrated validation and readiness

- [x] 4.1 Run focused tests as each group lands, then `just check` and `git diff --check`; record results without static-prose or duplicated pipeline tests.
- [x] 4.2 Exercise overlay/standalone loading, maintenance updates, custom labels/basis, observation dates, explicit move/remove/tombstones, generated pruning, stale per-operation checks, import citation failures, reset, and replacement on supported CI platforms; explicitly report any unexecuted platform checks.
- [x] 4.3 Update CHANGELOG.md and durable user/developer architecture/pricing guidance; self-review final behavior against the three delta specs and verify the separate credit/quota proposal is unchanged.

## Workflow follow-up

- Archive only after all stages, required review, and validation are complete; retain durable guidance before removing mechanical planning artifacts under the project's workflow.


## Validation record — 2026-11-06

- `just check` passed: formatting, 224 Rust tests on this macOS host, 13 version-tool tests, and Clippy with warnings denied. Focused catalog, proposal, evidence, storage, command, and isolated CLI checks passed during implementation.
- `git diff --check` and `openspec validate add-local-pricing-refresh --strict` passed. Pricing command help and Zsh completion generation were exercised.
- A temporary-home live-source exercise generated an offline full-check prompt, collected a deterministic proposal, independently re-fetched its citations through `--from`, and applied one eligible observation-derived change. The original custom historical point remained. Import returned exit 3 with `activation: applied` because unsupported current coverage remained unresolved; no real user catalog or session data was modified.
- Reset/invalid schema, standalone/custom labels, cache repricing, explicit moves/removals, pruning and review-date conflicts, stale atomic groups, bad citations, and final-byte replacement behavior have focused regression coverage. Post-activation output failures explicitly report that the change was already applied.
- The existing Windows CI job runs `cargo test`, including replacement over an existing destination and temporary-home CLI tests. Windows and x86_64 macOS execution were not run on this host; those platform results are not claimed. No release, installation, or title repricing was performed.
- Independent implementation review identified citation-fact validation and post-activation output-error reporting issues; both were fixed and rechecked. The separate credit/quota proposal was not modified.

Current deterministic coverage is intentionally disclosed as partial when older tables/model-tier-context combinations, new-model context thresholds, or retained proxy/threshold histories are not freshly verified. User-managed research/import can supply evidence-backed corrections; incomplete evidence is never described as a full update. The change remains unarchived for the normal archive workflow.


## Review correction

The review-date conflict rule excludes researched points so researched resolutions persist across invocations; history-preserving pruning still includes researched points. The previous temporary-home exercise demonstrated an unwanted current-price takeover of a custom point; regression tests now require deterministic refresh to leave custom, standalone, and tombstoned cells unresolved instead.

Review-fix validation: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (236 passing Rust tests), `git diff --check`, and strict OpenSpec validation passed. New regressions cover user-controlled cells, repeated researched resolutions, existing short-only models, exact scoped citation facts, HTML/quote boundaries, decoded response bounds, failed-fetch caching, duplicate source rows, and best-effort cleanup. HEAD remains 7565158; no commit, merge, or rebase was performed. Durable user/pricing documentation was left untouched under the review handoff's file-scope restriction; its conflict wording needs the same official/observed clarification before merge.

Import re-review fixes: all imported operations now preserve custom targets/effective custom points and standalone catalogs. Point removals require a supported replacement in the same model/tier/context atomic group; every group member still passes evidence and preconditions. Regression coverage includes replacement/removal/move/later-boundary custom protection, standalone protection, independent removal rejection, different-group rejection, successful paired correction, and atomic failure on missing evidence. Required checks passed: formatting, all-target Clippy, 238 Rust tests, diff whitespace, and strict OpenSpec validation. No commit, merge, or rebase.
