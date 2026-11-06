## Purpose

Convert current pricing evidence or externally researched proposals into validated overlay changes through one review and application pipeline.

## ADDED Requirements

### Requirement: Deterministic refresh emits the common proposal format
Deterministic refresh SHALL collect current official pricing/model Markdown only and emit the same versioned proposal format used for imports. It SHALL use already fetched same-run evidence without a second fetch. After evidence acquisition, all proposals SHALL pass through one validation, summary, partial-result, review, and application pipeline.

#### Scenario: Explicit current rates
- **WHEN** current Markdown unambiguously identifies model, tier, context, component rates, units, and dates
- **THEN** refresh produces typed proposals and verifies them against the collected content without redundant retrieval

#### Scenario: Source unavailable or reformatted
- **WHEN** current sources cannot be fetched or extracted reliably
- **THEN** refresh reports partial unavailable coverage without inventing rates or deleting historical models

### Requirement: Observation dates are disclosed provisional boundaries
A newly observed price without an explicit source date SHALL use the UTC observation date provisionally. Existing prices SHALL remain effective before it; new models SHALL remain unknown before it. Summaries SHALL disclose the potentially mispriced gap. Unchanged rates SHALL NOT create new boundaries; conflicting values at an occupied same-day boundary SHALL remain unresolved.

#### Scenario: Price changed before observation
- **WHEN** a different current price has no explicit cutover date
- **THEN** the proposed point starts on observation day and the summary discloses that earlier usage still uses the old price until researched correction

### Requirement: Proposals use typed operations and local preconditions
The versioned proposal SHALL declare supported catalog/proposal schemas, operations, evidence, and coverage. Each operation SHALL identify add, replace-value, remove, or move, its typed target, exact old effective value or expected absence, replacement as applicable, and evidence/date basis. Preconditions SHALL be checked against the current effective catalog; failed preconditions SHALL leave the operation unresolved rather than reject unrelated valid work.

#### Scenario: Unrelated data changed since research
- **WHEN** a valid proposal targets a point whose old value still matches and an unrelated point has changed
- **THEN** the targeted operation remains eligible without requiring the whole file to match an earlier snapshot

#### Scenario: Target changed since research
- **WHEN** an operation's expected old value no longer matches the effective point
- **THEN** that operation is unresolved with partial status while independent eligible work can proceed

#### Scenario: Invalid proposal structure
- **WHEN** a proposal has an unsupported version, operation kind, or malformed references
- **THEN** processing fails before writing any catalog changes

### Requirement: Imports preserve user-controlled data
Imported operations SHALL leave standalone catalogs and custom targets or effective custom prices unchanged and unresolved. Users MAY change these values by editing their catalog manually.

#### Scenario: Research proposes replacing a penny price
- **WHEN** an imported replacement, removal, move, or later boundary would supersede custom pricing
- **THEN** its group stays unapplied with unresolved status, even with valid official citations

### Requirement: Dependent edits are validated together
Dependent operations SHALL be grouped atomically. The pipeline SHALL validate resulting effective histories, thresholds, and proxy references before accepting a group. Conflicting or overlapping operations SHALL NOT be silently reordered to bypass their preconditions. Ineligible groups SHALL remain unapplied without blocking independent eligible groups.

#### Scenario: Part of a group lacks evidence
- **WHEN** an operation in an atomic group lacks valid source support
- **THEN** the whole group remains unapplied and independent valid groups can proceed with partial status

### Requirement: Moves and removals are explicit overlay operations
Removing an effective embedded point SHALL create a tombstone. Removing an overlay override SHALL reveal any embedded value beneath it and show that effect. Moving an effective point SHALL remove the old effective key and add its replacement together, using a tombstone when necessary. The summary SHALL show before/after dates and values; source absence alone SHALL NOT authorize removal.

#### Scenario: Correct a provisional date
- **WHEN** supported research moves a point from October 26 to October 20 with matching preconditions
- **THEN** the candidate removes the old effective key and writes the replacement without leaving both active

#### Scenario: Remove an override rather than its effective key
- **WHEN** an eligible atomic correction removes a noncustom overlay override above an embedded point
- **THEN** the summary shows the embedded price that becomes effective

#### Scenario: Tombstone an embedded key
- **WHEN** an eligible atomic correction removes an effective point supplied by embedded data
- **THEN** an overlay tombstone prevents it from reappearing on the next invocation

### Requirement: Imported removals accompany supported replacements
Imported point removals SHALL remain unresolved unless an atomic group also supplies a supported replacement in the same model/tier/context history. A move SHALL retain its combined removal/replacement behavior. Every operation in the group SHALL still pass its preconditions and citation checks.

#### Scenario: Current row does not establish historical deletion
- **WHEN** an independent imported removal cites a matching current pricing row
- **THEN** the historical point stays unchanged and the removal is unresolved

### Requirement: Generated redundant points are pruned but custom points persist
Refresh and import SHALL prune generated overlay points only when the merged effective history with and without each removal is identical for every date. Basis `official`, `observed`, or `researched` SHALL identify generated points. Multiple removals SHALL preserve that condition in the resulting candidate. Custom points, absent-basis points, and standalone data SHALL never be pruned. The candidate summary SHALL show pruning.

#### Scenario: Equal values at different dates
- **WHEN** embedded November 1=A and generated overlay November 6=A produce the same effective history with or without the overlay point
- **THEN** refresh/import proposes pruning November 6 despite the differing keys

#### Scenario: Earlier overlay changes the result of removal
- **WHEN** embedded October 1=A, overlay October 15=B, and generated overlay November 6=A are effective
- **THEN** November 6 is retained because removing it would expose B after that date

#### Scenario: Equal custom price
- **WHEN** a custom overlay point equals embedded data
- **THEN** it remains in the file to preserve the user's explicit override

### Requirement: Covered official and observed differences are reported without automatic correction
For official or observed points dated D that differ from embedded effective pricing, refresh/import SHALL report an unresolved conflict only when embedded `as_of >= D`, without proposing correction. Resolution SHALL require researched import or manual editing. Custom, researched, and standalone data SHALL be excluded from this conflict check; no additional metadata SHALL be required.

#### Scenario: Observed price is newer than embedded review
- **WHEN** embedded November 1=A has `as_of` before November 6 and observed overlay November 6=B differs
- **THEN** the observed point remains unchanged and this comparison reports no conflict

#### Scenario: Embedded review covers the differing point
- **WHEN** an official or observed point at D differs from the embedded effective value and embedded `as_of` is on or after D
- **THEN** refresh/import reports an unresolved conflict without proposing removal or replacement of that point
- **AND** the result is partial unless the conflict is resolved by an eligible researched import or a manual edit

#### Scenario: Researched resolution persists
- **WHEN** a researched point differs from embedded pricing and a later refresh/import runs
- **THEN** that point is not reported as a review-date conflict, even when embedded lacks the model
- **AND** it remains eligible for pruning only when removal preserves effective history

### Requirement: Evidence details are verified but not persisted in pricing data
Proposals SHALL carry URL, retrieval time, excerpts, affected facts, and date basis for generated changes. The pipeline SHALL verify these details and expose them in the change summary/JSON result. Catalog writes SHALL retain only short per-point basis and an applicable source URL. Custom values SHALL not be certified merely by retention or imported assertions.

#### Scenario: Apply a researched correction
- **WHEN** a researched operation passes verification and is applied
- **THEN** the result includes its supporting evidence and the catalog stores researched basis and source URL without excerpts or retrieval times

### Requirement: Imported citations require independent retrieval
Imported citations SHALL be independently fetched, including official-page archive captures, and their excerpts matched against conservatively normalized content preserving numbers, dates, units, and labels. Fetch failure, absent excerpt, or unsupported representation SHALL leave affected operations unresolved/unapplied. Imported claims of same-run verification SHALL NOT bypass retrieval.

#### Scenario: Fabricated excerpt
- **WHEN** an imported proposal quotes a rate or date absent from the independently fetched official source
- **THEN** the operation remains unapplied even with --apply

#### Scenario: Citation unavailable
- **WHEN** independent retrieval fails
- **THEN** the result reports the limitation and does not mark the item verified or apply it

### Requirement: The binary generates a full research prompt
`pricing prompt` SHALL emit an offline self-contained full-check prompt with current effective prices, overlay entries marked, catalog path, supported schemas, operation examples, and bundled evidence rules. It SHALL invoke no AI tool and supply no session data. The prompt SHALL direct the user's preferred tool to write a proposal only, never edit the active catalog or apply changes.

#### Scenario: Research a changed source format
- **WHEN** the user generates a prompt after deterministic collection cannot handle a source
- **THEN** the prompt requests a full current/historical check and permits research of relocated or reformatted official sources without needing a saved refresh result

### Requirement: Import supplies proposals to the common pipeline
`pricing refresh --from <proposal.json>` SHALL load a bounded proposal file and skip deterministic extraction. After independent citation retrieval, it SHALL use the common pipeline and current effective-value preconditions. The app SHALL not manage the producing tool, its authentication, or its files.

#### Scenario: User-managed scheduled research
- **WHEN** a user's scheduler successfully produces a fresh proposal with their preferred tool and invokes import with --apply
- **THEN** the app validates and applies eligible changes without prompting and reports activation and coverage

### Requirement: Review modes are explicit and consistent
Bare refresh SHALL display validated before/after changes and prompt on a terminal; without a terminal it SHALL fail before work. --dry-run SHALL display changes without prompting/writing; --apply SHALL apply eligible changes without prompting. These flags SHALL be mutually exclusive. Summaries SHALL include effective dates, historical changes, basis, gaps, pruning, and unresolved coverage as applicable.

#### Scenario: User declines the proposal
- **WHEN** the user rejects the displayed changes
- **THEN** the external file remains unchanged and the result reports cancellation

#### Scenario: No terminal and no mode
- **WHEN** refresh runs without a terminal or an explicit application mode
- **THEN** it fails with instructions to select --dry-run or --apply

### Requirement: Outcomes distinguish partial work from errors
The pipeline SHALL return 0 for complete outcomes or cancellation, 1 for operational failure, 2 for usage errors, and 3 for partial results. JSON SHALL independently report activation and coverage completeness without prompt contamination. Well-formed unsupported evidence and failed preconditions SHALL yield partial status, including when independent changes are applied.

#### Scenario: Some operations are stale
- **WHEN** one operation fails its old-value precondition while another is applied
- **THEN** the command returns 3 and reports both the applied change and unresolved operation

### Requirement: Pricing updates remain bounded and separate from session mutation
The pipeline SHALL bound fetching and proposal processing, preserve unknown-rate semantics, and avoid session/title writes or scheduler installation. Catalog-only changes SHALL reuse cached usage facts. Explicit title-repricing controls SHALL remain unchanged.

#### Scenario: Cached report after update
- **WHEN** a valid overlay update changes effective prices
- **THEN** the next report recomputes cost from cached usage without requiring a parser-cache reset
- **AND** the update itself has not modified session storage or titles
