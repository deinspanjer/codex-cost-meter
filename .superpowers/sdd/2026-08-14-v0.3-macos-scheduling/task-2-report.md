# Task 2 report: bounded schedule status and circuit breaker

## Outcome

Implemented the Task 2 status-only foundation in commit `1a491e2` (`feat: add bounded schedule status`).

## Files

- Created `src/schedule.rs` with the stable serialized result codes, pure status transitions, typed status-storage errors, bounded reads, allowlisted remediation validation, and atomic private writes.
- Modified `src/main.rs` only to include the new internal module.

No runtime dependencies were added.

## TDD evidence

### RED

Command:

```text
cargo test schedule::tests -- --nocapture
```

Result: exited `101` at compile time with the expected unresolved imports for `ResultCode`, `Status`, `StatusError`, `after_failure`, `after_success`, `read_status`, `resume_status`, and `write_status`. This demonstrated that the tests named the missing status API rather than a fixture or environment problem.

### GREEN

Commands:

```text
cargo test schedule::tests -- --nocapture
cargo fmt && cargo test schedule::tests && cargo test
git diff --check
cargo fmt --check
```

Results:

- Focused schedule tests: 7 passed.
- Full suite: 63 unit tests, 12 `report_cli` integration tests, and 6 `update_cli` integration tests passed.
- `git diff --check` and `cargo fmt --check` exited successfully.

The focused tests cover third ordinary failure pause, immediate severe pauses, saturation, success/reset, resume time preservation, JSON round-trip, same-directory temporary-file cleanup, Unix `0600`, malformed JSON, unallowlisted remediation, oversized input, over-limit counts, and write-time privacy validation.

## Scope choices and review

- The module owns only serializable status state and filesystem persistence. It adds no CLI dispatch, LaunchAgent handling, process execution, background work, or platform abstraction.
- Remediation text is chosen only from a fixed `ResultCode` allowlist and revalidated on both read and write, so a caller cannot persist task metadata or an arbitrary error string.
- The status read is capped at 4096 bytes before JSON parsing. Writes create only the supplied parent directory, write a same-directory PID-named temporary file, set private Unix permissions before data is written, flush and synchronize it, then rename it over the target.
- The mutation checks requested by the brief are covered by named tests: `failure_transitions_pause_after_three_ordinary_failures_or_one_severe_failure`, `ordinary_failures_saturate_at_three`, `malformed_or_unallowlisted_status_is_rejected`, `oversized_or_over_limit_status_is_rejected`, and `success_and_resume_clear_the_circuit_breaker_without_changing_resume_time`.

## Concerns

`cargo test` reports `dead_code` warnings for the staged Task 2 internal APIs (and Task 1's `FailureClass`) because Task 4 has not yet integrated schedule execution. No lint suppression was added; Task 4 is the intended consumer. `just check` was therefore not run for this isolated task, while all commands explicitly requested in the brief passed.

## Review round 1: bounded-read correction

Finding: the initial implementation requested `MAX_STATUS_BYTES + 1` bytes from the opened status file to detect overflow, so its 4097-byte read exceeded the 4096-byte contract.

### RED

I first changed only the reader cap to `take(MAX_STATUS_BYTES)` and ran:

```text
cargo test schedule::tests::oversized_or_over_limit_status_is_rejected -- --nocapture
```

It failed at the existing 4097-byte status assertion because the bounded partial input was otherwise classified as malformed, proving that the existing oversized-input behavior requires a pre-read size check.

### GREEN

`read_status` now maps `File::metadata` failures to `StatusError::Read`, rejects a file whose metadata length is above 4096, and then reads with `take(MAX_STATUS_BYTES)`. The read is therefore never larger than 4096 bytes; a file that grows after metadata inspection remains bounded by the same read cap.

Commands:

```text
cargo fmt && cargo test schedule::tests -- --nocapture && cargo fmt --check && git diff --check
```

Result: all 7 focused schedule tests passed, including the preserved 4097-byte oversized-input regression. The staged unused-API warnings remain unchanged and are expected until Task 4 consumes the API.
