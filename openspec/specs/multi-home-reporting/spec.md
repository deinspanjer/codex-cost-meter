# multi-home-reporting Specification

## Purpose
Lets one read-only report combine usage from several Codex storage directories, so each rollout counts once and task trees and projects resolve across homes, while every command that writes stays limited to one home.

## Requirements

### Requirement: Report accepts a list of Codex homes
The `report` command SHALL accept `--codex-homes HOME[,HOME...]`. Entries are comma-separated. The option MAY be repeated, and repeated occurrences add their entries to the list. When the option is present, the report SHALL read exactly the listed homes and SHALL ignore `CODEX_HOME` and the default home. The option SHALL be available in every report mode.

#### Scenario: Two comma-separated homes
- **WHEN** a user runs `report --all --codex-homes A,B`
- **THEN** the report includes rollouts discovered in both A and B

#### Scenario: Repeated option
- **WHEN** a user runs `report --all --codex-homes A --codex-homes B`
- **THEN** the report is identical to `report --all --codex-homes A,B`

#### Scenario: Explicit homes override the environment
- **WHEN** `CODEX_HOME` names home C and a user runs `report --all --codex-homes A`
- **THEN** the report reads only A

#### Scenario: Same home listed twice
- **WHEN** a user lists one directory twice, either with identical text or with two paths that resolve to the same directory
- **THEN** the report is identical to listing that directory once

### Requirement: Listed homes are validated before reading
The system SHALL reject an empty entry in `--codex-homes` as a usage error. Before reading any rollout, the system SHALL fail if a listed home does not exist or is not a directory, and the error SHALL name that entry.

#### Scenario: Empty entry
- **WHEN** a user passes `--codex-homes A,,B`, `--codex-homes A,`, `--codex-homes ,A`, or `--codex-homes ""`
- **THEN** the command fails with a usage error and reads no home

#### Scenario: Mistyped home
- **WHEN** a user passes `--codex-homes A,B` and B does not exist
- **THEN** the command fails, names B, and produces no partial report from A

### Requirement: Single-home report selection is unchanged
The `report` command SHALL keep accepting `--codex-home PATH` as one path, with commas in it taken literally. Results SHALL be the same as before this change, except where a fix in this change applies: files outside the home are no longer read, and a fork child's parent is now used for replay suppression. The two options SHALL be mutually exclusive. Without either option, the report SHALL use the single `CODEX_HOME` or default home, as today.

#### Scenario: Existing invocation
- **WHEN** a user runs `report <ID> --codex-home A`
- **THEN** the output is the same as before this change for the same home contents, unless one of the listed fixes applies

#### Scenario: Copied home with an accessible original
- **WHEN** home COPY's thread metadata records rollout paths inside another home that still exists, and a user runs `report --all --codex-home COPY`
- **THEN** the report reads only COPY's own rollout files

#### Scenario: Conflicting selectors
- **WHEN** a user passes both `--codex-home A` and `--codex-homes B`
- **THEN** the command fails before reading any home

#### Scenario: Comma in CODEX_HOME
- **WHEN** `CODEX_HOME` is set to a path that contains a comma and the user passes no home option
- **THEN** the report reads that single path

### Requirement: Mutating commands remain single-home
The `update`, `schedule install`, and scheduled-run commands SHALL accept only the single `--codex-home` option. They SHALL reject `--codex-homes`. They SHALL read, lock, and write exactly one Codex home, and an installed schedule SHALL record exactly one home.

#### Scenario: Update rejects a home list
- **WHEN** a user runs `update --codex-homes A,B ...`
- **THEN** the command fails as an unknown option and changes nothing

#### Scenario: Schedule keeps one home
- **WHEN** a user runs `schedule install --codex-home A`
- **THEN** the installed job runs against A only, as before this change

### Requirement: Multi-home reports do not modify Codex data
A report over several homes SHALL NOT change Codex task data in any listed home: rollouts, SQLite state, the session index, or Desktop state. The only file the report may write is the tool's own rollout cache in each home, with the existing writable, read-only, and unavailable cache behavior applied separately for each home.

#### Scenario: Read-only copied home
- **WHEN** one listed home is on a read-only filesystem
- **THEN** the report completes, emits the existing cache notice for that home, and keeps using the cache normally in the other homes

### Requirement: A rollout ID counts once across homes
The system SHALL treat all copies of one rollout ID across the listed homes as one rollout. For each rollout ID and page, it SHALL select one file by the existing newest-file rule: the latest modification time, with ties broken by path. The selection, the totals, and the identifiers in the output SHALL NOT depend on the order of the listed homes.

#### Scenario: Rollout copied into two homes
- **WHEN** the same rollout exists in both A and B and B's copy was modified later
- **THEN** a corpus, project, or exact report over A and B counts that rollout once, using B's copy

#### Scenario: Home order does not matter
- **WHEN** the same report runs with `--codex-homes A,B` and with `--codex-homes B,A`
- **THEN** both runs produce the same totals and the same JSON output

### Requirement: Task trees span homes
Parent and child relationships SHALL resolve across all listed homes. An exact task report SHALL include descendants from any listed home. Legacy fork-replay suppression SHALL use a parent rollout from any listed home, under the existing fork-replay rules.

#### Scenario: Parent and child in different homes
- **WHEN** root R is in A and its subagent child C is in B
- **THEN** `report R --codex-homes A,B` includes C in R's tree, and `report --all --codex-homes A,B` counts R and C once each

#### Scenario: Cross-home legacy fork replay
- **WHEN** a legacy fork child in B has a copied prefix that matches its parent in A
- **THEN** the report over A and B excludes the copied prefix and keeps the child's first genuine request, the same as when both rollouts are in one home

#### Scenario: Exact report on a fork child
- **WHEN** a user reports a legacy fork child directly by its ID, with one home or with several, and its parent is in a listed home
- **THEN** the report and the JSONL export exclude the copied prefix, keep the child's first genuine request, and do not add the parent's usage to the totals

#### Scenario: Rollout not found in any home
- **WHEN** a requested ID is in none of the listed homes
- **THEN** the command fails with the existing not-found error, and the error names every listed home

### Requirement: Title matching spans homes
`report --match-title` SHALL match against the root rollouts of every listed home and SHALL require exactly one matching root ID. A root that exists in several homes SHALL count as one match. Its display name SHALL come from the home that holds the selected copy of the rollout. When that home has no name for it, the name SHALL come from another holding home, chosen in an order that does not depend on the order of the listed homes.

#### Scenario: Copied root matches once
- **WHEN** a root whose title contains "billing" exists in both A and B
- **THEN** `report --match-title billing --codex-homes A,B` selects that root and does not report an ambiguous match

### Requirement: Corpus reports cover every listed home
`report --all` over several homes SHALL include every discovered rollout from every listed home exactly once. Metadata-based pruning by date SHALL use each home's own thread metadata only for the candidates in that home. When a home's metadata is missing or unusable, the system SHALL NOT exclude that home's candidates, and pruning in the other homes SHALL continue to work.

#### Scenario: One home lacks metadata
- **WHEN** `report --all --since D --codex-homes A,B` runs, A has usable thread metadata, and B has none
- **THEN** A's out-of-range rollouts are pruned, and every rollout in B is still read and filtered by its recorded events

### Requirement: Projects resolve by name across homes
For `--project`, Desktop Projects in different homes that have the same case-insensitive name SHALL be one project, and its source roots SHALL be the union of their roots. A reference that matches in at least one home SHALL resolve, and it SHALL fail as not found only when it matches in no home.

#### Scenario: Same project in two account homes
- **WHEN** A and B each have a Desktop Project named "Billing", with different project IDs
- **THEN** `report --project Billing --codex-homes A,B` reports the roots from both homes as one project

#### Scenario: Project exists in only one home
- **WHEN** only A has a Desktop Project named "Billing"
- **THEN** the report resolves "Billing" and includes B's roots whose recorded starting directory is beneath that project's source roots and that B does not assign to another project or mark projectless

### Requirement: Each home classifies only the roots it holds
Each home SHALL apply its project assignments and projectless markers only to the roots it holds. A root SHALL be included when any home that holds it includes it. Each included root SHALL count once, under the strongest reason any holding home gives for including it: direct assignment first, then workspace fallback, then projectless thread. A root that no home includes SHALL count once in the exclusion counts, and only when a holding home rejected it within the project's scope.

#### Scenario: Conflicting assignments for a copied root
- **WHEN** root R is in A and B, A assigns R to the selected project, B marks R projectless, and root U is elsewhere, outside every source root and unassigned
- **THEN** R is included once and counted as a direct assignment, and U appears in neither the selection counts nor the exclusion counts

#### Scenario: Root held by only one home
- **WHEN** root R is only in B and A assigns some other root to the selected project
- **THEN** only B's assignments and projectless markers decide whether R is included

### Requirement: Project derived from a task across homes
When `--project` is given without a value together with a task, the system SHALL derive the project from the home that holds the selected copy of that task's root rollout. If that home assigns the root to a project, the report SHALL select that project by name across all homes. Otherwise it SHALL select the root's recorded starting directory across all homes. If that home has no Desktop metadata, the existing metadata-unavailable error SHALL apply.

#### Scenario: Assigned task in one home
- **WHEN** root R is only in A, A assigns R to "Billing", and B also has a project named "Billing"
- **THEN** `report R --project --codex-homes A,B` reports the "Billing" roots from both homes

### Requirement: Incomplete homes are visible
When a listed home exists but some part of it cannot be read, the report SHALL still include the readable rollouts from every home. It SHALL mark the affected totals incomplete with the existing lower-bound marker and warnings, exactly as a single-home report does for the same condition. Warnings SHALL appear in sanitized form.

#### Scenario: Unreadable sessions directory in one home
- **WHEN** B's sessions directory cannot be read and A is readable
- **THEN** `report --all --codex-homes A,B` reports A's rollouts with incomplete totals and includes a warning about B's unreadable directory
