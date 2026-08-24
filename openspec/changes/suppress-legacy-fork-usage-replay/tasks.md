## 1. Preserve Explicit Fork Provenance

- [x] 1.1 Extend rollout discovery and its cached representation with explicit-fork and history-mode provenance, bump the discovery cache version, and verify focused discovery tests cover `forked_from_id`, omitted-as-legacy mode, paginated mode, missing parents, and unchanged subagent lineage.

## 2. Detect Only Proven Replay

- [x] 2.1 Add a bounded parent/child prefix comparison for structurally eligible legacy forks, covering every persisted token component while ignoring timestamps, and verify focused tests cover complete prefixes, the observed one-record concurrent-parent race, first-record mismatch, malformed input, and paginated/non-fork exclusion.
- [x] 2.2 Integrate the prefix plan with targeted and full-scan report discovery so an unavailable parent retains all child usage, and verify an exact task report and a project report resolve the same parent/child behavior.

## 3. Correct Child Analysis

- [x] 3.1 Let analysis advance cumulative state without attributing the matched copied records, resume normal processing at the first mismatch, and verify a regression fixture counts the first genuine child request with its cached, uncached, cache-write, output, and reasoning components intact.
- [x] 3.2 Bypass per-file analysis cache reads and writes only when a nonzero replay prefix is applied, and verify unchanged and unmatched rollouts still round-trip through the existing cache while a matched child cannot reuse a stale parent-independent result.

## 4. Document and Validate the Correction

- [x] 4.1 Replace the replay ambiguity in the ccusage comparative study and follow-up plan with the sanitized source and corpus invariant, measured parser-path impact, legacy/paginated boundary, and explicit rejection of timing and cross-session heuristics; verify no rollout identifiers, prompts, or private paths are retained.
- [x] 4.2 Run `just fmt`, the focused discovery/analysis/report/cache tests, `just check`, and a fresh read-only corpus comparison; verify the deterministic prefixes are excluded, first post-prefix requests remain, and report every command and result before considering title repricing.
