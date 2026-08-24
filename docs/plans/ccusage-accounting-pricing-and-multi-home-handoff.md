# ccusage accounting, pricing, and multi-home handoff

## Outcome

Close the confirmed long-context and Fast/Priority pricing gaps, add multi-home reporting, and resolve replay accounting without trading a possible overcount for an opaque undercount. Preserve the existing historical-price, incomplete-input, and lower-bound guarantees.

Read [`../research/ccusage-codex-primary-source-findings.md`](../research/ccusage-codex-primary-source-findings.md) before implementation. Recheck current official OpenAI pricing on the implementation day because long-context and Fast pricing changed rapidly after launch.

## Known facts

### Replay accounting

- Codex may copy conversation history into a fork or subagent. A new child request that consumes that history is billable, including cached input.
- ccusage separately assumes that some child files begin with historical `token_count` records copied from the parent without a new request. It compares the child's leading normalized usage tuples with the parent's pre-fork sequence and suppresses matches.
- ccusage also has a one-second burst heuristic and a cross-session event-signature dedupe. Both can discard legitimate parallel requests.
- `codex-cost-meter` selects one file per rollout ID and normalizes cumulative counters within each file. It does not compare usage across parent and child rollout IDs.
- Codex source and a 2026-08-24 read-only corpus audit confirm that legacy explicit forks physically replay parent usage records without new API requests. Of 467 explicit forks, 381 had deterministic copied prefixes; the current parser attributes about 8.75 billion known duplicate tokens and marks another 842 million replayed tokens unattributed/incomplete.
- The child's first genuine request separately submits inherited model-visible history and remains billable according to its server-reported cached and uncached input. The audited post-prefix requests total about 31.7 million tokens and must remain counted.

### Pricing

- The ordinary formula is sound: uncached input, cached input, cache-write input, and output use separate rates; reasoning is already included in output.
- The catalog currently selects one effective-dated rate set per model. It cannot express a request-size threshold or a processing-service tier.
- As of 2026-08-21, the official GPT-5.6 model pages say requests above 272,000 input tokens use 2x input and 1.5x output pricing for the full request. The current source of truth is <https://developers.openai.com/api/docs/models/gpt-5.6-sol>.
- OpenAI documents `fast` and `priority` as the same premium API processing mode and records the served tier in API responses. The current source of truth is <https://developers.openai.com/api/docs/guides/fast-mode>.
- Codex rollouts contain chronological `thread_settings_applied.service_tier` values, but implementation must verify whether another event records the actually served tier. Requested Fast can be served and charged as Standard under documented downgrade behavior.

### Codex homes

- Report, update, and schedule commands currently resolve one home from an explicit `--codex-home`, then `CODEX_HOME`, then the platform user's `.codex` directory.
- ccusage can aggregate comma-separated homes in one report.
- Reporting is read-only; update and scheduling paths mutate or coordinate one Codex store and should remain single-home.

## Decisions

1. Keep `+` exclusively for its current meaning: the displayed value is a known lower bound because input or pricing is incomplete.
2. Exclude only the deterministic explicit-legacy-fork prefix: require `forked_from_id`, copied legacy history, an available parent, and exact leading token-component equality; stop at the first mismatch.
3. Retain every unmatched, paginated, subagent-only, missing-parent, timing-only, and cross-session-signature candidate. Do not add a replay approximation marker for records that are neither removed nor proven ambiguous.
4. Encode explicit long-context and Fast rates in the effective-dated catalog. Do not hardcode global multipliers: thresholds, rates, and effective dates can change independently by model.
5. Price from the actually served tier when present. If rollouts expose only the requested/applied setting, use it as the best available estimate and mark that limitation in pricing provenance.
6. Rename the read-only report option to `--codex-homes HOME[,HOME...]`. Keep `CODEX_HOME` and mutation commands single-path; do not add a compatibility shim for the pre-1.0 report flag.
7. Across report homes, one rollout ID remains one rollout. Reuse the existing newest-file duplicate rule across the combined candidate set so copied homes do not double-count a task.

## Implementation sequence

### 1. Freeze current pricing evidence

- Reopen the official model pages and Fast-mode guide.
- Record each supported model's normal rates, long-context threshold and rates, Fast rates, and the earliest supported effective date. Use an explicit rate only when a primary source supports it; otherwise leave that component unpriced so existing lower-bound behavior applies.
- Update the catalog `as_of` date and source provenance.

Completion: every added threshold/rate/effective date has a primary-source citation, and uncertain historical applicability is represented as incomplete rather than backdated by guesswork.

### 2. Add long-context rate selection

- Extend the effective-dated catalog with an optional input-token threshold and explicit long-context rate set.
- Classify each existing per-event `Usage` value before aggregation, using gross recorded input for the threshold. Apply the selected rate set to the entire request.
- Keep normal and long-context rates independently effective-dated.
- Add focused tests at 272,000 and 272,001 input tokens, covering cached input, cache-write input, output, missing long-context components, aliases, and historical dates on both sides of a rate change.

Completion: the boundary tests prove whole-request repricing without changing ordinary-request or historical lookup behavior.

### 3. Add Standard and Fast/Priority pricing

- Inspect representative rollouts for `thread_settings_applied.service_tier` and any response field identifying the served tier. Document the observed precedence beside the parser test.
- Track the tier chronologically with the model context. Normalize `fast` and `priority` to Fast; normalize confirmed Standard/default values to Standard; surface unknown nonempty values rather than guessing.
- Extend catalog selection with explicit Standard/Fast rates, including the long-context combination where published.
- Update pricing provenance from “standard API list pricing” to describe the recorded tier basis accurately.
- Add tests for a rollout that switches Standard -> Fast -> Standard, long-context Fast usage, absent tier metadata, unknown tier values, and a served-tier value overriding a requested tier when both exist.

Completion: mixed-tier requests are priced independently, unknown tiers cannot produce a falsely complete estimate, and JSON identifies the pricing basis.

### 4. Suppress deterministic legacy-fork replay

- Preserve explicit-fork and history-mode provenance during discovery; missing history mode uses Codex's legacy default.
- For a legacy `forked_from_id` child that embeds the available parent history, compare its leading usage tuples with parent usage through the fork timestamp. Include input, cached input, cache-write input, output, reasoning output, and total; ignore rewritten timestamps.
- Advance the copied cumulative baseline without attributing matched records. Stop at the first mismatch and retain that record and everything after it, including the first genuine child request.
- Keep unmatched and unsupported shapes unchanged. Do not use the one-second burst heuristic, cross-session signatures, or speculative suffix matching.
- Bypass parent-independent analysis cache entries only for children with a nonzero matched prefix.
- Add one report-level regression fixture covering a partial snapshot race and proving that cached, uncached, cache-write, output, and reasoning usage from the first child request remains.

Completion: exact task and project reports remove only structurally proven copied records, retain the first new child request, and a fresh read-only corpus comparison matches the documented aggregate direction without retaining private evidence.

### 5. Add multi-home reporting

- Change `ReportArgs` to `--codex-homes` with Clap comma delimiting and reject empty entries. Explicit homes override the single-path `CODEX_HOME` fallback.
- Leave update and schedule arguments as `--codex-home`; their locks, SQLite writes, session-index appends, and native schedules remain scoped to one store.
- Let rollout discovery accept multiple roots and run the existing rollout-ID duplicate resolution over the combined candidates. Preserve active/archive scanning, bounded line reads, symlink policy, warnings, and deterministic newest-file selection.
- For exact task reports, search the combined index. For project reports, resolve candidates within each home, merge root IDs, and deduplicate before tree aggregation. Keep source-home paths internal and sanitized.
- Add CLI and integration tests for one home, two comma-separated homes, a parent and child split across homes, the same rollout copied into two homes, an unreadable home, an empty entry, and single-home update/schedule compatibility.

Completion: all read-only report modes aggregate multiple homes without duplicate rollout IDs, while every mutating mode still accepts and touches exactly one home.

### 6. Closeout

- Update `USERS.md`, CLI help snapshots/assertions, JSON examples, and the comparative study's local capability table.
- Reprice affected existing titles through the established bounded `--reprice-before` campaign after the pricing release; do not silently rewrite titles during reporting.
- Run formatting, focused pricing/parser/CLI tests, the full test suite, and package validation. Report every command and result.

Completion: documentation states the symbols and pricing basis, old title costs have a bounded migration path, and all requested features have behavioral tests at their accounting boundaries.

## Explicit exclusions

- No ccusage-style timing heuristic or session-agnostic event dedupe.
- No live mutable pricing fetch; use the planned catalog override/export workflow for out-of-band corrections.
- No multi-home title updates or schedules.
- No retained forensic audit data containing task identifiers, prompts, or local paths.
