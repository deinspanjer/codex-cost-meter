# User guide

## Install and run

### Homebrew on macOS

Homebrew is the recommended macOS path. The custom tap builds the pinned release source with its committed `Cargo.lock` and links the command onto `PATH`:

```text
brew tap deinspanjer/codex-cost-meter https://github.com/deinspanjer/codex-cost-meter
brew install deinspanjer/codex-cost-meter/codex-cost-meter
codex-cost-meter report <THREAD_ID>
```

After `brew upgrade codex-cost-meter`, rerun `codex-cost-meter schedule install` if scheduled updates were installed; the schedule stores the resolved versioned Cellar path. To uninstall, use `codex-cost-meter schedule remove` followed by `brew uninstall codex-cost-meter`. Do not run the utility's self-uninstall on a Homebrew-owned executable.

### Cargo source install

With Rust 1.97.1 or newer installed, macOS, Windows, and Linux can build a pinned release directly from its tag:

```text
cargo install --locked --git https://github.com/deinspanjer/codex-cost-meter --tag v<VERSION> codex-cost-meter
```

Replace `<VERSION>` with the latest stable version. Cargo installs to its configured binary directory, normally `$CARGO_HOME/bin` or `~/.cargo/bin` on macOS and Linux and `%USERPROFILE%\.cargo\bin` on Windows. A local source build avoids the browser-download path, but it does not establish an Apple or Microsoft publisher identity or override organizational application-control policy. Remove a schedule before running `cargo uninstall codex-cost-meter`.

### Direct release archives

Use a direct archive when installing Homebrew or a Rust toolchain is not worthwhile:

1. Open the [latest stable release](https://github.com/deinspanjer/codex-cost-meter/releases/latest). Download the macOS Universal 2 `.tar.gz` archive, Windows x64 `.zip` archive, Linux x86_64 musl `.tar.gz` archive, or Linux aarch64 musl `.tar.gz` archive, plus that archive's matching `.sha256` file.
2. Verify the download before extracting it. On macOS, run `shasum -a 256 -c codex-cost-meter-v<VERSION>-macos-universal2.tar.gz.sha256`. On Linux, run `sha256sum -c codex-cost-meter-v<VERSION>-linux-x86_64-musl.tar.gz.sha256` or `sha256sum -c codex-cost-meter-v<VERSION>-linux-aarch64-musl.tar.gz.sha256`. In PowerShell on Windows, run `& { $archive = 'codex-cost-meter-v<VERSION>-windows-x64.zip'; $expected = (Get-Content "$archive.sha256" -Raw).Trim().Split()[0]; if ((Get-FileHash $archive -Algorithm SHA256).Hash -ne $expected) { throw "checksum mismatch" } }`.
3. Extract the verified archive: `tar -xzf codex-cost-meter-v<VERSION>-macos-universal2.tar.gz` on macOS, `tar -xzf codex-cost-meter-v<VERSION>-linux-x86_64-musl.tar.gz` or `tar -xzf codex-cost-meter-v<VERSION>-linux-aarch64-musl.tar.gz` on Linux, or `Expand-Archive codex-cost-meter-v<VERSION>-windows-x64.zip -DestinationPath .\codex-cost-meter` in PowerShell.
4. [Copy the session ID from the Codex app](#find-your-session-id), then run `./codex-cost-meter report <THREAD_ID>` on macOS or Linux, or `& .\codex-cost-meter\codex-cost-meter.exe report <THREAD_ID>` in PowerShell.

Place a directly downloaded binary in a directory you choose. If you have no preference, use `~/.codex/codex-cost-meter` on macOS or Linux, or `$env:USERPROFILE\.codex\codex-cost-meter.exe` in PowerShell on Windows. The Windows location is a suggestion, not a requirement.

The Windows ZIP contains `codex-cost-meter.exe`, `README.md`, and `LICENSE`.

macOS and Linux examples:

```text
~/.codex/codex-cost-meter report <THREAD_ID>
~/.codex/codex-cost-meter report <THREAD_ID> --json
~/.codex/codex-cost-meter report <THREAD_ID> --refresh
~/.codex/codex-cost-meter report <THREAD_ID> --codex-home <PATH>
```

Windows uses the same `report` and `update` arguments with `codex-cost-meter.exe`. In PowerShell, replace the placeholder before running:

```text
$threadId = "<THREAD_ID>"
& "$env:USERPROFILE\.codex\codex-cost-meter.exe" report $threadId
& "$env:USERPROFILE\.codex\codex-cost-meter.exe" update --thread-id $threadId
```

Use the bare executable name only when you place it in an existing directory on `PATH`; the tool does not require or create a `bin` directory under the Codex home. The tool resolves the Codex directory in this order: `--codex-home`, `CODEX_HOME`, then the platform home plus `.codex`. `report` never changes Codex task data. It may read Codex's SQLite projection to narrow discovery, with rollout-file fallback; `update` is dry-run by default and reads the supported local SQLite state for title-update selection or application (details below).

The first successful analysis creates `codex-cost-meter.sqlite` in the selected Codex home and prints its path once. This app-owned, disposable cache stores rollout metadata and parsed usage facts; report pricing and aggregation remain live. An entry is reused only when the rollout file's modification time and size match. `report --refresh` and `update --refresh` reprocess only the selected roots and linked descendants.

If an existing cache can be read but not written, the command reuses compatible entries read-only, analyzes misses without storing them, and warns that it must be run with write permission to update the cache. If no cache exists, the command attempts to create one; a permission failure warns that write access is required and continues uncached. An unusable existing cache likewise produces one unavailable warning and continues without caching. To rebuild the cache manually, remove only `codex-cost-meter.sqlite` while no meter command is running; the next analysis recreates it. The cache is separate from Codex's `state_5.sqlite` and is not removed by schedule removal or uninstall.

The current release workflow produces a macOS archive containing a Developer ID-signed, hardened-runtime, timestamped, and notarized Universal 2 executable. Because a standalone executable cannot carry a stapled notarization ticket, Gatekeeper may need network access to retrieve its ticket from Apple on first execution. Older releases retain the signature state they had when published. The Windows executable remains unsigned, so Windows or organizational controls can require an explicit trust decision or block it. Verify the downloaded checksum and follow your organization's trust process; checksums and GitHub artifact attestations establish integrity or provenance independently of platform publisher identity. See [direct release trust and remaining unsigned paths](docs/unsigned-releases.md) for details.

## Find your session ID

The report and explicit `update --thread-id` commands need a Codex session ID. If the root thread ID is not already in the Codex CLI status line, run `/status` to show it. To keep it visible between commands, run `/statusline` and add **Current thread identifier**. The app also offers two convenient ways to copy it:

1. Right-click the task in the sidebar and select **Copy session ID**.

   ![Codex sidebar task menu with Copy session ID selected](https://raw.githubusercontent.com/deinspanjer/codex-cost-meter/main/docs/assets/session-id-sidebar-menu.png)

2. Open the task header's ellipsis menu, select **Copy**, then select **Copy session ID**.

   ![Codex task header menu leading to Copy session ID](https://raw.githubusercontent.com/deinspanjer/codex-cost-meter/main/docs/assets/session-id-header-menu.png)

Paste that value in place of `<THREAD_ID>` or `<SESSION_ID>` in the examples. A session ID is often useful for support and reproducibility, but it can still identify a private task; review it before sharing.

## Ask Codex to run it

If you already have the executable and a copied session ID, you can ask Codex to run the report for you:

```text
Run <PATH_TO_CODEX_COST_METER> report <SESSION_ID> and explain the result.
```

Codex follows your normal approval settings for the local command. Review the result yourself before sharing it: reports can include task IDs, titles, project paths, and usage information.

## Read the report

Human output shows root and whole-tree totals, per-model usage, price metadata, and summed agent-turn time. A model with one service mode is labeled directly, such as `[Standard]`, `[Standard*]`, or `[Fast]`; only models with mixed modes receive child rows. `Standard*` combines explicit and assumed Standard usage and is explained once below the report, while JSON keeps `standard` and `assumed_standard` separate. Turns and duration stay on the aggregate model row because a tier can change between usage events within one turn; the tool does not guess how to split them. An empty Models section says `No model usage.` instead of showing a Total-only table. Token columns use compact `K`/`M`/`B` notation; JSON retains exact integers. Durations use `d`/`h`/`m`/`s`, omitting zero day, hour, and minute portions; seconds can be fractional. Model and aggregate durations can overlap. A trailing `+` on a human cost means the displayed amount is only the known partial cost. In JSON, the corresponding complete estimate is `null`; inspect `incomplete_input`, `unpriced_models`, `unpriced_service_tiers`, `unattributed_usage_tokens`, and `incomplete_input_warnings` before treating an estimate as complete.

### Corpus and date reports

Use `--all` for one aggregate across every discovered rollout, including roots, subagents, reviews, compactions, and other internal rollout types. Rollout type is reported as a breakdown and can be used as a grouping dimension; it is not a selection filter.

```text
codex-cost-meter report --all
codex-cost-meter report --all --since 2026-08-01 --through 2026-08-31
codex-cost-meter report --all --since 2026-08-01 --through 2026-08-31 --group-by day
codex-cost-meter report --project . --since 2026-08-01 --through 2026-08-31 --group-by week,rollout-type
codex-cost-meter report --all --since 2026-08-01 --through 2026-08-31 --group-by month,model --include-empty
```

`--since` and `--through` are inclusive `YYYY-MM-DD` bounds in the operating system's local timezone. Either bound may be omitted. Day and month buckets use local calendar boundaries, and weeks start on Monday. `--group-by` requires exactly one of `day`, `week`, or `month`, plus optional `rollout-type` and/or `model`. Empty periods are omitted unless `--include-empty` is present; that option requires both bounds and emits one zero-valued time-only row without inventing a model or rollout type.

For `--all`, available Codex thread metadata narrows discovery before rollout files are opened: `--since` skips indexed rollouts last updated before its local date, and `--through` skips indexed rollouts created after its local date. Each bound works independently. Unindexed rollouts, unusable timestamps, and unavailable metadata remain candidates, so the command falls back toward a full scan rather than hiding data. Retained rollouts still use the event- and turn-level filtering below. This metadata pruning does not apply to project reports.

Date filtering attributes tokens and cost to each usage-event timestamp, while turns and their complete duration are attributed to the turn's start date. Data with neither an event nor session timestamp remains in unfiltered lifetime totals but is excluded and visibly marked incomplete in a filtered report. Date and grouping options apply to `--all`, `--project`, and the bare current-project report, not an exact session-ID report. JSON retains the overall `tree` total and adds `date_range`, `by_rollout_type`, and `groups`.

Cost uses the embedded historical catalog and is an API-list-price approximation, not a billing record. Each request uses the service tier most recently applied in the rollout settings; current Codex token records do not reveal the tier actually served, so a Fast request that was downgraded cannot be corrected to Standard pricing. Missing tier metadata is priced at Standard rates. It is definitive Standard only when the usage timestamp and creator version place it before the first public Fast-capable package; otherwise human output marks the combined Standard mode as `Standard*` and JSON contributes it to `assumed_standard_tokens` and `by_service_tier.assumed_standard`. Explicit unsupported tiers remain unpriced.

`codex-auto-review` is an internal routing identity, not a public foundation-model ID recorded in local telemetry. OpenAI documented GPT-5.4 Thinking with low reasoning at launch and announced a migration to GPT-5.6 Luna on July 30, 2026. The catalog therefore prices Auto-review through GPT-5.4 before July 30 and GPT-5.6 Luna from July 30 onward. This is an announcement-date estimator boundary, not proof of the routed or billed model for an individual request or an exact account-level cutover. Human output shows the dated mapping; JSON preserves the latest target in `model_proxies` and the full typed history in `model_proxy_histories`. See the [Auto-review pricing evidence](docs/research/codex-auto-review-pricing-evidence.md).

Fast first appeared in public opt-in prerelease `0.108.0-alpha.2` on March 2 PST (March 3 UTC) and became generally available in stable `0.111.0` on March 5. Applied-tier snapshots were not persisted until stable `0.144.0` on July 9. The analyzer uses each usage event's timestamp because the canonical `session_meta.cli_version` identifies the rollout creator, not the client that may later resume and append to it.

Requests above a model's published input threshold use its long-context rate cell. Explicit Priority/Fast markers at or after stable `0.144.0` use the model-specific Fast premium captured in the embedded catalog; missing attribution always uses the Standard rate, with post-release assumptions labeled separately. Fast markers before the exact stable `0.144.0` publication time remain unpriced because durable rollout evidence was not yet available. Unsupported applied tiers appear under `unpriced_service_tiers`; unavailable model, date, tier, context, or token-component rates appear under `unpriced_models`. Both make the complete estimate unavailable while preserving known cost. Reasoning is included in output usage; cache reads are included in input usage. See the [attribution evidence](docs/research/fast-mode-attribution-evidence.md) for the official timeline and representative local chronology.

### Larger worked example

This deterministic worked example uses a synthetic session ID and fixture data. It shows why the whole-tree row, per-model rows, price metadata, and agent-turn time belong together when a task has descendants.

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

Models
Model                         Turns                         Input      Cache read  Output   Reasoning  Duration  Cost
----------------------------  ----------------------------  ---------  ----------  -------  ---------  --------  -----
gpt-5.6-sol [Fast]            1                             1M         850K        40K      28K        4m 0.0s   $2.38
gpt-5.6-terra                 2                             725K       600K        93K      64K        6m 4.0s   $1.49
↳ Standard                                                  400K       325K        45K      31K                  $0.80
↳ Fast                                                      325K       275K        48K      33K                  $0.69
codex-auto-review [Standard]  1                             300K       275K        12K      8K         2m 0.0s   $0.02
Total                         4 (4 complete, 0 incomplete)  2M         1.7M        145K     100K       12m 4.0s  $3.89

Agent-turn time: 9m 3.0s (agent time can overlap).
Pricing as of: 2026-08-22
Pricing basis: API list pricing; applied rollout tier (served tier unavailable); per request model/context; output
               includes reasoning
Pricing sources:
  - https://developers.openai.com/api/docs/pricing
  - https://openai.com/api-fast-mode/
Model proxies:
  codex-auto-review before 2026-07-30 -> gpt-5.4
  codex-auto-review from 2026-07-30 -> gpt-5.6-luna
  Note: codex-auto-review boundaries are announcement-date estimates, not observed routing or billing cutovers.
  gpt-5.6 -> gpt-5.6-sol
Notes: cache read is included in input; reasoning is included in output; agent time can overlap.
```

The worked example uses the catalog's effective-dated Auto-review proxy history. The July 30 boundary remains an evidence-backed estimate rather than observed request routing.

## Preview or apply title updates

`update` selects root tasks only. It never changes a title unless `--apply` is present, so begin with a dry run and review every proposed `id: old -> new` line:

```text
~/.codex/codex-cost-meter update --thread-id <THREAD_ID>
~/.codex/codex-cost-meter update --match-title "unique title text"
~/.codex/codex-cost-meter update --idle-minutes 15 --limit 20
~/.codex/codex-cost-meter update --thread-id <THREAD_ID> --refresh
~/.codex/codex-cost-meter update --thread-id <THREAD_ID> --apply
```

`--thread-id` and `--match-title` are repeatable and can be combined; a case-insensitive title match must resolve to exactly one root. `--idle-minutes` is an alternative selection mode. It chooses eligible idle roots newest first, after filtering, and accepts `--limit` (default 20), `--max-runtime SECONDS|MINUTESm`, and `--reprice-before` with an ISO date or RFC 3339 timestamp with a timezone. A repricing cutoff cannot be in the future.

Titles default to `--title-metrics cost,total-tokens` and `--max-width 65`. Choose an ordered comma-separated subset of `cost`, `total-tokens`, `input-tokens`, and `output-tokens`, or use `all`; duplicate metrics and a width too small for a visible base and suffix are rejected. The tool removes only its trailing canonical metric suffix before recomposing a title, preserves Unicode-scalar width, and marks incomplete known cost with `+`.

For dry runs, the tool opens Codex's state database read-only. With `--apply`, it locks one updater process, updates both `title` and `name` in a single SQLite transaction, then appends durable JSONL entries to `session_index.jsonl`. It supports `state_5.sqlite` directly under the selected Codex home or under `sqlite/`, and requires the `threads` columns `id`, `title`, `name`, `history_mode`, `updated_at`, and `first_user_message`; extra schema is accepted. SQLite and JSONL are deliberately not cross-store atomic: if index writing fails after a committed transaction, the command fails and a later identical `--apply` remains eligible to repair both stores. A busy updater, database/schema problem, or unreadable index is a safe actionable failure.

## Schedule idle title updates

On Linux, `schedule install` creates one current-user systemd service and timer. On macOS, it creates one current-user LaunchAgent. On Windows, it registers one current-user Task Scheduler task named `Codex Cost Meter`. Each starts once when enabled or loaded and then runs every five minutes. It applies updates only to eligible idle root tasks and uses these defaults: `--idle-minutes 15`, `--limit 500`, `--max-runtime 4m`, `--max-width 65`, and `--title-metrics cost,total-tokens`.

```text
<PATH_TO_BINARY> schedule install
<PATH_TO_BINARY> schedule install --idle-minutes 30 --limit 100 --max-runtime 2m
<PATH_TO_BINARY> schedule status
<PATH_TO_BINARY> schedule resume
<PATH_TO_BINARY> schedule remove
<PATH_TO_BINARY> uninstall
```

On Linux, this requires a working systemd user manager and user bus. Minimal or headless environments commonly lack one, so `systemctl --user` cannot install, remove, or operate the timer there; use explicit `update` commands or arrange the host's user-systemd environment before installing. The fixed unit files are `$XDG_CONFIG_HOME/systemd/user/io.github.deinspanjer.codex-cost-meter.service` and `.timer`, falling back to `~/.config/systemd/user/` when `XDG_CONFIG_HOME` is unset, empty, or not absolute. The bounded schedule status is `$XDG_STATE_HOME/codex-cost-meter/status.json`, falling back to `~/.local/state/codex-cost-meter/status.json` when `XDG_STATE_HOME` is unset, empty, or not absolute.

On Windows PowerShell, invoke the suggested install location as `& "$env:USERPROFILE\.codex\codex-cost-meter.exe" schedule install`; use the actual path if you chose another location. `schedule install` accepts the displayed defaults as overrides, plus `--reprice-before` and `--codex-home`; the scheduled command always applies updates and does not accept explicit thread or title selection. The job stores the canonical path of the executable. Moving or deleting that binary breaks the installed job; run `schedule remove` before moving it, then install again from its new location.

`schedule status` reports whether the Linux units are installed and the timer is active, whether the macOS property list is installed and loaded, or whether the Windows task is registered, followed by the bounded status record. When a run has occurred, its fields are `last run`, stable `result`, `consecutive failures`, `paused`, and fixed remediation; otherwise it reports `last run: never`. `schedule resume` clears a pause without re-registering the schedule. `schedule remove` removes only this tool's fixed Linux units, macOS LaunchAgent, or Windows task and its bounded status record; on Linux it disables and stops the fixed timer before deleting its unit files and reloading user units, and on Windows it also removes an abandoned temporary task-definition file. It is safe to repeat after successful removal.

On Windows, scheduling commands require a nonempty `LOCALAPPDATA`; they store the bounded status at `%LOCALAPPDATA%\codex-cost-meter\status.json`. `--codex-home` and `CODEX_HOME` select Codex storage only and do not replace `LOCALAPPDATA`. Task Scheduler keeps the registered task definition for the current user, including the executable and Codex-home paths. On Linux and macOS, `uninstall` removes the schedule and deletes only the currently running executable. On Windows, it removes the schedule first and starts a short-lived cleanup process that deletes the executable after it exits; `executable deletion scheduled` does not mean deletion has already completed. If the executable remains after the process has exited, delete that exact `.exe` manually. No platform's uninstall deletes Codex data or a parent directory.

Scheduled runs create no append-only log and are normally silent on success and ordinary lock contention. If an update succeeds but its bounded status cannot be persisted, it prints only `update completed; schedule status unavailable`. They store only fixed result codes and remediation, not task metadata, paths, IDs, titles, prompts, or arbitrary error text. Three consecutive ordinary failures pause the schedule; disk-full, incompatible SQLite schema, and permission-denied failures pause it immediately. Use `schedule status`, correct the local issue, then run `schedule resume`.

## App display limitations

- A scheduled update can write the title successfully without changing an already-open ChatGPT/Codex App window. Restart the app or open **File > New Window** to see the updated title.
- Opening a task can cause Codex to regenerate its title and temporarily remove the cost suffix. The next successful scheduled update restores it, but its display can still wait for a new window as described above.

## Privacy and troubleshooting

Reports read local Codex JSONL files. Reports, update previews, warnings, and runtime errors can include thread names, thread IDs, old/new titles, and local project paths, so review all output before sharing. Human output and runtime errors sanitize control characters and whitespace to prevent terminal or line injection; sanitization does not remove private values. JSON stays structured for programmatic use.

- **`rollout not found`** — confirm the exact ID and the selected Codex home; active and archived sessions are scanned.
- **Partial cost or warnings** — retain the result as incomplete. Unknown prices, ambiguous history, malformed or oversized JSONL, and unreadable nonselected inputs are intentionally not guessed.
- **`another title updater is already running`** — wait for the other updater to finish, then rerun the dry run or apply command.
- **Database, schema, or session-index error** — do not edit Codex storage by hand. Resolve the local access issue and rerun; a failed index append after commit is intentionally recoverable by rerunning the same selection with `--apply`.
- **Scheduled updates are paused** — use `schedule status` for the fixed remediation, correct the reported storage, schema, or permission problem, then run `schedule resume`.
- **Scheduled job no longer starts** — if the binary was moved or deleted, run `schedule remove` if possible and install again from its new location.
- **Windows scheduling state cannot be resolved** — run from a normal Windows user session where `LOCALAPPDATA` is nonempty; `--codex-home` cannot replace it.
- **Windows task is not registered or Task Scheduler cannot be inspected** — use `schedule status` to confirm the state, then run `schedule install` again after resolving the current-user Task Scheduler or access problem.
- **Windows uninstall left the executable behind** — wait until `uninstall` has exited, then delete that exact executable manually. It is safe to rerun `schedule remove`; it does not use a wildcard task name.
- **Linux scheduling cannot contact the user manager** — use a normal logged-in systemd user session with a working user bus, then rerun `schedule install`; `--codex-home` does not replace the user manager or change XDG scheduler paths.
- **Linux timer no longer starts** — run `schedule status`, then reinstall from the binary's current location after resolving the user-session issue. `schedule remove` affects only this tool's two fixed unit files and bounded status.
- **macOS blocks execution** — for releases produced after production signing was enabled, recheck the archive checksum and network access, then follow your organization’s Gatekeeper policy. Users do not install publisher certificates manually.
- **Windows blocks execution** — recheck the ZIP checksum, then follow your organization’s application-control or SmartScreen policy. The unsigned executable does not establish publisher identity.
