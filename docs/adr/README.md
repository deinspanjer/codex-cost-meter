# Architectural decision records

Decisions are ordered by dependency and adoption, not by the date this repository was created.

| ADR | Status | Decision |
| --- | --- | --- |
| [0001](0001-read-task-metadata-from-sqlite-and-usage-from-rollouts.md) | Accepted | Read task metadata from SQLite and usage from rollout JSONL |
| [0002](0002-persist-titles-to-sqlite-and-session-index.md) | Accepted | Persist titles to SQLite and `session_index.jsonl` |
| [0003](0003-use-session-index-updated-at-as-the-high-water-mark.md) | Accepted | Use session-index `updated_at` as the high-water mark |
| [0004](0004-preserve-the-python-prototype-and-port-to-rust.md) | Accepted | Preserve the Python prototype and port the active tool to Rust |
| [0005](0005-use-native-user-schedulers-with-bounded-status.md) | Accepted | Use native user schedulers with bounded status |
| [0006](0006-use-current-user-windows-task-scheduler.md) | Accepted | Use current-user Windows Task Scheduler for idle updates |
| [0007](0007-use-current-user-systemd-scheduler.md) | Accepted | Use a current-user systemd scheduler for idle updates |
| [0008](0008-use-a-source-built-homebrew-tap-and-attested-release-assets.md) | Accepted | Use a source-built Homebrew tap and attested release assets |

Later decisions may amend or supersede these records; do not rewrite an accepted decision to hide a changed direction.
