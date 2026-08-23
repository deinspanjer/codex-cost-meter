## 1. Attribute Applied Service Tiers

- [x] 1.1 Add one rollout-analysis regression covering Standard → Fast → Standard chronology, `priority` normalization, an omitted tier resetting attribution, and an unsupported tier remaining unpriced; run the focused test and confirm it fails before implementation
- [x] 1.2 Add the serialized applied-tier value to usage events, track settings in the analysis pass, and increment the analysis cache version; verify the focused rollout-analysis test passes

## 2. Select Tier and Context Rates

- [x] 2.1 Add one catalog regression covering 272,000 versus 272,001 input tokens, Standard versus Fast, an event before the Fast effective date, and a Fast long-context rate; run the focused test and confirm it fails before implementation
- [x] 2.2 Extend catalog parsing/validation and the embedded price data with explicit tier/context rate grids captured on 2026-08-22; verify the focused pricing test and existing pricing tests pass

## 3. Preserve Incomplete Report Semantics

- [x] 3.1 Add one report regression combining fully priced usage with missing and unsupported tier attribution; run the focused test and confirm it fails before implementation
- [x] 3.2 Aggregate unpriced service-tier usage separately, keep known cost from supported events, and update pricing provenance; verify the focused report test and report CLI tests pass

## 4. Document and Validate

- [x] 4.1 Update `USERS.md` to describe applied-tier estimates, unavailable served-tier data, long-context selection, and incomplete tier pricing; verify documentation matches rendered report terminology
- [x] 4.2 Run `cargo fmt --check`, focused pricing/parser/report tests, `cargo test`, `openspec validate add-fast-mode-pricing --strict`, and `git diff --check`; record every result before completion

## 5. Expose Model Tier Detail and Clarify Historical Evidence

- [x] 5.1 Add one report regression proving Standard, Fast, and unavailable-tier usage and known cost remain distinct beneath one aggregate model; run it and confirm it fails before implementation
- [x] 5.2 Add structured per-tier model detail, render `⚡ Fast` human subrows without guessing tier-specific turns or duration, and document the evidence boundary around missing snapshots and the 2026-08-22 price-capture date; verify focused report and output tests pass
- [x] 5.3 Run `cargo fmt --check`, focused parser/pricing/report/output tests, `cargo test`, `openspec validate add-fast-mode-pricing --strict`, and `git diff --check`; record every result before completion

## 6. Prefer Useful Best-Effort Estimates

- [x] 6.1 Record the first public opt-in and ordinary-user Fast boundaries and add focused regressions proving pre-release missing tiers are Standard while creator-version or post-release timestamp evidence produces assumed Standard; run them red then green
- [x] 6.2 Add assumed-Standard tier handling, cache invalidation, structured assumption counts, human labeling, and concise user documentation; verify focused parser/report/output tests and representative real reports
- [x] 6.3 Run `cargo fmt --check`, focused parser/pricing/report/output tests, `cargo test`, `openspec validate add-fast-mode-pricing --strict`, and `git diff --check`; record every result before completion
