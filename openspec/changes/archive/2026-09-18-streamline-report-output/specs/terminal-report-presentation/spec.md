## MODIFIED Requirements

### Requirement: Terminal progress replaces its complete prior line
Interactive progress SHALL replace the complete previous progress line when its message changes and SHALL clear that line before final human or JSON report output. File counts SHALL use grammatically correct singular and plural labels. Non-interactive forced progress SHALL remain newline-delimited.

#### Scenario: Indexing transitions to a shorter analysis message
- **WHEN** interactive progress changes from an indexing message ending in `files` to a shorter analysis message
- **THEN** no characters from the indexing message remain visible after the analysis message

#### Scenario: One file is indexed
- **WHEN** progress reports exactly one indexed file
- **THEN** it displays `1 file` rather than `1 files`

#### Scenario: Interactive analysis completes
- **WHEN** an interactive report finishes analysis and begins final output
- **THEN** the progress line is cleared instead of remaining above the report

#### Scenario: Forced progress is redirected
- **WHEN** progress is explicitly enabled while standard error is not a terminal
- **THEN** each emitted progress update occupies its own completed line without terminal cursor control

### Requirement: Human layout remains intentional at practical widths
Human tables SHALL align columns according to displayed character width and SHALL keep ordinary rows readable at the established practical terminal widths. Human output SHALL avoid repeating identical scope totals, model totals already shown by the scope table, normal zero-state qualifiers, and static interpretation prose. A report with no included usage SHALL render one intentional empty state and omit empty usage sections.

#### Scenario: Exact rollout has no descendants
- **WHEN** an exact-rollout report contains only the selected rollout
- **THEN** human output renders one Root scope row and omits an identical All agents row

#### Scenario: Model rows already have a scope total
- **WHEN** a human report renders a model breakdown
- **THEN** the model section omits a Total row that would duplicate the preceding scope aggregate

#### Scenario: One ordinary model duplicates the whole report
- **WHEN** exactly one model accounts for the report and its tier evidence requires no exceptional Fast, mixed, or unavailable detail
- **THEN** human output omits the redundant model section

#### Scenario: Report contains mixed service modes
- **WHEN** a human report renders aggregate and child service-mode rows for material tier distinctions
- **THEN** every numeric column begins at the same displayed column for all rows in that table

#### Scenario: Pricing metadata contains multiple sources
- **WHEN** pricing provenance contains a long basis and multiple source URLs
- **THEN** default human output omits the repeated sources while structured output preserves them and report help explains where to inspect exact provenance

#### Scenario: Project has no model data
- **WHEN** a project, corpus, or date-filtered selection contains no usage
- **THEN** human output states that no usage exists in the selected scope or range and omits empty scope, model, rollout-type, group, and pricing sections

## ADDED Requirements

### Requirement: Exact-rollout headers identify the result compactly
An exact root-rollout human report SHALL show the task name without a recognized cost-meter title-metric suffix, a short rollout identifier, the project display name, the included rollout count, and the model/reasoning pair occurring in the greatest number of root turns. It SHALL label that pair `Most root turns` rather than `Primary`. An exact non-root report SHALL identify its rollout type and use `Most turns`. Full identifiers, full project paths, unmodified stored titles, and exact structured fields SHALL remain available in JSON.

#### Scenario: Named root cluster is reported
- **WHEN** an exact root report has a stored title, project, and descendants
- **THEN** the first line contains the cleaned task name and short identifier
- **AND** the compact metadata line contains the project display name, total rollout count, and `Most root turns: <model>/<effort>`

#### Scenario: Meter-managed title suffix is stale
- **WHEN** the stored task title ends with a cost-meter-generated cost and token suffix
- **THEN** human report identity omits that suffix rather than repeating potentially stale report metrics
- **AND** JSON retains the stored title unchanged

#### Scenario: Non-root rollout is selected directly
- **WHEN** an exact report selects a security review, subagent, or other non-root rollout
- **THEN** human output identifies the rollout type and short identifier and labels its most frequent pair `Most turns`

### Requirement: Human usage tables expose nested metrics and turn-time scope
Human usage tables SHALL present `Turns`, `Input (cached)`, `Output (reasoning)`, `Turn time`, and `Est. cost`. Cached input SHALL appear as the nested component of input and reasoning output as the nested component of output. Complete turn counts SHALL render as the count alone; nonzero incomplete counts SHALL be appended. Exact clusters SHALL distinguish Root from All agents, where Root turn time sums recorded lifecycle durations in the selected root rollout and All agents sums durations across the root and descendants regardless of overlap.

#### Scenario: Complete cluster has descendants
- **WHEN** an exact root report contains descendants and no incomplete turns
- **THEN** human output contains Root and All agents rows with plain turn counts and nested cached-input and reasoning-output values

#### Scenario: Cluster contains incomplete turns
- **WHEN** a scope row contains one or more incomplete turns
- **THEN** its turn cell includes the nonzero incomplete count without also spelling out the complete count

#### Scenario: Parallel descendant work is aggregated
- **WHEN** an exact cluster includes descendant turns that overlap root or sibling turns
- **THEN** All agents turn time sums every included recorded turn duration
- **AND** human output states concisely that all-agent turn time sums overlapping work

### Requirement: Aggregate reports use scope-appropriate identity
Project reports SHALL identify the project, selected task count, rollout count, and date range. Corpus reports SHALL identify all rollouts, rollout count, and date range. They SHALL NOT present a single model/reasoning pair as root metadata. Grouped output SHALL use the compact nested-metric columns and preserve its requested period and dimensions. Selection resolver diagnostics SHALL remain in JSON and human output SHALL surface only nonzero exclusions, incomplete roots, or partial-cost roots that materially qualify the result.

#### Scenario: Project lifetime report is rendered
- **WHEN** a project report includes multiple task roots and rollouts without date bounds
- **THEN** human output identifies the project, task count, rollout count, and Lifetime range without a `Most root turns` field

#### Scenario: Corpus date report is grouped
- **WHEN** a corpus report uses date bounds and grouping dimensions
- **THEN** human output identifies the selected range and rollout count and renders compact group rows for the requested dimensions

#### Scenario: Project selection has material qualifications
- **WHEN** one or more selected task trees are incomplete, partially priced, or excluded
- **THEN** human output reports the corresponding nonzero counts concisely
- **AND** JSON retains the full resolver and assignment diagnostics

### Requirement: Human model breakdowns remain explanatory
When a model breakdown is not redundant, human output SHALL show model, turns, input, output, turn time, and estimated cost, sorted by known cost as today. It SHALL omit nested cache-read and reasoning columns from model rows, omit a duplicate Total row, and retain child rows only for material tier distinctions that cannot be represented by the aggregate row alone.

#### Scenario: Cluster uses several models
- **WHEN** an exact cluster contains more than one model
- **THEN** human output renders one aggregate row per model with the reduced column set and no Total row

#### Scenario: Aggregate contains legacy usage without attributed turns
- **WHEN** a model has token usage and cost but zero attributed turns
- **THEN** its model row remains visible with a zero turn count rather than discarding the usage

### Requirement: Static report interpretation lives in report help
The `report --help` output SHALL explain nested token columns, recorded turn-time semantics and overlap, API-list estimate status, applied-versus-served tier limits, assumed Standard pricing, partial-cost markers, model-proxy estimation, and where exact JSON provenance can be found. Default human reports SHALL contain only a concise pricing date and conditional warnings that apply to that result.

#### Scenario: Complete report has no material warning
- **WHEN** a human report is complete and contains no assumed tier, exceptional tier, or proxy warning that changes interpretation
- **THEN** its footer contains only the concise API-list estimate and pricing date

#### Scenario: User requests report help
- **WHEN** the user runs `report --help`
- **THEN** the command describes the static interpretation rules omitted from ordinary report output
