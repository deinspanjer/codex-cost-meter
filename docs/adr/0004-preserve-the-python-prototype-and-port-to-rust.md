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

## Evidence

The decision uses the completed prototype work on rollout statistics, persisted title updates, recovery behavior, fleet portability, and the Rust multi-architecture assessment. Private Codex task identifiers are intentionally omitted from the public repository.
