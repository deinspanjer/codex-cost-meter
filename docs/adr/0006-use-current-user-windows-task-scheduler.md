# ADR 0006: Use current-user Windows Task Scheduler for idle updates

- Status: Accepted
- Date: 2026-08-14

## Context

Windows x64 now supports the same idle title-update lifecycle as macOS. It
needs native, current-user scheduling without a daemon, a cross-platform
scheduler abstraction, persistent configuration, or an added Windows API
dependency. Scheduler state must preserve the bounded, privacy-safe contract
already used on macOS.

## Decision

Register one fixed current-user Task Scheduler task named `Codex Cost Meter`.
Use `schtasks` with a synchronized temporary XML definition because its `/TR`
form has a command-length limit that can be exceeded by the canonical
executable path and persisted update options. The definition uses a
`RegistrationTrigger` with a five-minute repetition, `InteractiveToken`,
`LeastPrivilege`, and `IgnoreNew`.

Query the exact task name with HRESULT status; only signed `0x80070002`
(`HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND)`) means absent. Remove only that
fixed name and keep removal idempotent.
Write the temporary XML under scheduler state, synchronize it before
registration, and remove it after every attempt. If registration fails, expose
only a bounded, redacted, display-only diagnostic.

Keep scheduler state under `LOCALAPPDATA`, separate from Codex-home selection.
Use the shared bounded status record and fixed remediation values: no
append-only logs, task metadata, IDs, titles, prompts, paths, or arbitrary
errors are persisted. `uninstall` removes scheduler state and starts a
noninteractive PowerShell child that waits for the running executable, retries
its deletion for a bounded period, and deletes its own script. Its deferred
outcome requires a manual deletion fallback.

Verify the lifecycle with fake-runner tests and a native Windows smoke that
uses a copied release executable, exercises absent/install/remove/uninstall,
and performs exact-name cleanup.

## Alternatives considered

- A resident daemon: rejected because Task Scheduler already owns periodic
  execution and the application remains run-once.
- `/TR` registration: rejected because its fixed command-length limit can
  reject a valid executable path plus persisted options.
- A scheduler trait, Windows API crate, or persistent configuration file:
  rejected because one narrow native module and fixed command vectors cover the
  current requirement.
- Immediate self-delete: rejected because Windows cannot reliably unlink the
  running executable.

## Consequences

- The task definition is visible to the current Windows user and necessarily
  contains local executable and Codex-home paths.
- The Windows x64 v0.5 archive remains unsigned; checksum verification and
  organizational trust policy remain necessary before execution.
- A missing or unusable `LOCALAPPDATA`, Task Scheduler access issue, failed
  registration, or failed cleanup is actionable rather than silently guessed.
- Windows arm64 and Linux scheduling remain separate future decisions; this
  decision adds no daemon, shared scheduler abstraction, or dependency.
