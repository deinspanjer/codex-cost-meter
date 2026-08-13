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

Preserve the Python scripts, embedded self-tests, documentation, and sanitized LaunchAgent template under `python-prototype/`. Build the active replacement as a self-contained Rust 2024 executable for macOS arm64, macOS x86_64, and Windows x64.

Use one crate while the project is small. Introduce another crate only for a demonstrated ownership, reuse, or build boundary. Keep the executable run-once and let each operating system own scheduling.

Port the behavioral decisions and verified invariants, not the Python module layout.

## Alternatives considered

- Continue distributing Python: rejected because Python is not a reliable clean-host dependency on either target platform and the updater already contains Unix/macOS-specific behavior.
- Bundle a Python runtime: rejected because a native executable has a simpler runtime and deployment footprint for this small utility.
- Build a long-running cross-platform daemon: rejected because periodic OS scheduling is sufficient.
- Split immediately into multiple Rust crates: rejected as speculative structure.

## Consequences

- Release work must produce and sign native artifacts for each supported architecture.
- Windows needs a locking implementation and scheduler/deployment package distinct from macOS.
- New product work belongs in Rust; the Python prototype remains runnable for comparison and regression discovery.
- The port remains coupled to version-sensitive internal Codex storage until a supported API replaces it.

## Pre-implementation estimate

The final estimate from the pre-implementation Codex assessment on 2026-08-13 was:

| Outcome | Agent turns | Active agent session time | Approximate tokens |
| --- | ---: | ---: | ---: |
| Minimal Rust port with fixture tests | 1–2 | 45–90 minutes | 40k–80k |
| macOS and Windows build artifacts through CI | 2–3 total | 1.5–3 hours | 70k–140k |
| Signed packages, schedulers, and uninstallers | 4–6 total | 3–5 hours | 110k–220k |
| Pilot verification on real Mac and Windows state | 5–8 total | 4–7 hours | 140k–280k |

The summary estimate for the basic cross-platform executable was **2–3 turns, about 2 hours of active agent session time, and 80k–140k tokens**. A turn meant one sustained Codex work cycle containing many tool calls, builds, and test iterations. The estimate assumed no prolonged dependency, signing, credential, or Windows-host failures.

When the project reaches each outcome, record actual turns, active agent session time, and token usage beside this baseline. Keep packaging and pilot totals separate from the basic executable so the comparison does not change the definition of “done.”

## Evidence

The decision uses the completed prototype work on rollout statistics, persisted title updates, recovery behavior, fleet portability, and the Rust multi-architecture assessment. Private Codex task identifiers are intentionally omitted from the public repository.
