# TODO

- Scaffold the Rust 2024 implementation as one crate and port the behavioral contracts from the Python self-tests.
- Implement cross-platform rollout discovery, parsing, descendant accounting, historical pricing, and title formatting.
- Implement safe SQLite and `session_index.jsonl` updates with narrow macOS and Windows locking adapters.
- Add CI builds and focused tests for macOS arm64, macOS x86_64, and Windows x64.
- Define packaging, signing, and release artifacts for the supported architectures.
- Document macOS LaunchAgent/Jamf and Windows Task Scheduler/Intune or MECM deployment after those packages exist.
- Decide whether Windows arm64 is warranted from deployment inventory.
