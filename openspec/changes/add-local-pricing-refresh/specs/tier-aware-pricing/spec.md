## MODIFIED Requirements

### Requirement: Uncertain combinations preserve incomplete-cost semantics
The system SHALL return a complete best-effort estimate when every included usage event can use explicit or fallback Standard/Fast/Ultrafast pricing and every consumed token component has an effective rate. It SHALL count post-release assumed-Standard tokens separately. Unsupported explicit tiers or unavailable rate cells SHALL retain known costs, expose the affected usage as unpriced, and mark the complete estimate unavailable.

#### Scenario: Fast marker predates durable stable support
- **WHEN** a Fast usage event predates stable `0.144.0` at 2026-07-09T16:47:12Z
- **THEN** the report excludes unsupported components from known cost and exposes an incomplete estimate

#### Scenario: Durable Fast marker uses its model premium
- **WHEN** a recognized Fast/Priority usage event occurs at or after stable `0.144.0`
- **THEN** the system uses that model's Fast rates from the effective catalog effective for the event date

#### Scenario: Recorded and assumed usage are combined
- **WHEN** a report contains one explicitly priced event and one post-release event without recorded tier metadata
- **THEN** the report prices both events, emits a complete best-effort estimate, and reports the assumed-Standard token count

### Requirement: Reports identify the applied-tier estimate basis
The system SHALL identify pricing as an API-list estimate based on the tier applied in Codex rollout settings and SHALL NOT claim authoritative billing or served-tier attribution. Human output SHALL state this basis and the pricing-catalog date, display version, and optional user label concisely; output SHALL identify embedded, embedded + overlay, or standalone mode and distinguish custom basis from official or inferred prices; `report --help` SHALL explain that the tier actually served is unavailable from current Codex token records; JSON SHALL retain exact pricing metadata.

#### Scenario: Pricing provenance is emitted
- **WHEN** a human report includes a complete price estimate
- **THEN** its footer identifies the result as an API-list estimate and gives the pricing-catalog date, version, and optional label without repeating pricing source URLs or the full tier methodology

#### Scenario: Structured pricing provenance is emitted
- **WHEN** a JSON report includes price metadata
- **THEN** the metadata describes the applied-tier basis and does not claim authoritative billing or served-tier attribution
- **AND** it exposes catalog version, optional label, and effective catalog mode as separate fields

#### Scenario: Custom prices are reported honestly
- **WHEN** an external catalog supplies user-selected prices
- **THEN** the human footer identifies user-controlled pricing and its version, effective mode, and optional label
- **AND** the report does not present those values as verified official API list prices

#### Scenario: Embedded correction with a sparse overlay
- **WHEN** a maintenance release changes a price key the overlay does not override
- **THEN** the report uses that embedded correction offline while retaining overridden values elsewhere
- **AND** its footer and JSON identify embedded + overlay mode
