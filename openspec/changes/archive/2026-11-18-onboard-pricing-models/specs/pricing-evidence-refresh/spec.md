# Spec Delta

## ADDED Requirements

### Requirement: Refresh onboards a model with an explicit scoped cutoff
For a previously unknown model, deterministic refresh SHALL propose the supported labeled rates and their input-token cutoff as one atomic group when the official pricing section explicitly defines complementary short and long bands for those rows. Missing, conflicting or unrelated cutoff evidence SHALL block that group without blocking other models.

#### Scenario: Two eligible models with partial overall coverage
- **WHEN** flagship Standard and Fast tables list GPT-6.2 Astra and Sol with both bands and define short as ≤272K input tokens and long as >272K, without a supported start date
- **THEN** refresh proposes one group per model with cutoff 272,000 and four tier/band rate points effective on the UTC observation date; earlier dates remain unpriced
- **AND** unsupported promotional tables or unrelated coverage gaps leave those groups eligible and the overall result partial

#### Scenario: One legend for multiple speed tables
- **WHEN** the bounded flagship pricing block has one standalone unqualified short/long definition after its Ultrafast table
- **THEN** the definition covers matching short/long column headers across its supported speed tables, including Standard and Fast; a qualifier inside another model's row applies only to that row
- **AND** a missing block terminator does not authorize searching the remainder of the page for a legend

#### Scenario: Existing history cannot acquire a retroactive cutoff
- **WHEN** the model has effective rate history, an existing threshold, a proxy identity or a user suppression
- **THEN** it is not eligible for an automatic new-model threshold add; existing short-only rates remain usable and newly unsupported long-band observations remain unresolved

#### Scenario: Provenance of a current cutoff
- **WHEN** deterministic onboarding applies a cutoff observed on the current price page without an evidenced start date
- **THEN** the threshold records observed basis and the source URL, and does not establish historical pricing before the first supported point

#### Scenario: Cutoff from a different section
- **WHEN** a new model has short and long price rows but the only cutoff is in a different product section, or applicable definitions disagree
- **THEN** no threshold or dependent price is applied for that model and refresh identifies the missing or conflicting evidence

### Requirement: Review connects eligibility to source facts
The human refresh summary SHALL lead with the outcome and coverage, and present each new model's eligibility, proposed rates by tier and band, units, effective date and basis, cutoff and supporting source section together. Blocked models SHALL identify the specific reason. JSON SHALL retain full evidence and its association with each proposal operation.

#### Scenario: Dry run with eligible and unresolved work
- **WHEN** a preview finds eligible Standard/Fast rates and other pricing it could not verify
- **THEN** the reviewer can see which changes would apply and why the cutoff applies to those rows without reading repeated full source tables; no catalog write occurs
- **AND** the result distinguishes prices not verified from prices proposed for activation

#### Scenario: Unsupported Ultrafast prices on an otherwise eligible model
- **WHEN** an onboarded model appears in named Ultrafast promotional or list tables that refresh does not support
- **THEN** its group identifies Ultrafast as unresolved in human and JSON results without applying those rates or withholding its supported Standard/Fast rates

### Requirement: Imported shared cutoffs require verified document scope
For a threshold add citing a shared flagship legend, import SHALL verify the model row, table scope and cutoff in the same independently retrieved official pricing Markdown document or allowed exact raw capture. Unsupported representations SHALL remain unresolved for this evidence path. Existing model-specific evidence remains supported.

#### Scenario: Matching strings alone do not establish applicability
- **WHEN** an imported group quotes an authentic model row and threshold but the retrieved document places them in different product blocks
- **THEN** the group remains unresolved even when both excerpts match; a correctly scoped equivalent passes the evidence check
