## Purpose

Provide user-controlled pricing differences over embedded data, or a standalone custom catalog, with one consistent offline snapshot and a simple external-file lifecycle.

## ADDED Requirements

### Requirement: Each invocation merges the external overlay once
The system SHALL merge a compatible external overlay onto embedded prices once per invocation and use that effective snapshot for all valuation. Overlay values and tombstones SHALL win on the same key. Without an external file the system SHALL use embedded data alone. Ordinary valuation SHALL remain offline.

#### Scenario: Consistent override across consumers
- **WHEN** an overlay replaces a price at an exact model, tier, context band, and effective date
- **THEN** reports, rates, exports, and title valuation use that replacement from one snapshot
- **AND** later file changes cannot mix effective catalogs within the running invocation

#### Scenario: Maintainer updates an unoverridden price
- **WHEN** a new application release changes an embedded key absent from the overlay
- **THEN** the next invocation uses the corrected embedded value without network access or an external-file update

#### Scenario: Explicit removal survives an upgrade
- **WHEN** the overlay tombstones an embedded point
- **THEN** that point is absent from the effective catalog even if a later embedded catalog still contains it

### Requirement: Generated points preserve effective history until justified removal
Generated overlay points SHALL retain normal precedence. Refresh/import pruning SHALL preserve merged effective values for every date; disagreement with embedded data alone SHALL NOT remove a point. Review-date-covered disagreements for official and observed points SHALL be surfaced by refresh/import without changing prices, as specified by the evidence-refresh capability.

#### Scenario: New observation over older reviewed data
- **WHEN** embedded November 1=A has `as_of` before November 6 and observed overlay November 6=B is loaded
- **THEN** B remains effective from November 6 and refresh/import neither removes it nor reports a conflict merely from that comparison

#### Scenario: Redundant-looking point restores an embedded value
- **WHEN** embedded October 1=A, overlay October 15=B, and generated overlay November 6=A are loaded
- **THEN** the effective history returns to A on November 6 and refresh/import retains that point because pruning would expose B

### Requirement: Standalone mode excludes embedded prices
An external file with `base: "none"` SHALL provide the entire effective catalog without embedded fallback. Models or components absent from that file SHALL remain unknown. Standalone data SHALL NOT be pruned merely because it equals embedded data.

#### Scenario: Custom whole-catalog pricing
- **WHEN** a standalone catalog gives all its listed price cells a penny rate and a new release adds an embedded model
- **THEN** existing listed models keep their custom rates and the newly embedded model remains unpriced until added to the standalone file

### Requirement: Custom values require structure but not official evidence
The system SHALL accept finite nonnegative custom rates without official citations or equality to official prices. It SHALL validate supported schema, keyed points, dates, thresholds, and effective proxy references. Null or absent optional rates SHALL remain unknown. Missing basis SHALL default to custom.

#### Scenario: Valid penny overlay
- **WHEN** a structurally valid overlay replaces listed rate cells with 0.01 USD per million tokens and no citations
- **THEN** the effective prices use those custom values

#### Scenario: Replacement has unknown components
- **WHEN** a replacement point omits an optional rate component present in the embedded point
- **THEN** that component is unknown rather than silently inherited

#### Scenario: Invalid effective references
- **WHEN** applying an overlay would leave a proxy pointing to an unavailable target
- **THEN** loading fails before valuation or title mutation with an actionable error

### Requirement: Catalog identity and basis remain compact
The external file SHALL support a simple display version, optional user label, and per-point basis `official`, `observed`, `researched`, or `custom`, with a source URL when applicable. Excerpts and retrieval times SHALL NOT be stored in the catalog. Reports SHALL identify embedded, embedded + overlay, or standalone mode and distinguish custom from official or inferred pricing. Labels SHALL be terminal-safe.

#### Scenario: Labeled overlay
- **WHEN** the user supplies label `My penny pricing`
- **THEN** human output includes it alongside display version information and JSON exposes it separately
- **AND** the label changes neither schema validation nor calculation

### Requirement: Incompatible files are not automatically migrated
Unsupported external schemas SHALL fail without partial interpretation or automatic mutation. The error SHALL explain deletion or regeneration. Removing the external file SHALL return subsequent invocations to embedded prices.

#### Scenario: Incompatible data after upgrade
- **WHEN** the running application cannot interpret the external schema
- **THEN** it reports the file and supported schema and directs the user to delete or regenerate the file
- **AND** the incompatible file remains untouched

### Requirement: Reset removes the external file
Bare `pricing reset` SHALL display the effect and prompt on a terminal; without a terminal it SHALL fail. `--dry-run` SHALL show the effect without prompting or writing; `--apply` SHALL remove the file without prompting. Reset SHALL work for invalid external files, retain no backup, and treat an absent file as a no-op.

#### Scenario: Reset a standalone or invalid file
- **WHEN** the user applies reset
- **THEN** the external file is removed and future pricing uses embedded data
- **AND** the summary makes clear that custom data is discarded without a backup

### Requirement: Replacement uses a temp file and final byte check
Application SHALL write a same-directory temporary file, compare destination bytes or absence immediately before replacement, and rename the temp file over the destination. Detected changes SHALL stop application. The system SHALL document the accepted lost-update race between check and rename. Reset SHALL likewise compare bytes before deletion, with the same accepted check-to-action race.

#### Scenario: File changes before the final check
- **WHEN** the destination differs from the bytes captured for the invocation
- **THEN** application stops without overwriting that change

#### Scenario: Concurrent updaters pass the check
- **WHEN** two updaters both check before either replaces the destination
- **THEN** the last replacement can win as a documented limitation rather than a guaranteed conflict rejection

### Requirement: Temporary files have simple age-based cleanup
The system SHALL retain no automatic backups or evidence archives. It SHALL clean its temp file on handled completion/failure and delete leftover same-directory files with the fixed temp prefix older than one hour on startup. It SHALL leave recent temp files and user-saved prompts/proposals untouched.

#### Scenario: Startup after interrupted write
- **WHEN** a prior run left an old temp file matching the app's prefix
- **THEN** a later startup removes it once older than one hour without removing the active catalog or user artifacts
