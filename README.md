# Codex Cost Meter

## Overview

`codex-cost-meter` is a small cross-platform utility for exact-thread usage and estimated API-list-price cost. Its capabilities include descendant accounting, bounded root-title updates, and current-user schedules for eligible idle updates on macOS, Windows, and Linux. It is diagnostic, not a ChatGPT billing record.

v0.7 supports macOS 14+ as a Universal 2 archive for Apple Silicon and Intel, Windows x64 as a native archive, and static musl Linux archives for x86_64 and aarch64. Linux schedules through a current-user systemd service and timer; macOS and Windows use their native current-user schedulers. All three platforms can install, inspect, resume, remove, and uninstall their schedule; Windows arm64 remains deferred. See the user guide for platform requirements and lifecycle details.

![Codex sidebar showing cost and token metrics in task titles](https://raw.githubusercontent.com/deinspanjer/codex-cost-meter/main/docs/assets/sidebar-title-metrics.png)

## Quick start

1. Download the macOS Universal 2, Windows x64, Linux x86_64 musl, or Linux aarch64 musl archive for your platform and its matching checksum from the [latest stable release](https://github.com/deinspanjer/codex-cost-meter/releases/latest).
2. Verify the checksum before extracting the archive.
3. [Find your session ID](https://github.com/deinspanjer/codex-cost-meter/blob/main/USERS.md#find-your-session-id).
4. Run `./codex-cost-meter report <SESSION_ID>` on macOS or Linux, or `& .\codex-cost-meter\codex-cost-meter.exe report <SESSION_ID>` in PowerShell.

For platform-specific download, checksum, extraction, and trust guidance, see [install and run](https://github.com/deinspanjer/codex-cost-meter/blob/main/USERS.md#install-and-run).

The opening of a human report looks like this; the full report continues with per-model and pricing details:

```text
$ ./codex-cost-meter report f8b0c8e4-3dfd-4f33-99e7-9eb2d02f7c71
Codex rollout f8b0c8e4-3dfd-4f33-99e7-9eb2d02f7c71
Project: codex-cost-meter
Name: Example release session
Type: root   Primary: gpt-5.6-terra / high   Descendants: 3

Scope
Scope       Turns                         Input      Cache read  Output   Reasoning  Duration  Cost
----------  ----------------------------  ---------  ----------  -------  ---------  --------  -----
Root        1 (1 complete, 0 incomplete)  125K       100K        18K      12K        3m 1.0s   $0.29
Whole tree  4 (4 complete, 0 incomplete)  2M         1.7M        145K     100K       12m 4.0s  $3.89
```

You can also [ask Codex to run the downloaded tool](https://github.com/deinspanjer/codex-cost-meter/blob/main/USERS.md#ask-codex-to-run-it). The [user guide](https://github.com/deinspanjer/codex-cost-meter/blob/main/USERS.md#find-your-session-id) explains how `/status` shows the session ID and `/statusline` keeps it visible.

## Documentation

The [user guide](https://github.com/deinspanjer/codex-cost-meter/blob/main/USERS.md), [developer guide](https://github.com/deinspanjer/codex-cost-meter/blob/main/DEVELOPERS.md), and [future work](https://github.com/deinspanjer/codex-cost-meter/blob/main/TODO.md) are maintained separately. The [`python-prototype/`](https://github.com/deinspanjer/codex-cost-meter/tree/main/python-prototype) directory is an archived historical reference.

## License

Licensed under the [MIT License](LICENSE).
