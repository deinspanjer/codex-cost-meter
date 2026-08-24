# Terminal Report Presentation Specification

## Purpose

Provide compact, stable, and self-explanatory terminal reports across task, project, and corpus modes without changing structured report data.

## Requirements

### Requirement: Terminal progress replaces its complete prior line
Interactive progress SHALL replace the complete previous progress line when its message changes and SHALL leave exactly one completed progress line before report output. File counts SHALL use grammatically correct singular and plural labels. Non-interactive forced progress SHALL remain newline-delimited.

#### Scenario: Indexing transitions to a shorter analysis message
- **WHEN** interactive progress changes from an indexing message ending in `files` to a shorter analysis message
- **THEN** no characters from the indexing message remain visible after the analysis message

#### Scenario: One file is indexed
- **WHEN** progress reports exactly one indexed file
- **THEN** it displays `1 file` rather than `1 files`

#### Scenario: Forced progress is redirected
- **WHEN** progress is explicitly enabled while standard error is not a terminal
- **THEN** each emitted progress update occupies its own completed line without terminal cursor control

### Requirement: Human layout remains intentional at practical widths
Human tables SHALL align columns according to displayed character width without glyph-specific padding exceptions. Pricing provenance SHALL use bounded, indented continuation lines so prose and source lists do not hard-wrap mid-field at 120- and 160-column terminal widths. A report with no model data SHALL render an explicit empty model state instead of a table containing only a Total row.

#### Scenario: Report contains mixed service modes
- **WHEN** a human report renders aggregate and child service-mode rows
- **THEN** every numeric column begins at the same displayed column for all rows in that table

#### Scenario: Pricing metadata contains multiple sources
- **WHEN** pricing provenance contains a long basis and multiple source URLs
- **THEN** human output renders readable labeled or indented continuation lines within the bounded prose width

#### Scenario: Project has no model data
- **WHEN** a selected project produces no model rows
- **THEN** the Models section states that no model usage was found and omits the model table header, separator, and Total row
