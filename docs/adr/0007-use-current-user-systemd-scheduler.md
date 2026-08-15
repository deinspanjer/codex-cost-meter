# ADR 0007: Use a current-user systemd scheduler for idle updates

- Status: Accepted
- Date: 2026-08-15

## Context

Linux musl x86_64 and aarch64 now support the same idle title-update lifecycle
as macOS and Windows. Linux needs a native, current-user scheduler without a
resident service, a cross-platform scheduler abstraction, persistent
configuration, or an additional dependency. Scheduler state must retain the
existing bounded, privacy-safe contract.

## Decision

Install fixed current-user systemd units named
`io.github.deinspanjer.codex-cost-meter.service` and
`io.github.deinspanjer.codex-cost-meter.timer`. Put the units under
`$XDG_CONFIG_HOME/systemd/user` (falling back to `~/.config/systemd/user`) and
the bounded status record under `$XDG_STATE_HOME/codex-cost-meter/status.json`
(falling back to `~/.local/state`).

The timer starts at activation and repeats every five minutes. Its oneshot
service runs the canonical executable's internal `schedule run` command with
the fixed, explicitly selected idle-update options. Quote all dynamic unit
arguments for systemd and reject a non-absolute Codex home or non-UTF-8 path
before writing a unit.

Manage the units only through fixed `systemctl --user` command vectors.
Install atomically replaces the two units, reloads the user manager, then
enables and starts the timer. Inspect treats only systemctl's documented
inactive and missing exit codes as non-errors. Removal disables the fixed timer,
removes only these two units and the bounded status record, and reloads the
user manager; it is idempotent. Uninstall performs removal, then deletes only
the canonical current executable.

Verify the lifecycle through a private fake-runner seam and native Linux CI;
do not install a real unit on hosted runners, which can lack a user bus.

## Alternatives considered

- A resident daemon: rejected because systemd owns periodic activation and the
  executable remains run-once.
- A cross-platform scheduler abstraction or systemd library: rejected because
  one narrow native module and fixed command vectors cover the current
  requirement.
- A real hosted-CI installation test: rejected because it depends on a user
  systemd bus that hosted runners do not reliably provide.

## Consequences

- Moving or deleting an installed executable invalidates the service and
  requires removal or reinstallation from its new location.
- The current user can inspect the unit, which necessarily contains local
  executable and Codex-home paths.
- Missing systemd user support, unavailable `systemctl`, failed unit writes,
  failed registration, or failed cleanup is actionable rather than silently
  guessed.
