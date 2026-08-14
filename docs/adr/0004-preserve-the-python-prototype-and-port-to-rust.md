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

- Release work must produce native artifacts and checksums for each supported architecture. Before v1.0, a Windows archive may remain unsigned when its checksum and trust limitation are disclosed; production macOS and Windows signing remains a v1.0 readiness goal.
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

## v0.2 post-design baseline and forecast

The cumulative snapshot taken after the just-in-time v0.2 title-update design was finalized was:

| Agent turns | Summed agent time | Input tokens | Output tokens | Total tokens |
| ---: | ---: | ---: | ---: | ---: |
| 247 | 5h 21m 01.087s | 236,906,552 | 876,736 | 237,783,288 |

This is the post-design baseline for v0.2. The 12 turns, 35.257s, 6,091,343 input tokens, and 23,884 output tokens since the published-v0.1 snapshot include the late v0.1 self-report capture and documentation plus v0.2 design refinement. They are excluded from v0.2 delivery comparisons rather than being divided across mixed task boundaries after the fact. Total program output remained below the 1,000,000,000-token stop gate.

The refined v0.2 design adds one bundled SQLite dependency, three focused modules, one shared report context, dry-run/apply CLI integration, mutation hardening, documentation, and the existing release gates. Beginning after this snapshot, the post-design forecast is:

| Remaining v0.2 work | Agent turns | Summed agent time | Output tokens |
| --- | ---: | ---: | ---: |
| Detailed implementation planning | 6–12 | 0.5–2 hours | 20k–60k |
| Title composition and shared session-index behavior | 10–22 | 2–5 hours | 35k–110k |
| SQLite selection, locking, mutation, and recovery | 18–36 | 3–8 hours | 55k–180k |
| CLI integration, hardening, docs, review, and release | 14–30 | 3–7 hours | 45k–140k |
| **Remaining v0.2 total** | **48–100** | **8.5–22 hours** | **155k–490k** |

The forecast assumes bundled SQLite builds for both macOS architectures without toolchain intervention, the observed Codex schema still satisfies the required-column contract, disk-full behavior can be tested through bounded I/O failure injection, and scheduling remains outside v0.2. Bounded implementation and review packets should use Terra where available; architectural or unresolved data-integrity decisions remain with Sol.

### v0.2 planning actual and post-plan forecast

The finalized six-task TDD plan produced this cumulative post-plan baseline:

| Agent turns | Summed agent time | Input tokens | Output tokens | Total tokens |
| ---: | ---: | ---: | ---: | ---: |
| 250 | 5h 21m 11.025s | 238,900,661 | 890,051 | 239,790,712 |

Compared with the v0.2 post-design baseline, detailed planning used 3 turns, 9.938s of summed agent time, 1,994,109 input tokens, and 13,315 output tokens. This planning cost counts against the post-design forecast. The snapshot is the implementation baseline for the post-plan comparison, and total program output remained below the 1,000,000,000-token stop gate.

The plan has five independently reviewable implementation tasks plus documentation, accounting, final review, release-decider, and publication work. Release-persona refresh and release-decider execution remain excluded governance overhead; fixes they request remain delivery work.

| Remaining v0.2 work | Agent turns | Summed agent time | Output tokens |
| --- | ---: | ---: | ---: |
| Shared report/session-index state and title composition | 8–16 | 2–4 hours | 35k–100k |
| SQLite selection, mutation, lock, and recovery | 14–28 | 3–7 hours | 55k–170k |
| CLI integration and compatibility hardening | 6–14 | 1.5–4 hours | 25k–80k |
| Durable docs, validation, review, correction, and release | 10–20 | 1.5–5 hours | 35k–100k |
| **Remaining v0.2 total** | **38–78** | **8–20 hours** | **150k–450k** |

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

### v0.2.0 candidate-prep accounting snapshot

This snapshot was taken after the `0.2.0` version bump, local package validation, and temporary-plan removal, but before the final review, owner-persona decision, protected merge, and release checks. The selected program root is resolved from the current Codex task lineage; its identifier, title, prompt, local path, and all other task metadata are intentionally omitted here.

Prototype accounting command (with placeholders in this public record):

```text
python3 python-prototype/rollout_stats.py <program-root> --codex-home <local-codex-home> --json
```

| Snapshot | Agent turns | Summed agent time | Input tokens | Output tokens | Total tokens |
| --- | ---: | ---: | ---: | ---: | ---: |
| v0.2 post-plan baseline | 250 | 5h 21m 11.025s | 238,900,661 | 890,051 | 239,790,712 |
| Candidate-prep whole tree | 288 | 8h 40m 09.835s | 290,975,700 | 1,128,594 | 292,104,294 |
| Candidate-prep raw delta | 38 | 3h 18m 58.810s | 52,075,039 | 238,543 | 52,313,582 |

The raw delta is the candidate-prep delivery actual to date: implementation, ordinary reviews, validation, durable documentation, release preparation, and corrective work. Against the post-plan remaining-v0.2 forecast of 38–78 turns, 8–20 summed hours, and 150k–450k output tokens, it has reached the lower turn bound, remains below the time range, and is within the output-token range. It is not a final milestone actual because the remaining review, owner, merge, and release work has not yet occurred.

No separately attributable re-estimation or accounting-capture lifecycle occurred after the post-plan baseline; direct reporter invocations do not create an agent-turn lifecycle. No owner-persona or release-decider lifecycle had run at this candidate-prep boundary. Accordingly, separately excluded governance overhead is zero in this snapshot; any small mixed-turn capture work is conservatively retained in the raw delivery delta rather than guessed apart. Future separately attributable governance work remains excluded from estimate-versus-actual while still contributing to the absolute stop check.

The prototype saw complete priced model data for the selected tree: 54 rollouts, 285 complete/aborted turns and 3 incomplete turns, with known estimated cost of $180.22. Its absolute whole-tree output total, 1,128,594 tokens, was below the 1,000,000,000-output-token program stop gate.

The v0.2.0 candidate binary self-report is additive and deliberately unfiltered: it includes the root and every linked descendant regardless of whether a lifecycle is accountable delivery or excluded governance work. Candidate command (placeholders preserve the public sanitization format):

```text
target/universal2/release/codex-cost-meter report <program-root> --codex-home <local-codex-home> --json
```

| Scope | Rollouts | Turns | Input tokens | Cache-read tokens | Output tokens | Reasoning tokens | Summed duration | Estimated cost |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Root | 1 | 43 (42 complete, 1 incomplete) | 136,581,760 | 133,911,552 | 380,883 | 146,180 | 341m 58.427s | $91.73 |
| Whole tree | 54 | 288 (285 complete, 3 incomplete) | 291,112,195 | 282,023,424 | 1,129,261 | 437,533 | 520m 09.835s | $180.26+ |

| Model | Turns | Output tokens | Known estimated cost |
| --- | ---: | ---: | ---: |
| `gpt-5.6-sol` | 96 | 726,864 | $161.25 |
| `gpt-5.6-terra` | 53 | 388,408 | $18.50 |
| `codex-auto-review` | 139 | 13,989 | $0.51 |

The candidate scan skipped oversized JSONL records, so its whole-tree complete estimate is unavailable and the displayed cost is a lower bound; no model was unpriced. Its higher immediate whole-tree total of 1,129,261 output tokens remains below the absolute 1,000,000,000-token stop gate. The small difference from the preceding prototype snapshot reflects live local task state advancing between the two read-only scans, not a filtered accounting adjustment.

### v0.2.0 approved pre-release accounting snapshot

This snapshot was taken after the whole-branch review, its single corrective fix wave and scoped re-review, the one-time durable owner-persona reconstruction, and the release-decider verdict, but before the protected merge and public release checks. The persona is now preserved in the release rubric; later milestones reuse it and incur persona-refinement overhead only when new owner evidence exposes a durable rubric gap.

| Snapshot | Agent turns | Summed agent time | Input tokens | Output tokens | Total tokens |
| --- | ---: | ---: | ---: | ---: | ---: |
| v0.2 post-plan baseline | 250 | 5h 21m 11.025s | 238,900,661 | 890,051 | 239,790,712 |
| Approved pre-release whole tree | 308 | 9h 27m 34.397s | 310,532,497 | 1,228,371 | 311,760,868 |
| Raw post-plan delta | 58 | 4h 06m 23.372s | 71,631,836 | 338,320 | 71,970,156 |
| Excluded governance overhead | 11 | 7m 52.487s | 1,914,497 | 14,687 | 1,929,184 |
| Accountable delivery actual | 47 | 3h 58m 30.885s | 69,717,339 | 323,633 | 70,040,972 |

The excluded governance row contains the separately attributable one-time persona-research tree and release-decider lifecycle. Direct reporter calls created no agent lifecycle. Mixed controller work remains conservatively in delivery actuals rather than being estimated away. Against the post-plan remaining-v0.2 forecast of 38–78 turns, 8–20 summed hours, and 150k–450k output tokens, accountable delivery is within the turn and output ranges and below the forecast time range. Protected merge, publication, and public-asset verification remain outside this pre-release snapshot and will be added to the final post-release actual.

The release decider returned `APPROVE_WITH_FOLLOWUPS` with no release blocker. Its only follow-up is the already-triaged loss of the underlying I/O cause when the updater cannot read `session_index.jsonl`; `TODO.md` bounds that v0.3 work to diagnostic clarity without a general error-framework expansion.

The rebuilt v0.2.0 candidate binary produced this additive, deliberately unfiltered root-task report after the final production fix:

| Scope | Rollouts | Turns | Input tokens | Cache-read tokens | Output tokens | Reasoning tokens | Summed duration | Known estimated cost |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Root | 1 | 43 (42 complete, 1 incomplete) | 145,289,061 | 142,512,640 | 402,186 | 153,339 | 341m 58.427s | $97.20 |
| Whole tree | 64 | 308 (306 complete, 2 incomplete) | 310,816,256 | 300,926,464 | 1,229,735 | 481,944 | 567m 34.397s | $192.99+ |

| Model | Turns | Output tokens | Known estimated cost |
| --- | ---: | ---: | ---: |
| `gpt-5.6-sol` | 100 | 785,774 | $171.66 |
| `gpt-5.6-terra` | 58 | 428,370 | $20.81 |
| `codex-auto-review` | 150 | 15,591 | $0.53 |

The application again marked whole-tree input incomplete because the rollout scan skipped oversized JSONL records, so the displayed known cost is a lower bound. No model was unpriced. The absolute whole-tree output total of 1,229,735 tokens includes excluded governance work and remains far below the 1,000,000,000-token program-stop gate.

## v0.3 post-design baseline and forecast

The v0.3 design boundary was captured after the v0.2 protected merge, the owner's requested execution-timeline report, and the just-in-time macOS scheduling design, but before v0.3 implementation planning:

| Snapshot | Agent turns | Summed agent time | Input tokens | Output tokens | Total tokens |
| --- | ---: | ---: | ---: | ---: | ---: |
| v0.2 approved pre-release baseline | 308 | 9h 27m 34.397s | 310,532,497 | 1,228,371 | 311,760,868 |
| v0.3 post-design whole tree | 321 | 11h 09m 43.512s | 329,545,225 | 1,276,125 | 330,821,350 |
| Excluded release/report/design delta | 13 | 1h 42m 09.115s | 19,012,728 | 47,754 | 19,060,482 |

The mixed delta is excluded from v0.3 feature delivery rather than guessing apart v0.2 publication mechanics, the owner-requested report, and v0.3 design work inside shared root-turn boundaries. The cumulative output total remains below the 1,000,000,000-token program stop gate.

The refined v0.3 design adds a standard-library LaunchAgent lifecycle, one bounded atomic status record, failure classification and circuit-breaker transitions, schedule status/resume/remove commands, self-uninstall, focused CLI integration, durable documentation, and the existing release gates. It adds no daemon, runtime dependency, empty future-platform module, or real-user scheduler mutation in automated tests.

| Remaining v0.3 work | Agent turns | Summed agent time | Output tokens |
| --- | ---: | ---: | ---: |
| Detailed implementation planning | 4–8 | 0.5–1.5 hours | 15k–45k |
| Status model, failure classification, and circuit breaker | 8–16 | 1.5–4 hours | 30k–90k |
| macOS LaunchAgent lifecycle and uninstall | 8–18 | 2–5 hours | 35k–100k |
| CLI integration and compatibility hardening | 4–10 | 1–3 hours | 20k–60k |
| Durable docs, validation, review, correction, and release | 8–18 | 2–5 hours | 35k–100k |
| **Remaining v0.3 total** | **32–70** | **7–18.5 hours** | **135k–395k** |

The forecast assumes `/bin/launchctl` and `/usr/bin/id` retain their supported macOS behavior, current-user LaunchAgents remain available on macOS 14 and later, the executable can unlink itself on macOS, and the existing updater errors can be classified without a general error framework. Most implementation and task-review packets should use Terra; Sol remains reserved for the final whole-branch and owner-approval judgments or unresolved safety questions.

### v0.3 planning actual and post-plan forecast

The finalized five-task TDD plan produced this cumulative post-plan baseline:

| Agent turns | Summed agent time | Input tokens | Output tokens | Total tokens |
| ---: | ---: | ---: | ---: | ---: |
| 321 | 11h 09m 43.512s | 332,613,311 | 1,286,373 | 333,899,684 |

Compared with the v0.3 post-design baseline, detailed planning and its accounting capture used no additional completed agent lifecycle or summed agent time, 3,068,086 input tokens, and 10,248 output tokens. The root turn remained open across both snapshots, so this is measured token work without a new turn-duration boundary. It counts against the post-design forecast. Total program output remained below the 1,000,000,000-token stop gate.

The plan has four implementation tasks plus candidate documentation, accounting, validation, final review, release-decider, and publication work. Stable-persona construction will not recur; only release-decider execution is excluded governance overhead unless new owner evidence requires a durable rubric refinement.

| Remaining v0.3 work | Agent turns | Summed agent time | Output tokens |
| --- | ---: | ---: | ---: |
| Updater failure classification | 2–4 | 0.5–1.5 hours | 10k–30k |
| Bounded status and circuit breaker | 2–4 | 0.75–2 hours | 15k–40k |
| macOS LaunchAgent lifecycle and uninstall | 2–6 | 1–3 hours | 20k–60k |
| CLI integration and scheduled-run hardening | 2–6 | 1–3 hours | 20k–60k |
| Durable docs, validation, final review, correction, and release | 6–14 | 1.5–4 hours | 25k–80k |
| **Remaining v0.3 total** | **14–34** | **4.75–13.5 hours** | **90k–270k** |

### v0.3.0 candidate-prep accounting snapshot

This snapshot was taken after the `0.3.0` version bump and before final whole-branch review, release-decider work, protected merge, and publication. The selected program root, task title, prompt, local paths, and other task metadata are intentionally omitted from this public record.

The independent prototype reporter measured the whole tree as follows:

| Snapshot | Agent turns | Summed agent time | Input tokens | Output tokens | Total tokens |
| --- | ---: | ---: | ---: | ---: | ---: |
| v0.3 post-plan baseline | 321 | 11h 09m 43.512s | 332,613,311 | 1,286,373 | 333,899,684 |
| Candidate-prep whole tree | 337 | 11h 52m 52.894s | 359,947,292 | 1,425,901 | 361,373,193 |
| Raw post-plan delta | 16 | 43m 09.382s | 27,333,981 | 139,528 | 27,473,509 |

No separately attributable post-plan governance lifecycle is excluded at this candidate-prep boundary. Mixed root work is conservatively charged to delivery rather than guessed apart. Against the post-plan remaining-v0.3 forecast of 14–34 turns, 4.75–13.5 summed hours, and 90k–270k output tokens, the raw delivery actual is within the turn and output ranges and below the time range. The selected tree had complete model-price coverage, 334 complete or aborted turns, and 3 incomplete turns; its known estimated cost was $220.61. Its absolute whole-tree output total, 1,425,901 tokens, remains below the 1,000,000,000-output-token program stop gate.

The `0.3.0` candidate binary produced the following additive, deliberately unfiltered root-task report immediately afterwards:

| Scope | Rollouts | Turns | Input tokens | Cache-read tokens | Output tokens | Reasoning tokens | Summed duration | Known estimated cost |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Root | 1 | 45 (44 complete, 1 incomplete) | 177,976,163 | 174,504,704 | 483,183 | 182,874 | 443m 30.353s | $119.11 |
| Whole tree | 77 | 337 (334 complete, 3 incomplete) | 360,468,101 | 349,047,040 | 1,427,029 | 557,220 | 712m 52.894s | $220.82+ |

| Model | Turns | Output tokens | Known estimated cost |
| --- | ---: | ---: | ---: |
| `gpt-5.6-sol` | 102 | 866,771 | $193.56 |
| `gpt-5.6-terra` | 71 | 543,162 | $26.70 |
| `codex-auto-review` | 164 | 17,096 | $0.57 |

The binary scan skipped oversized JSONL records, so the whole-tree complete estimate is unavailable and the displayed known cost is a lower bound; no model was unpriced. Its slightly later whole-tree output total remains below the same absolute stop gate. The difference from the independent snapshot is live local task state advancing between read-only scans, not a filtered accounting adjustment.

### v0.3.0 final pre-decider accounting snapshot

This snapshot follows the whole-branch correction, focused validation, and local Universal 2 package verification, and precedes the release-decider, protected merge, and publication. The selected program root and other task metadata are intentionally omitted from this public record.

| Snapshot | Agent turns | Summed agent time | Input tokens | Output tokens | Total tokens |
| --- | ---: | ---: | ---: | ---: | ---: |
| v0.3 post-plan baseline | 321 | 11h 09m 43.512s | 332,613,311 | 1,286,373 | 333,899,684 |
| Final pre-decider whole tree | 343 | 12h 19m 14.830s | 377,525,260 | 1,489,534 | 379,014,794 |
| Raw post-plan delta | 22 | 1h 09m 31.318s | 44,911,949 | 203,161 | 45,115,110 |

No separately attributable post-plan governance lifecycle is excluded at this boundary; direct reporter invocations do not create an agent lifecycle, and mixed controller work remains conservatively charged to delivery. Against the post-plan remaining-v0.3 forecast of 14–34 turns, 4.75–13.5 summed hours, and 90k–270k output tokens, the raw delivery actual is within the turn and output ranges and below the time range. The selected tree had complete model-price coverage, 340 complete or aborted turns and 3 incomplete turns, with known estimated cost of $230.17. Its absolute whole-tree output total of 1,489,534 tokens remains below the 1,000,000,000-token program stop gate; autonomous work may continue without this gate requiring owner review.

The rebuilt `0.3.0` candidate binary produced the following additive, deliberately unfiltered root-task report immediately afterwards:

| Scope | Rollouts | Turns | Input tokens | Cache-read tokens | Output tokens | Reasoning tokens | Summed duration | Known estimated cost |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Root | 1 | 45 (44 complete, 1 incomplete) | 184,325,312 | 180,785,152 | 492,767 | 186,591 | 443m 30.353s | $122.88 |
| Whole tree | 82 | 343 (340 complete, 3 incomplete) | 377,860,307 | 365,939,456 | 1,490,146 | 588,023 | 739m 14.830s | $230.31+ |

The binary scan skipped oversized JSONL records, so the whole-tree complete estimate is unavailable and the displayed known cost is a lower bound; no model was unpriced. Its slightly later whole-tree output total also remains below the same absolute stop gate. The difference from the independent snapshot is live local task state advancing between read-only scans, not a filtered accounting adjustment.


## v0.4 post-design baseline and forecast

The v0.4 Windows application boundary was captured after the just-in-time design and before detailed planning:

| Agent turns | Summed agent time | Input tokens | Output tokens | Total tokens |
| ---: | ---: | ---: | ---: | ---: |
| 357 | 12h 36m 52.223s | 394,616,391 | 1,528,428 | 396,144,819 |

This is the post-design baseline for v0.4. The design keeps shared `report` and `update` behavior in the single crate, omits the macOS scheduler surface from Windows at compile time, packages one native Windows x64 executable, and adds native Windows CI. It adds no scheduler abstraction, Windows scheduling placeholder, installer, signing infrastructure, or Windows arm64 build. Total program output remained below the 1,000,000,000-token stop gate.

| Remaining v0.4 work | Agent turns | Summed agent time | Output tokens |
| --- | ---: | ---: | ---: |
| Detailed implementation planning | 3–8 | 0.5–2 hours | 15k–45k |
| Windows application boundary | 5–12 | 1–3 hours | 20k–60k |
| Package and native automation | 6–16 | 1.5–4 hours | 30k–90k |
| Closeout, reviews, corrections, and release | 8–20 | 2–5 hours | 40k–120k |
| **Remaining v0.4 total** | **22–56** | **5–14 hours** | **105k–315k** |

Accounting capture, re-estimation, and release-decider work remain excluded governance overhead for feature estimate-versus-actual comparisons while still counting toward the absolute stop gate.

### v0.4 planning actual and post-plan forecast

The finalized three-task plan produced this cumulative post-plan baseline:

| Snapshot | Agent turns | Summed agent time | Input tokens | Output tokens | Total tokens |
| --- | ---: | ---: | ---: | ---: | ---: |
| Post-design whole tree | 357 | 12h 36m 52.223s | 394,616,391 | 1,528,428 | 396,144,819 |
| Post-plan whole tree | 360 | 12h 42m 50.581s | 397,183,300 | 1,538,239 | 398,721,539 |
| Planning delta | 3 | 5m 58.358s | 2,566,909 | 9,811 | 2,576,720 |

The plan retains separate application-boundary, package/automation, and durable-closeout responsibilities. Beginning after the post-plan snapshot, its execution forecast is:

| Remaining v0.4 work | Agent turns | Summed agent time | Output tokens |
| --- | ---: | ---: | ---: |
| Windows application boundary | 5–12 | 1–3 hours | 20k–60k |
| Package and native automation | 6–16 | 1.5–4 hours | 30k–90k |
| Closeout, reviews, corrections, and release | 8–20 | 2–5 hours | 40k–120k |
| **Remaining v0.4 total** | **19–48** | **4.5–12 hours** | **90k–270k** |

The planning delta counts against the post-design forecast. The accounting capture and re-estimation work are excluded governance overhead; input tokens are measured actual only. Total program output remained below the absolute stop gate.

### v0.4.0 final candidate accounting snapshot

This snapshot follows candidate validation and the `0.4.0` version bump, and precedes final review, release-decider work, protected merge, and publication. The selected program root and other task metadata are intentionally omitted from this public record.

| Snapshot | Agent turns | Summed agent time | Input tokens | Output tokens | Total tokens |
| --- | ---: | ---: | ---: | ---: | ---: |
| v0.4 post-plan baseline | 360 | 12h 42m 50.581s | 397,183,300 | 1,538,239 | 398,721,539 |
| Final candidate whole tree | 594 | 13h 03m 52.687s | 1,413,924,408 | 4,183,846 | 1,418,108,254 |
| Raw post-plan delta | 234 | 21m 02.106s | 1,016,741,108 | 2,645,607 | 1,019,386,715 |

The raw delta is a conservative candidate delivery comparison because no separately attributable excluded governance lifecycle is available from the aggregate reporter at this mixed-turn boundary. Against the post-plan forecast of 19–48 turns, 4.5–12 summed hours, and 90k–270k output tokens, it exceeded the turn ceiling by 186 turns and the output-token ceiling by 2,375,607 tokens, while finishing 4h 08m 57.894s below the time range. Input remains measured actual only.

The separately recorded post-design-to-post-plan planning and re-estimation delta was 3 turns, 5m 58.358s, 2,566,909 input tokens, and 9,811 output tokens. Accountant capture, re-estimation, and later fixed-rubric release-decider work remain excluded where their lifecycles can be attributed separately; the absolute stop gate includes all work. Candidate cumulative output was 4,183,846 tokens, below the 1,000,000,000-token stop gate.

The published v0.4 binary's additive, unfiltered self-report remains a post-publication milestone artifact rather than being guessed into this pre-review candidate boundary.

## Evidence

The decision uses the completed prototype work on rollout statistics, persisted title updates, recovery behavior, fleet portability, and the Rust multi-architecture assessment. Private task identifiers are intentionally omitted from the public repository.
