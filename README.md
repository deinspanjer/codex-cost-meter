# Codex Cost Meter

`codex-cost-meter` is a small macOS utility that reports token usage and an estimated API-list-price cost for one exact Codex thread ID and its descendants. It can also preview or explicitly apply bounded cost-and-token titles to selected root tasks. It is diagnostic, not a ChatGPT billing record.

v0.2 supports macOS 14+ as a Universal 2 archive for Apple Silicon and Intel. Download the release archive and its checksum from the project's releases; it contains only `codex-cost-meter`, this README, and the license. With no preferred install directory, use `~/.codex/codex-cost-meter`; see the [user guide](https://github.com/deinspanjer/codex-cost-meter/blob/main/USERS.md) for details.

```text
codex-cost-meter report <THREAD_ID>
codex-cost-meter update --thread-id <THREAD_ID>
```

Read the [user guide](https://github.com/deinspanjer/codex-cost-meter/blob/main/USERS.md) for installation, report/update workflows, privacy, and troubleshooting. [Developer guidance](https://github.com/deinspanjer/codex-cost-meter/blob/main/DEVELOPERS.md) covers architecture and release validation; [future work](https://github.com/deinspanjer/codex-cost-meter/blob/main/TODO.md) is kept separately. The [`python-prototype/`](https://github.com/deinspanjer/codex-cost-meter/tree/main/python-prototype) directory is an archived historical reference.

## License

Licensed under the [MIT License](LICENSE).
