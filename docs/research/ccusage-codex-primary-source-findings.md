# ccusage and codex-cost-meter comparative study

## Scope

This study compares the Codex-specific behavior of `ccusage` with `codex-cost-meter`, concentrating on rollout discovery, replay and duplicate handling, usage normalization, and estimated API-list-price cost. It uses the supplied [`codex-usage.json`](../../codex-usage.json), generated with:

```text
npx ccusage@latest codex daily --since 2026-07-18 --until 2026-08-17 --timezone America/Chicago --json
```

Research was pinned to npm package **`ccusage` 20.0.20**, the version reported by the [official npm package page](https://www.npmjs.com/package/ccusage) on 2026-08-21. The corresponding official tag is commit [`bd7f89b469aee5635fb2e6722dd6d70f2d113ac1`](https://github.com/ccusage/ccusage/commit/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1), whose published package manifest also says `20.0.20` ([source](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/apps/ccusage/package.json#L1-L4)). All repository links below are immutable permalinks to that revision.

The local comparison is pinned to `codex-cost-meter` **0.8.0**, commit [`b90f05a4075f81eebdc1ea284eaed79e125a3021`](https://github.com/deinspanjer/codex-cost-meter/commit/b90f05a4075f81eebdc1ea284eaed79e125a3021). No benchmark or billing claim is made: both tools estimate API list price from local usage records, not the user's ChatGPT invoice.

During the study, upstream `main` advanced by one commit to **0.8.1** ([`6356d16`](https://github.com/deinspanjer/codex-cost-meter/commit/6356d16881e0a036d909a8a71c3916a17e40ce1e)). That release uses SQLite rollout paths and spawn edges to avoid opening unrelated files for exact reports and bounded updates, while retaining reconciliation and full-scan fallback ([amended ADR](https://github.com/deinspanjer/codex-cost-meter/blob/6356d16881e0a036d909a8a71c3916a17e40ce1e/docs/adr/0001-read-task-metadata-from-sqlite-and-usage-from-rollouts.md#L17-L45)). It does not change usage normalization, replay handling, or pricing, so the substantive comparison remains current. The working checkout was not pulled during this study.

## Executive conclusion

**Yes, ccusage has some materially better methods, but its base token math and historical pricing are not among them.**

1. **Replay suppression is a promising method that needs direct validation before adoption.** ccusage removes leading child `token_count` records whose normalized usage tuples match the parent's pre-fork usage sequence. Those are presumed duplicate history records, not the input or cached-input tokens of a new child request. `codex-cost-meter` has no cross-rollout comparison, so it would count such records twice if Codex actually serializes them under attributable child context; this study has not yet isolated that condition in the local corpus.
2. **Long-context handling exposes a real local pricing omission; Standard/Fast handling is a separate reporting-policy choice.** OpenAI publishes higher rates for a whole request above the model's input threshold, but `codex-cost-meter` has only one rate set per model and effective date. ccusage preserves per-request usage until that classification. It also distinguishes Standard from Fast/Priority activity, which matters only if the report intends to estimate the recorded service tier rather than a standard-price equivalent.
3. **Daily/monthly, timezone-aware date windows, simultaneous multiple-home discovery, and parsing of user-saved headless output are genuinely unique ccusage features.** ccusage does not create those headless logs; it can consume JSONL previously saved from `codex exec --json`. Of these, date views are the obvious product gap for this project.
4. **`codex-cost-meter` is materially better for historical and incomplete estimates.** It selects prices effective at each event timestamp and returns a known lower bound plus explicit unpriced/incomplete metadata. ccusage applies the catalog loaded at report time to every date, uses model fallbacks, silently assigns zero cost when pricing is absent in JSON, and omits pricing provenance from focused JSON.
5. **The ordinary usage formula is equivalent.** Both separate cached from non-cached input, charge output once even though reasoning is a reported subset, and derive request deltas from cumulative counters. There is no better core arithmetic to port from ccusage.

The minimum useful follow-up is therefore: correct the published long-context pricing gap; add date-bucketed reporting; and validate exact replay duplication with an isolated fixture or counted corpus evidence before changing accounting. Decide separately whether the product reports standard-price equivalents or recorded Fast/Priority rates. Do not copy ccusage's heuristic replay deletion, session-agnostic event signature, fuzzy price substitution, or silent zero-cost behavior unchanged.

## Comparison at a glance

| Area | ccusage 20.0.20 | codex-cost-meter 0.8.0 | Assessment |
| --- | --- | --- | --- |
| Public report scope | Daily, monthly, and flat session reports with inclusive dates and timezone grouping | Exact root plus descendants, rollout types, and lifetime Desktop Project reports | Different strengths; daily/monthly are unique to ccusage, descendant accounting is unique locally |
| Storage roots | Multiple `CODEX_HOME` values and direct headless-log directories | One selected Codex home | ccusage is broader |
| Active/archive discovery | Both trees | Both trees | Equivalent |
| Duplicate files | Active-relative-path preference plus later event dedupe | Newest file per rollout ID by modification time | Neither dominates every shape |
| Fork/subagent replay | Exact parent-prefix suppression plus a fallback burst heuristic | No cross-rollout replay filter | Candidate improvement, but local overcount is not yet proven |
| Within-file usage deltas | Cumulative delta with last-usage fallback | Cumulative delta with last-usage fallback | Equivalent core method |
| Corrupt/incomplete input | Skips unreadable or malformed data; focused JSON has no warning array | Bounded line reader, warnings, incomplete flag, unattributed tokens, unpriced models | Local method is materially safer |
| Input/reasoning semantics | JSON exposes non-cached input separately; reasoning is an output subset | JSON exposes gross input with cache-read/write subsets; reasoning is an output subset | Different schema, equivalent cost basis |
| Cache-write input | Focused Codex output always reports zero | Parsed, reported, and priced when present | Local method is more complete |
| Price freshness | Embedded plus live LiteLLM/models.dev and user overrides | Embedded, source-cited catalog | ccusage is fresher and broader; local is deterministic |
| Historical prices | One run-time price per resolved model for all usage dates | Effective-dated price history selected per event | Local method is materially better |
| Long-context and speed tiers | Per-request long-context and per-event Standard/Fast buckets | Not represented in the catalog/formula | Long-context is a real local pricing gap; speed-tier relevance depends on the report's stated basis |
| Unknown prices | Fallback model or zero cost; human warning only | Known lower-bound cost, `null` complete estimate, explicit warnings | Local method is materially safer |
| Pricing provenance in JSON | None in focused JSON | Basis, as-of date, source, and explicit proxies | Local output is materially better |

## ccusage primary-source detail

These are the ccusage features most likely to matter in a comparison:

- Multiple comma-separated `CODEX_HOME` roots, independent discovery of active and archived sessions, and direct-directory parsing of JSONL previously saved from `codex exec --json` ([paths](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/paths.rs#L20-L52), [environment parsing](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/paths.rs#L104-L116)). This is an input compatibility feature; ccusage does not create or retain headless logs itself.
- Three separate duplicate defenses: same relative active/archive file, replayed fork/subagent prefixes, and identical cross-file usage-event signatures (details below).
- Streaming, parallel JSONL processing rather than whole-file string loading; the parser uses a 128 KiB buffered reader and line prefilters ([parser](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L156-L176), [prefilter](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L470-L507)).
- Per-event Standard/Fast attribution from chronological `thread_settings_applied` events, with `config.toml` fallback only for unclassified usage and explicit `--speed` overrides ([tier parsing](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L302-L315), [speed policy](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/speed.rs#L23-L60)).
- Per-request long-context classification before aggregation, preserving the information required to price all tokens in a request at its model-specific long-context rate ([aggregation](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/aggregate.rs#L342-L377), [cost formula](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/report.rs#L187-L219)). The supplied report reconciliation below shows that this feature did not change that run's cost.
- Embedded and live pricing sources, model alias/fuzzy lookup, `codex-auto-review` date-based resolution, user pricing overrides, and model-specific Fast multipliers.
- User-facing daily/monthly/session views, JSON output, `--last`, inclusive date filters, timezone grouping, `--offline`, `--no-cost`, compact tables, and layered config ([focused help](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-cli-parser/src/snapshots/ccusage_cli_parser__tests__codex_daily_help.snap#L5-L25), [config priority](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/docs/guide/configuration.md#L15-L24)).

## Command inventory: Codex-focused versus generic ccusage

The Codex namespace supports exactly `codex daily`, `codex monthly`, and `codex session`; it uses the standard focused-report set, which excludes weekly ([report set](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-cli/src/types.rs#L136-L148), [Codex parser](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-cli-parser/src/parser.rs#L583-L608)). `--speed auto|standard|fast` is the only Codex-specific report option; JSON, date/timezone, offline pricing, no-cost, compact output, config, and `--last` are shared report options.

By contrast, bare `ccusage daily|weekly|monthly|session` are generic multi-provider reports. Their `--sections` and `--by-agent` options can load Codex alongside every detected source, but those are not capabilities of `ccusage codex daily` itself ([unified reports](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/docs/guide/all-reports.md#L1-L45)). Root-level `ccusage blocks` and `ccusage statusline` are Claude Code features, not Codex views ([blocks](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/docs/guide/blocks-reports.md#L1-L8), [statusline](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/docs/guide/statusline.md#L1-L13)).

## Rollout discovery and duplicate handling

### Verified discovery behavior

For each `CODEX_HOME` entry, ccusage checks `sessions/` first and then `archived_sessions/`. It uses the root directly only when neither exists. It recursively collects regular files whose extension is exactly `jsonl`; it does not follow directory symlinks because it recurses only when `DirEntry::file_type().is_dir()` ([discovery](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/paths.rs#L20-L52), [recursive collector](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/common/src/lib.rs#L14-L33)).

The same relative file path is retained only once within one home’s dedupe scope. Because active sessions are scanned first, this shape keeps the active file and drops the archive copy:

```text
$CODEX_HOME/sessions/2026/08/rollout-a.jsonl
$CODEX_HOME/archived_sessions/2026/08/rollout-a.jsonl
```

The key is `(Codex home root, relative JSONL path)`, so the same relative path in two different configured homes remains two inputs ([file key](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/paths.rs#L81-L102)).

`codex-cost-meter` also honors Codex home selection: an explicit `--codex-home` wins, followed by the `CODEX_HOME` environment variable, then the platform user's `.codex` directory. It treats the selected value as one path and scans that home's `sessions/` and `archived_sessions/`; it does not split or aggregate multiple homes in one report ([local resolution](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/cli.rs#L360-L367), [local discovery](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/rollout/discovery.rs#L69-L89)).

### Verified fork/subagent replay suppression

The first JSONL line is inspected for `session_meta`. A child is associated with a parent through either `payload.forked_from_id` or `payload.source.subagent.thread_spawn.parent_thread_id` ([metadata](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/replay.rs#L224-L260)). Representative shape:

```jsonl
{"timestamp":"2026-08-01T12:00:00Z","type":"session_meta","payload":{"id":"parent"}}
{"timestamp":"2026-08-01T12:00:01Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":100,"cached_input_tokens":80,"output_tokens":10,"total_tokens":110}}}}
```

```jsonl
{"timestamp":"2026-08-01T12:05:00Z","type":"session_meta","payload":{"id":"child","forked_from_id":"parent"}}
{"timestamp":"2026-08-01T12:05:00.010Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":100,"cached_input_tokens":80,"output_tokens":10,"total_tokens":110}}}}
```

The planner reads the parent’s usage sequence only through the child’s fork time, then the child parser discards a matching leading usage prefix. Matching compares token-count tuples, so rewritten child timestamps do not prevent suppression ([plan](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/replay.rs#L31-L110), [prefix filter](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L177-L218)). If the parent is unavailable or the first usage tuple does not match, ccusage falls back to a heuristic: two leading usage events at most one second apart establish a rewritten burst, and consecutive events remain suppressed until a gap exceeds one second ([heuristic](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L84-L149), [application](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L197-L217)).

The suppressed prefix is a sequence of already-recorded usage counters copied into the child file, not merely inherited conversation text. A new child request still produces a later usage event, including its own uncached and cached input, and that event must count. Exact prefix suppression is correct only if the matching leading events represent serialization replay without new API requests. ccusage infers that from position, parent relationship, fork time, and equal token tuples; the fallback burst heuristic makes a weaker inference.

### Verified cross-file exact-signature dedupe; inferred risk

After replay filtering, daily/monthly/weekly aggregation drops duplicate events with the same parsed timestamp, resolved model, input, cached input, output, reasoning output, and total token counts. Session identity is deliberately omitted outside the session report ([dedupe key](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/aggregate.rs#L488-L553)). If duplicate copies disagree about service tier, explicit metadata is retained and Standard wins a Standard/Fast conflict ([tier merge](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/types.rs#L20-L35)).

**Inference:** this is useful for copied/replayed records that escape the earlier defenses, but it can undercount two legitimate requests from different sessions if all signature fields happen to be identical at millisecond precision. The source has no provenance field in the non-session key with which to distinguish that collision. The risk is probably low for ordinary requests but should be tested against real parallel/subagent workloads rather than assumed away.

### Parsing and failure behavior

- Session rollouts consume `turn_context`, `thread_settings_applied`, and `event_msg/token_count`. Saved headless logs accept usage/model/timestamp fields at the top level or under `data`, `result`, or `response`; missing headless timestamps fall back to file modification time, and headless events have no recorded service tier ([session parser](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L276-L367), [headless parser](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L370-L456), [mtime fallback](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L1050-L1061)).
- Usage field aliases include `input_tokens`/`prompt_tokens`/`input`, three cache-read spellings, three output spellings, and two reasoning spellings. When total is absent or zero, it derives `input + output`; reasoning is not added because it is a subset of output ([normalization](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/types.rs#L234-L297)).
- Unopenable files, line-read failures, and malformed candidate lines are skipped rather than failing the report ([stream behavior](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L156-L166), [read/parse handling](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L223-L247)). This favors report completion but can silently omit damaged or unreadable data.

## Usage aggregation and JSON meaning

For a token event, ccusage prefers `last_token_usage` only when the cumulative total advanced. Otherwise it derives a per-event delta from cumulative totals with saturating subtraction. Repeated cumulative totals and all-zero deltas are ignored; cached input is clamped to total input ([delta logic](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L320-L367), [subtraction](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L1063-L1083)).

`codex daily --json` emits this schema ([report construction](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/report.rs#L16-L99), [totals](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/report.rs#L101-L129)):

```json
{
  "daily": [{
    "date": "YYYY-MM-DD",
    "inputTokens": 0,
    "cacheCreationTokens": 0,
    "cacheReadTokens": 0,
    "outputTokens": 0,
    "reasoningOutputTokens": 0,
    "totalTokens": 0,
    "costUSD": 0.0,
    "models": {
      "model-id": {
        "inputTokens": 0,
        "cacheCreationTokens": 0,
        "cacheReadTokens": 0,
        "outputTokens": 0,
        "reasoningOutputTokens": 0,
        "totalTokens": 0,
        "isFallback": false
      }
    }
  }],
  "totals": {
    "inputTokens": 0,
    "cacheCreationTokens": 0,
    "cacheReadTokens": 0,
    "outputTokens": 0,
    "reasoningOutputTokens": 0,
    "totalTokens": 0,
    "costUSD": 0.0
  }
}
```

Important semantics:

- Raw Codex `input_tokens` includes cached input. JSON `inputTokens` means **non-cached input** (`input - cached`, saturating), while `cacheReadTokens` carries the cached portion. `cacheCreationTokens` is always zero ([report](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/report.rs#L51-L99)).
- `reasoningOutputTokens` is informational and already included in `outputTokens`; it is not billed or added again to derived totals ([normalization](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/types.rs#L286-L297)).
- Per-model JSON contains token counts and `isFallback`, but not a per-model cost. Daily and overall `costUSD` are computed from hidden per-model buckets.
- Missing pricing produces `0.0` for that model’s cost ([cost lookup](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/report.rs#L131-L151)); JSON has no field distinguishing “free” from “unpriced.”
- Dates with no retained usage event are omitted; ccusage does not synthesize zero-usage calendar rows. With no groups, focused JSON is `{ "daily": [], "totals": { ...zero fields... } }`, while the table path prints `No Codex usage data found.` and returns ([JSON construction](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/report.rs#L16-L30), [table behavior](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/report.rs#L263-L273)).
- Focused daily JSON carries no source file, session id, Codex home, pricing revision, price-match key, missing-price flag, or warning array. Session JSON adds path-derived `sessionFile`, `directory`, and `lastActivity`, but still no pricing provenance ([row schema](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/report.rs#L51-L99)). Missing-price warnings are emitted only after the human table, not by the JSON branch ([table warning](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/report.rs#L350-L371), [JSON branch](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/lib.rs#L35-L48)). `isFallback` is model-attribution provenance only.

## Pricing behavior

At build time, v20.0.20 embeds a compact LiteLLM snapshot pinned to revision `1a183efaa1a2108aed7e1bed8d445d93bd1aa60d` and a models.dev snapshot pinned to `bff41227803631c84903fcf7f486370e9fbcde86` ([lock file](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/flake.lock#L159-L183)). At runtime:

1. Start with embedded LiteLLM, built-ins, and embedded models.dev fallbacks.
2. Unless `--offline`, refresh from LiteLLM’s mutable `main` pricing JSON; failure or a zero-entry parse leaves embedded prices in place.
3. Reapply long-context rates from the embedded models.dev snapshot.
4. Apply user `pricingOverrides` last.
5. If the primary map misses a model, online mode may query live models.dev, then falls back to the embedded models.dev map ([load order](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-core/src/pricing.rs#L630-L680), [lookup order](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-core/src/pricing.rs#L970-L1025), [network endpoints](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-core/src/pricing.rs#L51-L54)).

Lookup tries exact entries, built-in/configured aliases, and then boundary-aware fuzzy matching; exact-only tier variants are guarded from being shadowed by base-model fuzzy matches ([lookup](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-core/src/pricing.rs#L970-L1103)). `codex-auto-review` is resolved by log date through a pinned fallback list; absent model metadata otherwise falls back to `gpt-5` and marks `isFallback` ([model fallback](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L583-L627)).

The base formula is:

```text
(input - cached_input) * input_rate
+ cached_input * (published_cache_read_rate or input_rate)
+ output * output_rate
```

Each request above the model-specific threshold is put wholly into the long-context bucket before daily aggregation; for the pinned GPT-5.4/5.5/5.6 entries that threshold is 272,000 input tokens and the snapshot carries separate input/cache/output rates ([example snapshot rates](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-core/src/models-dev-pricing.json#L11509-L11529), [GPT-5.5](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-core/src/models-dev-pricing.json#L11615-L11635), [GPT-5.6 Sol](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-core/src/models-dev-pricing.json#L11738-L11760)).

This is not merely a ccusage convention. OpenAI's current model pages state that requests with more than 272,000 input tokens are charged at 2× the input/cache rate and 1.5× the output rate for the entire request ([GPT-5.6 Terra](https://developers.openai.com/api/docs/models/gpt-5.6-terra), [GPT-5.6 Sol](https://developers.openai.com/api/docs/models/gpt-5.6-sol), [GPT-5.6 Luna](https://developers.openai.com/api/docs/models/gpt-5.6-luna)).

Fast cost is standard cost plus the Fast-designated bucket’s standard cost times `(model_multiplier - 1)`. The pinned overrides include 2.0× for GPT-5.6 Sol/Terra/Luna and GPT-5.4/GPT-5.3 Codex, and 2.5× for GPT-5.5; models without a multiplier stay at 1.0× ([formula](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/report.rs#L131-L152), [multipliers](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-core/src/fast-multiplier-overrides.json#L1-L14)). OpenAI's API documentation confirms that `fast`/`priority` is a per-request service tier with a per-token premium over Standard processing ([Fast mode](https://developers.openai.com/api/docs/guides/fast-mode)).

The internal `recorded_standard_usage` and `recorded_fast_usage` fields are token shards used to apply multipliers to the correct subset after aggregation; long-context token shards serve the analogous two-stage-rate purpose ([model buckets](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/types.rs#L52-L80)). They are neither logged costs nor JSON output. Unlike adapters that can display provider-recorded costs, the Codex run path always loads pricing and calculates `costUSD` from normalized tokens ([run path](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/lib.rs#L35-L48)).

## Date/time behavior and source-backed cautions

- Event timestamps are converted to the requested IANA timezone before grouping and before the inclusive `--since`/`--until` comparison ([grouping/filter](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/aggregate.rs#L318-L340), [timezone conversion](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-core/src/date_utils.rs#L293-L310)). Thus the supplied `America/Chicago` command should bucket boundary events by Chicago calendar date.
- If no timezone is given, the system timezone is used. In v20.0.20 an invalid timezone also becomes `None` and silently falls back to the system timezone; this is a robustness weakness, not an inferred behavior ([parse/fallback](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-core/src/date_utils.rs#L293-L306), [aggregator fallback](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/aggregate.rs#L433-L444)).
- v20.0.20’s CLI merely removes hyphens from date bounds rather than validating real calendar dates ([normalizer](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage-cli/src/types.rs#L69-L71)). The supplied bounds are valid, so this does not explain differences for that run.
- `--last N` is resolved in the selected timezone to a `--since` bound for the report’s calendar unit ([implementation](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/crates/ccusage/src/cli/last_window.rs#L6-L23)).

## Supplied-report pricing reconciliation

The supplied `codex-usage.json` totals can be reproduced exactly from its per-model non-cached input, cache-read, and output counts using v20.0.20's pinned standard rates for GPT-5.5, GPT-5.6 Sol, Terra, and Luna: **$3,774.43602864**, versus the file's **$3,774.436028639999**. The difference is floating-point noise.

Both the long-context rates and every relevant Fast multiplier are strictly greater than their standard counterparts. Therefore neither per-request long-context pricing nor Fast pricing changed this run's reported cost. They remain real ccusage capabilities for other rollouts, not explanations for this file's result. This reconciliation also confirms that reasoning tokens were not charged separately.

The 31-day inclusive request produced 27 rows because zero-usage dates are omitted. The totals are 204,933,696 non-cached input tokens, 5,547,254,784 cache-read tokens, 17,985,619 output tokens, and 6,907,426 reasoning-output tokens. Cache reads are 96.44% of gross input, so even small cache-price differences materially affect the estimate.

The reconciliation also proves that ccusage used the post-2026-07-30 Terra and Luna rates for usage before that date. It has no event-date argument in its price lookup. Applying `codex-cost-meter`'s effective-dated rates to the supplied **daily buckets** instead gives **$3,816.92737606**, $42.49134742 or 1.126% above ccusage: $38.83063670 from Terra and $3.66071072 from Luna. This is a close comparison, not an exact local reprice, because the JSON retains Chicago calendar days rather than individual UTC event timestamps around the July 30 boundary. The directional conclusion is exact: ccusage repriced historical usage at the current catalog rates, while the local catalog selects the rate effective at each event ([local history](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/data/model-prices.json#L19-L27), [local lookup](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/pricing.rs#L135-L178)).

The v20.0.20 Codex guide is internally inconsistent: one paragraph says sessions without model metadata are skipped, another says they are retained with the `gpt-5` fallback, and troubleshooting again says they are ignored ([guide](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/docs/guide/codex/index.md#L46-L54), [troubleshooting](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/docs/guide/codex/index.md#L98-L106)). The executable source is unambiguous: it assigns `gpt-5` and sets `isFallback` ([source](https://github.com/ccusage/ccusage/blob/bd7f89b469aee5635fb2e6722dd6d70f2d113ac1/rust/adapters/codex/src/parser.rs#L583-L611)). Treat the source behavior as authoritative for this comparison.

## Comparison with codex-cost-meter's rollout processing

### Discovery and tree construction

Both tools recursively scan `sessions/` and `archived_sessions/` without following directory symlinks. Their duplicate units differ:

- ccusage prefers the active copy of one relative path, then uses replay and event-signature defenses later.
- `codex-cost-meter` reads metadata until it identifies a rollout, selects the newest file for each rollout ID by modification time, and builds a cycle-safe parent/descendant graph ([discovery](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/rollout/discovery.rs#L69-L145), [ID dedupe](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/rollout/discovery.rs#L341-L428)). Its reports then aggregate the selected root and every linked descendant by rollout type ([tree report](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/report.rs#L531-L632)).

The local ID-based choice is better for an active/archive copy whose relative path changes; ccusage's later defenses are better when copied history exists under distinct child IDs. ccusage additionally understands `forked_from_id`, which the local parent extractor does not currently read.

Version 0.8.1's targeted index improves exact-report I/O by using SQLite only to narrow which rollout files are opened; it deliberately falls back and still derives final identity and descendants from JSONL. It is a performance/discovery optimization, not a replay or accounting change.

### Delta calculation and replay

The within-file algorithms are substantively equivalent. `codex-cost-meter` tracks cumulative totals, emits a delta when counters advance, uses `last_token_usage` after a reset or when totals are absent, and drops zero deltas ([normalization](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/rollout/analysis.rs#L52-L116), [event handling](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/rollout/analysis.rs#L225-L284)). It also refuses to guess model attribution for ambiguous embedded legacy history, retaining an incomplete/unattributed signal instead ([attribution](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/rollout/analysis.rs#L119-L178), [incomplete path](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/rollout/analysis.rs#L262-L284)).

What it lacks is cross-rollout replay recognition. If a forked child physically repeats the parent's historical `token_count` records under otherwise valid modern turn context, normalizing the parent and child separately retains both copies; the whole-tree sum then counts accounting records for the same requests twice. This is the precise condition under which the local result is wrong. Inherited context used by a genuinely new child request is different and must remain billable, including cache reads. ccusage's exact token-tuple prefix comparison is designed to distinguish the leading replay record sequence, but this study has not yet proved how often that condition occurs locally. Its fallback burst heuristic and global event key go farther, with more undercount risk.

### Read-only corpus probe

A read-only diagnostic mirrored the local v0.8.0 discovery, attribution, and cumulative-delta rules over the same Codex home and Chicago date interval, then converted gross input to ccusage's non-cached-input convention. It found 4,485 unique rollout IDs and no malformed JSON lines. Before any ccusage-style replay suppression, locally attributable usage was:

| Measure | Local-rule probe | Supplied ccusage JSON | Difference |
| --- | ---: | ---: | ---: |
| Non-cached input | 261,966,495 | 204,933,696 | +27.83% |
| Cache read | 8,026,171,008 | 5,547,254,784 | +44.69% |
| Output | 24,565,056 | 17,985,619 | +36.58% |
| Reasoning output | 9,360,130 | 6,907,426 | +35.51% |

This is not a CLI-to-CLI benchmark because `codex-cost-meter` has no date-window command, and the probe did not independently label how much ccusage removed through exact parent matching versus its fallback heuristics or other parser-policy differences. It establishes a material aggregate difference, not its cause. The earlier conclusion that replay was the leading cause was too strong. A production change should first identify real parent/child files with duplicated historical usage records, then measure exact-prefix removals separately from heuristic removals.

### Failure and completeness policy

`codex-cost-meter` caps a JSONL record at 16 MiB, refuses directory symlinks, and propagates unreadable files, malformed/oversized records, invalid usage, ambiguous attribution, and unknown prices into warnings or an incomplete result. Unknown rates preserve a known-cost lower bound while making the complete estimate `null` ([bounded reader](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/rollout/discovery.rs#L281-L338), [report completeness](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/report.rs#L379-L475)).

ccusage is more permissive about input shapes and more likely to finish a corpus report, but focused JSON does not disclose skipped files/lines, heuristic replay removal, event-key collisions, price catalog revision, or missing model prices. For auditability, the local policy is better.

## Comparison with codex-cost-meter's pricing

### What is equivalent

The normal formula is the same: non-cached input at input price, cache read at cached-input price, cache write when supported, and output at output price. Reasoning is a subset of output and is never charged twice. `codex-cost-meter` stores gross input and subtracts cache-read and cache-write tokens during costing ([formula](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/pricing.rs#L135-L178)); ccusage exposes non-cached input directly in focused JSON. The schema differs, not the arithmetic.

### Where ccusage is better

- Live and embedded catalogs cover more model names without waiting for a release.
- `pricingOverrides` allow correction without rebuilding.
- Exact/built-in alias handling improves coverage.
- Per-request long-context buckets preserve a price boundary that a flat aggregate would lose.
- Per-event Standard/Fast attribution can price mixed-tier activity correctly.

Only the first three are relevant to the supplied run, and their benefit is freshness rather than a better formula. The flat reconciliation proves long-context and Fast did not affect its result.

### Where codex-cost-meter is better

- Its catalog contains explicit effective dates and prices each event at the rate in force at that time.
- The embedded catalog and source/as-of metadata make a report reproducible and offline by default.
- Only explicit proxies are used; fuzzy matches cannot silently turn one model into another.
- Missing component or model prices yield a partial lower bound and an incomplete result, not a complete-looking zero.
- It already parses, reports, and prices `cache_write_input_tokens`; ccusage's focused Codex `cacheCreationTokens` is always zero.

The local aggregate retains per-event usage and timestamp through costing, so long-context tiers can be added without redesigning the parser ([event pricing](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/report.rs#L414-L439)). Standard/Fast requires parsing the tier timeline as ccusage does.

## Unique ccusage features worth distinguishing from correctness

These are genuinely absent from the current local CLI ([local report arguments](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/cli.rs#L124-L145)):

- daily and monthly aggregation;
- inclusive `--since`, `--until`, and `--last` windows;
- IANA timezone grouping;
- multiple comma-separated Codex homes;
- parsing of user-saved `codex exec --json` output from a direct directory (ccusage creates no log files);
- compact/no-cost views;
- offline/live pricing selection and pricing overrides;
- Standard/Fast override and automatic attribution; and
- long-context pricing.

They should not obscure what `codex-cost-meter` uniquely provides: exact root-plus-descendant task accounting, rollout-type breakdowns, turns, durations, incomplete-turn counts, lifetime Desktop Project resolution, pricing provenance, safe partial cost, and optional bounded title updates/scheduling ([report schema](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/report.rs#L33-L120), [human report](https://github.com/deinspanjer/codex-cost-meter/blob/b90f05a4075f81eebdc1ea284eaed79e125a3021/src/output.rs#L10-L88)). ccusage's generic weekly, blocks, and statusline commands are not Codex-focused features.

## Recommendations

1. **Validate replay duplication before changing accounting.** Identify a real or minimal Codex parent/child pair that physically repeats historical `token_count` records and prove that no request occurred for the repeated prefix. If confirmed, recognize `forked_from_id` as well as subagent parent metadata and remove only the exact pre-fork parent prefix; preserve every subsequent child request's input and cache usage.
2. **Do not adopt heuristic deletion or session-agnostic dedupe without corpus evidence.** A one-second leading burst and an identical millisecond event signature can both describe legitimate parallel work. If either is ever added, count and report removals in JSON.
3. **Correct long-context pricing.** The current one-rate formula underprices published >272K-input requests. Preserve per-request classification, effective dates, and incomplete-rate semantics. Treat Standard/Fast separately: either keep the current standard-price basis explicit or price recorded service tiers, but do not mix the meanings.
4. **Add date-bucketed corpus reporting.** Default calendar boundaries to the OS timezone. An explicit IANA-timezone option can wait until portability, travel, or reproducible cross-machine reporting supplies a concrete requirement.
5. **Improve price freshness without sacrificing reproducibility.** Prefer a release-time catalog refresh and an explicit local override file over fetching mutable pricing by default. Keep exact aliases and surface every proxy/missing rate.

The ordinary token formula remains correct; the required pricing change is selecting the applicable per-request rate set before applying it.
