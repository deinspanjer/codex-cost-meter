## 1. Shared Presentation Primitives

- [x] 1.1 Expose and reuse canonical title-suffix stripping for report identity, add short-ID and project-display fallbacks, and verify focused tests preserve noncanonical user titles while removing canonical meter suffixes.
- [x] 1.2 Add compact scope/group row formatting for nested cached-input and reasoning-output values plus conditional incomplete counts, and verify focused output tests cover complete and incomplete rows.
- [x] 1.3 Add reduced model-row formatting and a centralized model-section visibility predicate, and verify focused tests cover one ordinary model, assumed Standard, Fast-only, mixed Standard/Fast, unavailable tier, multiple models, and zero-turn legacy usage.

## 2. Exact-Rollout Human Reports

- [x] 2.1 Replace the exact-rollout header with cleaned name/type, short identifier, project display name, rollout count, and turn-frequency model/effort labels; verify root, unnamed, short-ID, and directly selected non-root fixtures.
- [x] 2.2 Render Root and conditional All agents rows with the compact scope table and overlap qualification, and verify isolated, clustered, incomplete, and parallel-duration fixtures.
- [x] 2.3 Remove duplicate model totals and redundant ordinary single-model sections while retaining exceptional tier evidence, and verify exact-report snapshots or assertions cover small, medium, and large shapes.

## 3. Project, Corpus, and Grouped Human Reports

- [x] 3.1 Replace project headers with project, included-root task count, rollout count, and range while reducing resolver diagnostics to material nonzero qualifications; verify lifetime, bounded, incomplete-root, partial-cost, and exclusion fixtures.
- [x] 3.2 Replace corpus headers with scope, rollout count, and range without root-only model/effort metadata; verify lifetime and bounded corpus fixtures.
- [x] 3.3 Apply compact tables to rollout-type and grouped sections without duplicate totals, and verify day/type and day/model fixtures including zero-turn attributed usage.
- [x] 3.4 Render one terminal empty-state sentence and omit empty usage/pricing sections; verify project, corpus, bounded-range, and included-empty-bucket behavior remains intentional.

## 4. Pricing, Help, and Progress

- [x] 4.1 Replace full human pricing provenance with the catalog-date estimate line and conditional assumed-tier, incomplete-input, and lower-bound warnings; verify complete, assumed Standard, unsupported-tier, and partial-cost reports.
- [x] 4.2 Remove model-proxy histories and static methodology from ordinary human reports while preserving structured JSON fields, and verify existing JSON proxy/tier serialization tests remain unchanged.
- [x] 4.3 Expand `report --help` with nested-column, turn-time, pricing-tier, partial-cost, proxy, source, and JSON-provenance guidance; verify the rendered help manually through `cargo run -- report --help` without adding prose-only unit assertions.
- [x] 4.4 Clear the owned interactive progress line before final output while preserving forced redirected progress, and verify focused progress tests cover both terminal and non-terminal paths.

## 5. Documentation and Validation

- [x] 5.1 Update README and user-guide examples for exact, project, corpus, grouped, empty, incomplete, and partial-cost output, and verify documented labels and examples match the executable.
- [x] 5.2 Record or regenerate representative 120- and 160-column terminal evidence for small, medium, and large isolated roots and clusters plus project, corpus, grouped, non-root, and empty reports; verify no ordinary row wraps or loses material qualification.
- [x] 5.3 Run formatting, focused tests, the broad repository test suite, linting, and strict OpenSpec validation; report every command and result before marking the change complete.
