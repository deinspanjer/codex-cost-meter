## 1. Effective-dated proxy selection

- [x] 1.1 Replace flat proxy targets and `undated_proxies` with ordered proxy points, validate baseline placement, increasing dates, and target histories, and verify focused invalid-catalog tests pass
- [x] 1.2 Resolve the proxy target by usage-event date before target-rate selection, retain latest-target fallback for missing time, and verify focused pre-boundary, boundary-day, static-proxy, and missing-time pricing tests pass

## 2. Proxy provenance

- [x] 2.1 Preserve the latest/default `model_proxies` structured field and add typed proxy-history metadata, then verify report JSON exposes separate target and effective-date fields
- [x] 2.2 Render static and dated proxy mappings with the Auto-review announcement-date qualification, then verify the focused human-output test passes

## 3. Catalog and documentation closeout

- [x] 3.1 Encode GPT-5.4 before 2026-07-30 and GPT-5.6 Luna from 2026-07-30 for `codex-auto-review`, update the worked example and temporary limitation text, and verify repository search finds no timeless historical-Luna claim
- [x] 3.2 Run `cargo fmt --check`, `cargo test`, `openspec validate track-auto-review-proxy-history --strict`, and `git diff --check`, recording every result before closeout

## 4. Complete Human Provenance Coverage

- [x] 4.1 Add project and corpus human-output regressions proving static and dated model-proxy mappings plus the Auto-review announcement-date qualification are rendered
- [x] 4.2 Share model-proxy provenance rendering across task, project, and corpus output; run the focused output tests, `cargo test`, strict OpenSpec validation, and `git diff --check`
