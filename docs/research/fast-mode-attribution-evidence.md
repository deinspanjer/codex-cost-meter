# Fast-mode attribution evidence

Research date: 2026-08-23.

## Conclusions

1. **March has separate pre-release, gated-stable, and ordinary-customer boundaries.** Commit [`2f5b01a`](https://github.com/openai/codex/commit/2f5b01abd605dfa1304b3b8a12b0033ddf020c75) added `/fast` and sent `service_tier=priority`. Public prerelease [`0.108.0-alpha.2`](https://github.com/openai/codex/releases/tag/rust-v0.108.0-alpha.2) contained it, but the feature was explicitly `UnderDevelopment`, “not ready for external use,” hidden from `/experimental`, and default-off. `0.108.0` and `0.109.0` exist as source tags but were not published as stable `@openai/codex` packages. [`0.110.0`](https://github.com/openai/codex/releases/tag/rust-v0.110.0) was the first installable stable package containing the still-gated toggle. [`0.111.0`](https://github.com/openai/codex/releases/tag/rust-v0.111.0) was the first stable package with Fast enabled by default for ordinary CLI users.
2. **The repository records 2026-08-22 as its price-source capture date.** For best-effort historical estimates, the calculator applies the captured per-model Fast premiums only to explicit Fast/Priority markers at or after stable `0.144.0`, the first stable release that could persist such a marker. Missing attribution always uses Standard pricing and remains labeled as assumed where appropriate.
3. **Durable tier snapshots began in July, even though the event existed in May.** [`a668379`](https://github.com/openai/codex/commit/a668379abf0f67d81a61dc971ea463c483846fd2) introduced `thread_settings_applied` with optional `service_tier` on May 18, but explicitly put that event in the never-persist branch. [`0746e8a`](https://github.com/openai/codex/commit/0746e8a34574b4bf4721672c97fc6a94fd8bfad8) moved it to the persisted branch on July 8. The first published package containing that change was `0.144.0-alpha.4`; the first stable was `0.144.0`.
4. **Local rollout usage records still do not persist the API-served tier.** `thread_settings_applied` records the effective client setting; `token_count` records token counters but no tier. OpenAI's API response can report the tier that actually served the request, including a Standard downgrade, but that field is absent from the inspected rollout records.
5. **Missing local attribution is not explained by age.** It occurs in current initial turns and in same-day subagent records alongside explicitly Priority parent records. It must remain unknown unless another authoritative source supplies the served tier. `session_meta.cli_version` is a creation-time field, not a per-turn or per-resume version marker, so it cannot safely date later appended usage to a client capability boundary.

## Historical Fast-price boundary

The embedded catalog records its source capture date as 2026-08-22. The calculator nevertheless begins historical Fast pricing at the exact stable `0.144.0` publication time, 2026-07-09T16:47:12Z, because that is the first stable release capable of persisting the explicit signal needed to justify a Fast premium.

At and after that boundary, an explicit marker uses the captured multiplier appropriate to its model—2× for the GPT-5.6 variants represented in the local corpus and 2.5× for GPT-5.5. Before that boundary, an explicit Fast value remains unpriced. This is an estimator policy tied to observable evidence, not a claim that Priority/Fast did not exist earlier.

## Official timeline and tier semantics

| Date | Primary-source evidence | What it proves |
| --- | --- | --- |
| 2026-03-02 20:29:33 PST | OpenAI Codex commit [`2f5b01a`](https://github.com/openai/codex/commit/2f5b01abd605dfa1304b3b8a12b0033ddf020c75) added a persisted `/fast` toggle and sent `service_tier=priority`. | Implementation landed in source. The feature spec was `UnderDevelopment` and `default_enabled: false`; this commit alone is not a released build. |
| 2026-03-03 05:35:04 UTC | OpenAI published prerelease [`0.108.0-alpha.2`](https://github.com/openai/codex/releases/tag/rust-v0.108.0-alpha.2), target commit `83fa62425a5ea9ea95580ff760e4e65e4fb5f906`. Local tag ancestry contains `2f5b01a`; `0.108.0-alpha.1` does not. | Earliest publicly distributed Codex package containing the opt-in toggle. It required explicit feature configuration or `--enable fast_mode`; it was not exposed through the normal experimental menu. |
| 2026-03-04 11:55:41 PST | OpenAI created source tag [`0.108.0`](https://github.com/openai/codex/releases/tag/rust-v0.108.0), target commit `89b79419a1e0720856d4450cf17379221e5a3b1d`. Its notes call `/fast` under-development. [Public npm metadata](https://registry.npmjs.org/@openai%2fcodex) has no `@openai/codex@0.108.0` publication, nor a `0.109.0` publication. | Public source/tag boundary, not the first installable stable package. |
| 2026-03-05 02:23:39 UTC | OpenAI published stable [`0.110.0`](https://github.com/openai/codex/releases/tag/rust-v0.110.0), target commit `77aabe4218ab7ddaf4b6d471887bda043a4c16e6`. Its notes advertise the persisted `/fast` toggle and app-server tier support. It excludes the later default-on commit. | Earliest distributed stable package containing Fast; still `UnderDevelopment` and default-off. |
| 2026-03-04 20:06:35 PST | OpenAI Codex commit [`394e538`](https://github.com/openai/codex/commit/394e53864013a25dc60cc924c62a58385b0a4fe7) changed Fast from `UnderDevelopment`/off to `Stable`/on. | Implementation became generally visible without explicit feature opt-in, but this commit alone was not yet a release. |
| 2026-03-05 19:12:13 UTC | OpenAI published stable [`0.111.0`](https://github.com/openai/codex/releases/tag/rust-v0.111.0), target commit `8c75cd9afcd405d134530e53c78e5e0e4e5312a3`. Its release notes say Fast is enabled by default. Stable `0.110.0` and prerelease `0.111.0-alpha.1` exclude `394e538`; `0.111.0-alpha.2` and stable `0.111.0` contain it. | Earliest stable public package with Fast available without feature configuration. |
| 2026-04-07 | OpenAI Codex commit [`80ebc80`](https://github.com/openai/codex/commit/80ebc80be5dbe61b300279c0123918275fc145a5) carried model `additional_speed_tiers` through app-server's `model/list`; its commit message says the UI path remained signed-in and feature-gated. | Open-source evidence that graphical clients could discover Fast-capable models. It does not identify a proprietary app release or prove general customer availability. |
| 2026-05-18 | [`a668379`](https://github.com/openai/codex/commit/a668379abf0f67d81a61dc971ea463c483846fd2) introduced `ThreadSettingsApplied` and an optional, omit-when-null `service_tier`, but classified the event as never persisted. First tagged prerelease/stable containing the event: `0.133.0-alpha.1`/`0.133.0`. | Clients could receive the effective settings snapshot, but rollout JSONL could not yet contain it. |
| 2026-07-06 | The official changelog for ChatGPT iOS `1.2026.181` says model, reasoning, and Fast settings were improved so changes remained scoped to the current task ([changelog](https://learn.chatgpt.com/docs/changelog)). An exhaustive check of that changelog found no earlier Fast-control entry. | Earliest explicit official graphical-app customer-control evidence found. It is a documentation lower bound, not a launch date; an earlier proprietary-app rollout remains possible. |
| 2026-07-08 17:58:28 PDT | [`0746e8a`](https://github.com/openai/codex/commit/0746e8a34574b4bf4721672c97fc6a94fd8bfad8) changed rollout policy so `ThreadSettingsApplied` returns `true` from `should_persist_event_msg`. | Exact mainline commit that began durable settings snapshots, including `service_tier` when non-null. |
| 2026-07-09 04:38:13 UTC / 16:47:12 UTC | Public release metadata records `0.144.0-alpha.4` and stable [`0.144.0`](https://github.com/openai/codex/releases/tag/rust-v0.144.0). `0.143.0` and `0.144.0-alpha.1` through `.3` exclude `0746e8a`; `.4` and stable contain it. | First published prerelease and stable package, respectively, that persist the event. |
| 2026-07-09 | This Codex home contains `payload.thread_settings.service_tier = "priority"` in a root rollout. | This account used the historical Priority name before July 30. |
| 2026-07-13 | The Codex changelog says Fast-mode selection and restoration were fixed per task ([changelog](https://learn.chatgpt.com/docs/changelog)). | Fast was already an active Codex feature before the rename. |
| 2026-07-30 | OpenAI says Priority processing was renamed Fast mode; both `priority` and `fast` request values select the same functionality ([Fast-mode guide](https://developers.openai.com/api/docs/guides/fast-mode)). | A name and performance update, not the service's introduction. |
| 2026-08-22 | The current rates were captured into this repository. | Safe effective date for the currently documented cells; it says nothing about earlier availability or prices. |

### Release-boundary interpretation

There are four distinct CLI boundaries:

1. **Implementation:** `2f5b01a`, March 2 at 20:29:33 PST. It is source-code evidence, not evidence of a public binary.
2. **Opt-in public prerelease:** `0.108.0-alpha.2` was the first distributed build containing the feature. Users could select Fast only after explicitly enabling `fast_mode`.
3. **Opt-in stable package:** source tags `0.108.0` and `0.109.0` were not published to npm; `0.110.0` was the first distributed stable package and was still gated/default-off.
4. **Default-on stable package:** `394e538` promoted the flag, and `0.111.0` was the first stable package containing that promotion. This is the first unqualified CLI boundary for ordinary users seeing and selecting Fast without feature configuration.

The local Git history establishes these boundaries by ancestry, not by version-number guesswork. At source tag `0.108.0`, the lifecycle docs define `UnderDevelopment` as “not ready for external use,” expose only `Experimental` flags through `/experimental`, and mark Fast `UnderDevelopment`/false ([stage definition](https://github.com/openai/codex/blob/rust-v0.108.0/codex-rs/core/src/features.rs#L27-L39), [Fast spec](https://github.com/openai/codex/blob/rust-v0.108.0/codex-rs/core/src/features.rs#L708-L713)). This establishes client capability, not the earlier launch date of the underlying API Priority service.

OpenAI also distinguishes requested from served tier. The Responses API accepts `fast` or `priority`, returns `priority` for that mode on GPT-5.6 and earlier, and says the returned value may differ from the request ([Responses API reference](https://developers.openai.com/api/reference/cli/resources/responses/methods/create)). The Fast-mode guide specifically says a ramp-rate downgrade returns `service_tier: "default"` and charges Standard rates, and recommends grouping the Usage Dashboard by service tier or line item ([Fast-mode guide](https://developers.openai.com/api/docs/guides/fast-mode)).

## What the rollout records persist

The inspected `token_count` shape contains:

```text
payload.info.last_token_usage:
  input_tokens
  cached_input_tokens
  cache_write_input_tokens
  output_tokens
  reasoning_output_tokens
  total_tokens

payload.info.total_token_usage:
  the same six counters
```

It has no structural `service_tier` field. In both representative root files, every structural tier value occurs only at `payload.thread_settings.service_tier` under a `thread_settings_applied` event. The local `state_5.sqlite` `threads` table and database schema also contain no tier column; current `config.toml` is only current global state and cannot prove a historical task override.

The analyzer therefore initializes tier attribution as missing, changes it only when a chronological `thread_settings_applied` record appears, and copies that applied value to later normalized usage events ([initial state and event handling](../../src/rollout/analysis.rs#L155-L193), [usage-event construction](../../src/rollout/analysis.rs#L258-L292), [tier mapping](../../src/rollout/analysis.rs#L310-L324)). This is a defensible applied-tier estimate, but it is not the API response's served tier.

### Settings event versus durable rollout record

The source history has two distinct changes:

1. On May 18, `a668379` added the event and snapshot. `service_tier` was optional and omitted when `None` from its first definition ([event and field](https://github.com/openai/codex/blob/a668379abf0f67d81a61dc971ea463c483846fd2/codex-rs/protocol/src/protocol.rs#L1861-L1872)). The same commit placed `ThreadSettingsApplied` in the policy branch whose documented result is `None`, meaning “never persisted” ([May policy](https://github.com/openai/codex/blob/a668379abf0f67d81a61dc971ea463c483846fd2/codex-rs/rollout/src/policy.rs#L133-L219)). Thus `0.133.x` clients could notify a connected UI but could not write the event to JSONL.
2. On July 8, `0746e8a` moved `ThreadSettingsApplied` into the `true` branch ([July policy](https://github.com/openai/codex/blob/0746e8a34574b4bf4721672c97fc6a94fd8bfad8/codex-rs/rollout/src/policy.rs#L79-L101)). That exact mainline commit—not the May schema commit—is the durable-history boundary. `0.144.0-alpha.4` was its first published package and `0.144.0` its first stable package.

An earlier similar edit, `71aee97dfb9bfd6c81e4e48a4ef15726ce329909` on June 23, exists only on the untagged `automation/windows-sandbox-29610-resume-permissions-20260623` branch. No public tag contains it, so it is not a release boundary.

This explains one broad source of missing attribution: a rollout produced before the July persistence build cannot contain an applied-tier snapshot even if the live client knew the tier. It does not explain all gaps after July. A persisted settings event may omit `service_tier` because the field is structurally optional, and usage can occur before a producer emits its first persisted snapshot.

### Why `session_meta.cli_version` cannot date later usage

The recorder writes `cli_version: env!("CARGO_PKG_VERSION")` when it **creates** a rollout ([create path](https://github.com/openai/codex/blob/rust-v0.148.0-alpha.9/codex-rs/rollout/src/recorder.rs#L787-L860)). When it **resumes** a rollout, it opens the existing file for append and sets `meta: None`; it does not append a fresh version marker ([resume path](https://github.com/openai/codex/blob/rust-v0.148.0-alpha.9/codex-rs/rollout/src/recorder.rs#L873-L884)). The loader also documents that only the first `SessionMeta` is canonical and later metadata lines can be copied from fork history ([loader](https://github.com/openai/codex/blob/rust-v0.148.0-alpha.9/codex-rs/rollout/src/recorder.rs#L1008-L1031)).

The mixed local rollout makes the limitation concrete. It has 28 `session_meta` lines (including 1, 401, and 566) interspersed across several days, but all repeat creator version `0.148.0-alpha.9` and original payload timestamp `2026-08-19T20:06:35.242Z`. Those later lines are not version checkpoints. The first metadata line can identify the binary that created that rollout; it cannot attribute an individual later line, especially after the task was resumed by a newer process.

Accordingly, “before any applied-tier snapshot” means only that a usage record occurs earlier in JSONL order than the first durably recorded `thread_settings_applied`. It does not mean “before Fast existed,” “before the UI selection,” or even “before the process knew its current setting.”

## Representative local evidence

Local file references below use paths under `CODEX_HOME`; line numbers refer to the current JSONL snapshots.

### Mixed rollout `01a01ba1-f94b-7041-8e9b-727b5b3e864a`

- The first `token_count` occurs at 2026-08-19 20:09:03Z (line 17), while the first `thread_settings_applied` does not occur until 20:58:47Z (line 398). The initial 49-minute region therefore has usage but no preceding tier snapshot.
- The file later contains 14 `default` snapshots and 9 `priority` snapshots; the first Priority snapshot is on 2026-08-21 (line 1811). Applied-tier switching is directly observed.
- A fresh tree report prices 6,791,345 unattributed tokens as assumed Standard and keeps that assumption visible without making the estimate incomplete.
- The same report prices 5.5M input tokens under explicit Fast model detail and produces a complete $35.06 tree estimate.

File: `$CODEX_HOME/archived_sessions/rollout-2026-08-19T16-06-35-01a01ba1-f94b-7041-8e9b-727b5b3e864a.jsonl`.

### Current rollout `01a02b8e-5dad-7fe1-86c3-e5ce39442af8`

- The rollout begins with `task_started` on line 2 and produces its first `token_count` on line 19.
- Its first `thread_settings_applied` is `default` on line 258, immediately before the next `task_started`, roughly 12 minutes after initial usage began.
- Those initial normalized usage events are priced as assumed Standard: they precede the first persisted settings event in file order, but they do **not** precede Fast-mode availability.

File: `$CODEX_HOME/sessions/2026/08/22/rollout-2026-08-22T18-19-05-01a02b8e-5dad-7fe1-86c3-e5ce39442af8.jsonl`.

### July 9 parent and subagent counterexample

- A root rollout contains `service_tier: "priority"` in its `thread_settings_applied` event on line 161.
- A Guardian subagent created minutes earlier contains `thread_settings_applied` records beginning on line 14 and token usage beginning on line 11, but **no structural `service_tier` field anywhere in the file**.
- Both records were produced by Codex Desktop `0.144.0-alpha.4` on the same day. Missing tier is therefore correlated with record shape/producer path here, not with an era before Fast/Priority existed.

Files:

- `$CODEX_HOME/archived_sessions/rollout-2026-07-09T15-29-40-019f485b-7120-74d1-ad69-87f9f45fd2ae.jsonl`
- `$CODEX_HOME/sessions/2026/07/09/rollout-2026-07-09T15-25-18-019f4857-716d-7063-a007-47b3920ca1f6.jsonl`

## Observed, inferred, and unknown

### Observed

- The applied settings chronology in each rollout: `default`, `priority`, or an omitted field.
- Token usage and timestamps.
- Rollout policy did not persist `thread_settings_applied` before `0.144.0-alpha.4`.
- Initial-turn usage can precede the first settings snapshot.
- Some subagent settings snapshots omit the tier even while related root activity records Priority.
- `session_meta.cli_version` is written on rollout creation, not each resume.
- Priority/Fast could be downgraded to Standard according to OpenAI's API contract.

### Inferred by the meter

- A settings snapshot applies to subsequent usage until another full snapshot appears.
- `default`/`standard` maps to Standard; `priority`/`fast` maps to Fast.
- Missing values use the explicit Standard fallback and are labeled assumed unless pre-release evidence makes Standard definite; unsupported explicit values remain unpriced rather than inheriting an earlier value.

### Unknown from local records

- The tier that actually served each request.
- Whether an initial gap inherited the UI's newly selected task tier, a previous task setting, or a global default.
- Why every producer path does not emit a tier field before the initial turn.
- Which client version produced an individual line appended after a resume.
- Whether the captured per-model Fast premiums differed historically; the estimator deliberately applies them only to durable explicit markers from stable `0.144.0` onward.
- The authoritative launch date of the underlying API Priority processing service. The Codex client boundary is established above, but the service necessarily existed no later than the March 2 client implementation and may have existed earlier.

## Recommendation

For a useful best-effort estimator, price missing tier metadata at Standard rates while preserving the distinction between evidence and assumption. Mark usage as definitive Standard only when its timestamp predates `0.108.0-alpha.2` and its creator version does not prove Fast capability; otherwise label it assumed Standard. This is a pricing fallback, not proof of the API-served tier. Explicit Standard, Priority/Fast, and unsupported markers remain authoritative for applied-tier classification.

For display, annotate the aggregate row when a model has one service mode and show child rows only for mixed modes. Combine explicit and assumed Standard usage as `Standard*`, explain the assumption once, render Fast without a glyph, and retain unavailable labels for unsupported explicit tiers. Keep the exact Standard split in structured output. For billing-grade backfill, use an authoritative API response log or the OpenAI Usage Dashboard grouped by service tier and line item; the local rollout corpus cannot prove served-tier history on its own.

Reproduction commands used for the representative counts:

```text
target/debug/codex-cost-meter report 01a01ba1-f94b-7041-8e9b-727b5b3e864a --json
target/debug/codex-cost-meter report 01a02b8e-5dad-7fe1-86c3-e5ce39442af8 --json
```
