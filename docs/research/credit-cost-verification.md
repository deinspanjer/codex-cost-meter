# Selectable cost metrics: verification record

2026-11-19. Implementation and local verification for
`openspec/changes/archive/2026-11-19-add-credit-cost-metric`; not a release or hosted-CI approval.

## Artifact review

Reviewed the proposal, design, requirement scenarios and all 15 task entries
against the implementation. All 15 tasks meet their implementation/local
verification criteria. On 2026-11-19 the owner moved hosted platform CI to
post-archive PR delivery verification; results are recorded below.

- Credits: independently sourced observation boundaries, cache partition,
  tier/context/history gaps, known minima, and no inherited API exemptions.
  The exact source captures and intentionally unsupported combinations are in
  [credit-rate-evidence.md](credit-rate-evidence.md).
- Quota: typed strict TOML, half-open histories, model-specific replacement
  precedence, independent windows, frozen arithmetic, recorded guardian policy,
  identity assumptions/mixed-account refusal and content fingerprints.
  The complete numerical handoff is in [quota-calibration.md](quota-calibration.md).
- Selection/reporting: ordered subsets, global placement/repeated-flag rejection,
  separate units and independent aggregation; legacy USD values retained.
- Titles/scheduling: bounded recipes, strict suffix transitions and window
  matching, create-only policy installation, absolute scheduled paths, read-only
  settings inspection and stderr-only conflict warnings.
- Integration: one synthetic root/descendant fixture reconciles reports,
  grouping, schema-1 export and immutable preview across a period boundary,
  unknown credit history and unsupported quota speed. The modeled subtotal is
  464 points, not clamped to 100. Config edits change the fingerprint and quota
  estimate with cached usage; API catalog edits do not change frozen quota or
  credits. Invalid configuration and impossible widths preserve title stores.

Self-review, task review and final local artifact review identified and resolved:
Clap's cross-subcommand global-flag override, permissive malformed quota suffixes,
incomplete discovery propagation, uncalibrated empty-window zero presentation,
per-model credit observation provenance, and mixed-account quota reintroduced
by human Standard/assumed-Standard tier merging. Focused regressions cover the
behavioral defects. No independent-agent review is claimed.

## Commands and results

- `just test-filter credits::tests` — pass.
- `just test-filter quota::tests` — pass.
- `just test-filter costs::tests` — pass.
- `just test-filter cli::tests` — pass.
- `just test-filter title::tests` — pass.
- `just test-filter schedule::` — pass, including all three native fake-runner
  paths compiled on macOS; no real job was installed.
- `cargo test --test cost_metrics_cli --quiet` — 2 pass.
- `just check` — pass: 265 Rust tests, 13 version-tool tests, formatting and
  warnings-denied Clippy.
- `openspec validate add-credit-cost-metric --strict --json` — valid, no issues.
- Main-spec sync comparison — exact match for Purpose and all 15 requirements.
  `openspec validate cost-metric-selection --strict --json` passes, as does
  standard `openspec validate --specs --json` for all nine specs. Optional strict
  all-spec validation reports pre-existing length warnings in
  rollout-date-reporting, terminal-report-presentation and tier-aware-pricing;
  those unrelated authored requirements were left untouched.
- `git diff --check` — pass.
- Regenerated Bash/Zsh completions in memory with the real CLI, checked the
  four new option names, then ran `bash -n` / `zsh -n` on those scripts — pass.
- `cargo run --quiet -- rates --cost-metric credits` — manually inspected the
  eight model rows, units, observation dates, missing speed values and caveats.

All local code checks were rerun after corrective changes. Fixtures use
temporary homes, synthetic identities and the packaged preset; no private
rollouts, production account access, credentials or user-policy installation
were needed. The old direct-install executable/default-policy-parent collision
is documented with the existing explicit-config-path workaround.

## Post-archive delivery verification

The existing `.github/workflows/ci.yml` runs for pull requests or pushes to
`main`; it has no workflow-dispatch trigger. As requested, the feature branch
was published for hosted verification after archiving the OpenSpec change.

### First hosted run and Windows correction

[PR #78](https://github.com/deinspanjer/codex-cost-meter/pull/78) was published
after archive. In [run 44638789177](https://github.com/deinspanjer/codex-cost-meter/actions/runs/44638789177),
macOS checks, both macOS architecture builds and both Linux builds passed.
Windows tests passed, but warnings-denied Clippy rejected the unused
`read_definition` helper in the Windows production binary. Gate that Unix
file-definition reader to macOS/Linux and tests; Windows reads its definition
from native task-query XML instead. No warning suppression or runtime behavior
change was needed.

[Run 44639508651](https://github.com/deinspanjer/codex-cost-meter/actions/runs/44639508651)
passed all seven jobs on corrected commit `48ef9d9`: macOS checks and both
architecture builds, both Linux builds and scheduler contract checks, and native
Windows tests, warnings-denied Clippy, release build and scheduler lifecycle smoke.
