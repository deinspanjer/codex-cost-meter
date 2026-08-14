# ADR 0004: Preserve the Python prototype and port the active tool to Rust

- Status: Accepted
- Date: 2026-08-13

## Context

The prototype proved the read model, dual-store persistence, and high-water behavior recorded in ADRs 0001–0003. Its runtime and deployment assumptions are not fleet-portable:

- managed macOS and clean Windows installations do not reliably provide Python 3;
- the updater imports Unix-only `fcntl`;
- scheduling is implemented with macOS `launchctl` and a LaunchAgent;
- Jamf covers macOS while Windows requires a separate mechanism such as Intune or MECM.

Maintaining Python bootstrap and runtime installation for both platforms would add more deployment surface than the utility itself.

## Decision

Preserve the Python scripts, embedded self-tests, documentation, and sanitized LaunchAgent template under `python-prototype/`. Build the active replacement as one self-contained Rust 2024 executable with incremental reporting, title-update, scheduling-management, and uninstall modes.

Use one crate while the project is small. Shared behavior remains platform-independent; narrow conditional modules own macOS, Windows, and Linux scheduling or filesystem differences. CPU architecture is a build and packaging concern, not a source-module boundary. Introduce another crate only for a demonstrated ownership, reuse, dependency, or build boundary.

Keep every invocation run-once. The executable may install, inspect, resume, or remove the native scheduling definition, but the operating system owns periodic execution; do not introduce an application daemon.

Deliver support incrementally: macOS Universal 2 first, then Windows x64, then static Linux musl binaries for x86_64 and aarch64. Add the corresponding operating-system scheduler only after reporting and title updates work on that platform.

Port the behavioral decisions and verified invariants, not the Python module layout.

Treat the approved program design as standing owner authorization to refine, plan, and execute later roadmap milestones autonomously. Each milestone still uses a just-in-time temporary specification and plan, but does not require another owner review when it remains within the approved roadmap and durable decisions. Stop for owner review only when concrete review evidence identifies an unresolved structural/shared-foundation issue, a conflict with an approved requirement or safety invariant, a material security/data-integrity design change, or a rejected milestone that cannot be corrected confidently. A rejected release blocks dependent work, but isolated later investigation may continue on separate branches when it cannot compound the finding.

Also stop all autonomous work when cumulative output-token usage for the program's root task and all descendant agents exceeds 1,000,000,000 tokens. This budget gate counts every output token, including estimate capture, actual capture, persona construction, and release-decider work that is excluded from feature estimate-versus-actual comparisons. Once crossed, allow only read-only accounting needed to report the total, then wait for owner review and steering. Neither an acceptable release verdict nor isolated branch work bypasses this gate.

## Alternatives considered

- Continue distributing Python: rejected because Python is not a reliable clean-host dependency on either target platform and the updater already contains Unix/macOS-specific behavior.
- Bundle a Python runtime: rejected because a native executable has a simpler runtime and deployment footprint for this small utility.
- Build a long-running cross-platform daemon: rejected because periodic OS scheduling is sufficient.
- Split immediately into multiple Rust crates: rejected as speculative structure.

## Consequences

- Release work must produce and sign native artifacts for each supported architecture.
- Windows and Linux need locking and scheduling implementations distinct from macOS while sharing report and update behavior.
- New product work belongs in Rust; the Python prototype remains runnable for comparison and regression discovery.
- The port remains coupled to version-sensitive internal Codex storage until a supported API replaces it.
- Later milestone planning can continue unattended without speculative up-front plans, while program-level review findings still provide a hard stop.

## Original pre-design estimate

The final estimate from the pre-implementation Codex assessment on 2026-08-13 was:

| Outcome | Agent turns | Active agent session time | Approximate tokens |
| --- | ---: | ---: | ---: |
| Minimal Rust port with fixture tests | 1–2 | 45–90 minutes | 40k–80k |
| macOS and Windows build artifacts through CI | 2–3 total | 1.5–3 hours | 70k–140k |
| Signed packages, schedulers, and uninstallers | 4–6 total | 3–5 hours | 110k–220k |
| Pilot verification on real Mac and Windows state | 5–8 total | 4–7 hours | 140k–280k |

The summary estimate for the basic cross-platform executable was **2–3 turns, about 2 hours of active agent session time, and 80k–140k tokens**. A turn meant one sustained Codex work cycle containing many tool calls, builds, and test iterations. The estimate assumed no prolonged dependency, signing, credential, or Windows-host failures.

This estimate predates the approved multi-release roadmap and used “turn” to mean an informal sustained work cycle rather than a recorded Codex task/turn lifecycle. Preserve it as historical evidence, but do not silently normalize its numbers or scope.

## Measurement convention

For all new forecasts and actuals:

- one agent turn is one recorded task/turn lifecycle;
- actual input and output tokens are recorded separately, with cached input already included in input and reasoning already included in output;
- forecasts estimate output tokens only because input volume depends heavily on accumulated context, cache behavior, and harness mechanics and is not reasonably predictable;
- agent time is summed task/turn duration and can overlap during parallel work, so it is not wall-clock elapsed time; and
- cumulative snapshots include descendant agents but public documentation omits task identifiers.

After each milestone, run that milestone's built or published `codex-cost-meter` binary against the program root task and record its sanitized, unfiltered report here beside the rollout-based estimate and actual. This self-report is independent evidence rather than a replacement for the prototype accounting used for exclusions and estimate comparisons.

Implementation, ordinary review, validation, durable documentation, release preparation, and corrective rework count toward actuals. Re-estimation, actual capture, release-persona construction or refresh, and release-decider execution are measured separately as excluded governance overhead. Fixes requested by the decider count as implementation work.

## Pre-implementation background and design actual

The snapshot taken after owner approval of the written design on 2026-08-13 establishes the excluded baseline:

| Category | Agent turns | Summed agent time | Input tokens | Output tokens | Total tokens |
| --- | ---: | ---: | ---: | ---: | ---: |
| Entire task through design finalization | 75 | 1h 43m 05s | 32,583,871 | 173,656 | 32,757,527 |
| Persona construction and trial decider run, included above | 10 | 4m 33s | 709,703 | 9,257 | 718,960 |
| Other background and design work | 65 | 1h 38m 32s | 31,874,168 | 164,399 | 32,038,567 |

All three rows are excluded from implementation estimate-versus-actual comparisons. The persona row is shown separately so governance cost remains visible without being charged to feature delivery.

## Post-design forecast

The approved design expands delivery into seven pre-1.0 platform milestones plus a 1.0 readiness milestone. The following ranges begin after design approval. Planning is included because this forecast is taken before the implementation plan exists.

| Increment | Agent turns | Summed agent time | Output tokens |
| --- | ---: | ---: | ---: |
| Implementation planning | 15–30 | 1.5–4 hours | 40k–120k |
| v0.1 macOS reporting and release pipeline | 45–85 | 6–14 hours | 120k–350k |
| v0.2 macOS title updates | 60–120 | 9–22 hours | 180k–550k |
| v0.3 macOS scheduling | 45–90 | 7–17 hours | 130k–400k |
| v0.4 Windows reporting and updates | 45–90 | 7–18 hours | 130k–400k |
| v0.5 Windows scheduling | 35–75 | 5–14 hours | 100k–330k |
| v0.6 Linux reporting and updates | 45–90 | 7–18 hours | 130k–400k |
| v0.7 Linux scheduling | 35–75 | 5–14 hours | 100k–330k |
| Remaining 0.x UX and v1.0 signing readiness | 50–110 | 8–24 hours | 150k–500k |
| **Through v1.0** | **375–765** | **55.5–145 hours** | **1.1M–3.4M** |

At current list prices, the forecast output alone would be about $33–$102 if every output token used Sol or $13–$41 if every output token used Terra. Input and cache charges are additional and will be measured from actuals rather than guessed. The execution plan should use no model turn when a direct tool invocation suffices, route bounded implementation, review, and verification packets to Terra, and reserve Sol for architectural, ambiguous, or high-risk work. Luna may be considered if it becomes an available selectable worker model, but delivery does not depend on it.

The wide range reflects platform scheduling, CI, internal Codex storage compatibility, and signing uncertainty rather than speculative feature scope. It assumes no major Codex storage rewrite, that hosted CI can exercise non-macOS builds, and that required signing credentials become available without counting external wait time. Milestone actuals and the post-plan forecast will refine the remaining range without rewriting this baseline.

## Planning actual and post-plan execution baseline

The cumulative snapshot taken immediately before v0.1 SDD execution was:

| Agent turns | Summed agent time | Input tokens | Output tokens | Total tokens |
| ---: | ---: | ---: | ---: | ---: |
| 78 | 2h 06m 59s | 40,676,833 | 229,747 | 40,906,580 |

Compared with the post-design baseline, implementation planning and its adjacent governance updates added 3 recorded turns, 23m 54s of summed agent time, 8,092,962 input tokens, and 56,091 output tokens. Those activities occurred inside mixed root-turn boundaries that cannot be divided reliably after the fact. To avoid understating delivery cost, the full delta is conservatively charged to planning for the post-design comparison; no additional exclusion is claimed for the governance portion of those mixed turns.

This cumulative snapshot is also the post-plan baseline for v0.1 implementation actuals. At the threshold check, total program output was 229,747 tokens, below the 1,000,000,000-token program stop gate.

## Post-plan v0.1 forecast

The detailed v0.1 plan contains nine implementation tasks with one implementation turn and one review turn as the normal minimum, plus integration, fix-loop, and whole-branch review capacity. Direct build and test commands do not consume separate model turns. Most bounded implementation and review work uses Terra; Sol remains reserved for architectural ambiguity, unresolved failures, and final judgment.

| Remaining v0.1 work | Agent turns | Summed agent time | Output tokens |
| --- | ---: | ---: | ---: |
| Core reporting and CLI, Tasks 1–6 | 18–36 | 3.5–9 hours | 55k–170k |
| Versioning and release pipeline, Tasks 7–8 | 6–16 | 1.5–4 hours | 20k–70k |
| Closeout, final review, and bounded corrective work | 6–14 | 1–3 hours | 15k–60k |
| **Remaining v0.1 total** | **30–66** | **6–16 hours** | **90k–300k** |

Release-persona construction and release-decider execution remain excluded governance overhead. Corrective implementation prompted by any reviewer or decider remains included. Later milestone ranges retain the post-design forecast until their own just-in-time plans provide a narrower baseline.

## v0.1 actuals

The release-candidate snapshot was taken after the release decider approved its requested compatibility fix and before the mechanical `0.1.0` version bump. The final snapshot was taken after the protected merge, a failed publication attempt, the release-workflow correction, the successful retry, and independent verification of the downloaded public artifact. It precedes this accounting-only documentation update:

| Snapshot | Agent turns | Summed agent time | Input tokens | Output tokens |
| --- | ---: | ---: | ---: | ---: |
| Post-plan baseline | 78 | 2h 06m 59s | 40,676,833 | 229,747 |
| Approved v0.1 candidate | 208 | 5h 08m 57.136s | 202,517,653 | 804,248 |
| Published and verified v0.1 milestone | 235 | 5h 20m 25.830s | 230,815,209 | 852,852 |
| Final raw delta from post-plan baseline | 157 | 3h 13m 26.830s | 190,138,376 | 623,105 |

The raw delta contains the explicitly excluded governance work:

| Excluded governance work | Agent turns | Summed agent time | Input tokens | Output tokens |
| --- | ---: | ---: | ---: | ---: |
| Owner-persona construction | 10 | 2m 47.073s | 514,576 | 4,202 |
| Release-decider evaluation and re-evaluation | 44 | 3m 18.489s | 84,915,802 | 285,180 |
| **Excluded total** | **54** | **6m 05.562s** | **85,430,378** | **289,382** |

After those exclusions, v0.1 delivery used **103 turns, 3h 07m 21.268s of summed agent time, 104,707,998 input tokens, and 333,723 output tokens**. This includes implementation, ordinary reviews, tests, CI diagnosis, documentation, release preparation, publication recovery, artifact verification, and the decider-requested compatibility fix. No separate exclusion is claimed for actual-capture commands because they ran inside mixed root turns and cannot be isolated reliably; conservatively charging those mixed turns to delivery avoids understating implementation cost.

Compared with the post-plan forecast of 30–66 turns, 6–16 summed hours, and 90k–300k output tokens, delivery exceeded the turn ceiling by 37 turns, completed below the time range, and exceeded the output-token ceiling by 33,723 tokens (11%). Input remains measured actual only and had no forecast. The original pre-design estimate described a different, broader cross-platform outcome with an informal turn definition, so it remains historical evidence rather than a like-for-like v0.1 comparator.

Total program output at this snapshot was 852,852 tokens, below the 1,000,000,000-token stop gate.

## Milestone self-reports

### v0.1.0 late post-release snapshot

The published v0.1.0 Universal 2 binary produced this report after release verification and the final accounting merge, so it is intentionally later than the v0.1 boundary. The task identifier, local project path, and task title are omitted from the public record.

| Scope | Turns | Input tokens | Cache-read tokens | Output tokens | Reasoning tokens | Summed duration | Estimated cost |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Root | 42 (41 complete, 1 incomplete) | 108,481,952 | 106,291,200 | 316,266 | 125,137 | 209m 49.4s | $73.59 |
| Whole tree | 244 (242 complete, 2 incomplete) | 233,738,986 | 226,580,224 | 861,128 | 344,656 | 320m 52.5s | $151.20+ |

| Model | Turns | Output tokens | Estimated cost |
| --- | ---: | ---: | ---: |
| `gpt-5.6-sol` | 93 | 645,434 | $141.47 |
| `gpt-5.6-terra` | 35 | 204,402 | $9.32 |
| `codex-auto-review` | 116 | 11,292 | $0.41 |

The report also measured 111m 03.1s of descendant agent-turn time, which can overlap. It marked the whole-tree input incomplete because the rollout scan skipped oversized JSONL records; therefore the trailing `+` means the reported whole-tree cost is a lower bound. Pricing was effective 2026-08-06 and used the embedded model proxies recorded by the application.

## Evidence

The decision uses the completed prototype work on rollout statistics, persisted title updates, recovery behavior, fleet portability, and the Rust multi-architecture assessment. Private task identifiers are intentionally omitted from the public repository.
