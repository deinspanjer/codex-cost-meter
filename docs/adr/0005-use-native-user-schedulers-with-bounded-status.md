# ADR 0005: Use native user schedulers with bounded status

- Status: Accepted
- Date: 2026-08-14

## Context

Periodic idle title updates need a macOS lifecycle without turning the utility into a resident service or persisting private diagnostic data. The updater can fail repeatedly for transient or actionable local reasons, and an append-only scheduler log would unnecessarily retain task metadata or arbitrary system errors.

## Decision

Use one current-user macOS LaunchAgent managed by the run-once executable. The installed job invokes the executable's internal scheduled command at load and on a fixed five-minute interval. Its argument set is bounded to idle selection and explicit application; it does not persist explicit task or title selections.

Store one atomically replaced, owner-readable status record with a nullable run time, stable result code, capped consecutive-failure count, pause state, and fixed allowlisted remediation. Never store task metadata, paths, IDs, titles, prompts, command output, or arbitrary errors in that record. Do not create a default append-only log.

Treat updater lock contention as a successful no-op. Reset the failure count on success, pause after three consecutive ordinary failures, and pause immediately for disk-full, incompatible-schema, or permission-denied failures. Require an explicit resume after remediation.

Use fixed macOS tools with argument vectors, and keep the process boundary private behind a fake-runner test seam. Removal is idempotent and owns only this tool's property list and status record. Uninstall performs that removal and unlinks only the canonical current executable; it never deletes a parent directory or Codex data.

## Alternatives considered

- A resident daemon: rejected because launchd already owns periodic execution and the application remains run-once.
- An append-only scheduler log: rejected because bounded status is sufficient for remediation and avoids durable private diagnostics.
- A cross-platform scheduler abstraction: rejected because no second platform implementation exists yet.

## Consequences

- Moving or deleting an installed executable invalidates its job and requires removal or reinstallation from the new location.
- macOS scheduling tests use temporary homes and the fake runner; they do not register a real LaunchAgent.
- Future platform schedulers remain native implementations and require their own durable decision when introduced.
