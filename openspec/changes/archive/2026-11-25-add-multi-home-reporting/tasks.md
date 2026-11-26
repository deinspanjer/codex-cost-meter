# Tasks

## 1. CLI home selection

- [x] 1.1 Add `--codex-homes` to `ReportArgs` in `src/cli.rs` with comma delimiting, repeat-to-append, an element parser that rejects empty entries, and a conflict with `--codex-home`. Add `CodexHomes` and `ReportArgs::codex_homes()`, which validates explicit entries (missing or not a directory gives `CliError::HomeUnavailable` naming the entry) and removes duplicates by canonical path. The no-option path keeps the existing single-path resolution. Verify with parser unit tests: `A,B` and repeated occurrences give the same list; `A,,B`, `A,`, `,A`, and `""` are rejected; `--codex-home` with `--codex-homes` is rejected; `update` and `schedule install` reject `--codex-homes`; a comma inside `--codex-home` stays one path; a missing entry is named in the error; two spellings of one directory give one home.
- [x] 1.2 Write the help text for report's `--codex-homes` (comma list, report only, overrides `CODEX_HOME`) and `--codex-home` (single path). Do not add assertions on the help prose. Verify by reading `cargo run -- report --help`, and confirm with `just test-filter completions` that the existing completion tests still pass.

## 2. Discovery over a home set

- [x] 2.1 Add the internal `home: HomeId` to `RolloutRecord` and `holders` to `RolloutIndex`. Change the full-scan and corpus builders to take the home set and per-home caches, and run one `resolve_duplicates` over the combined candidates. Verify with unit tests in `src/rollout/discovery.rs`: the same ID copied into two homes resolves to the newer copy and lists both holders; home order `A,B` and `B,A` gives identical records; `--all --since` prunes a home that has metadata while keeping every candidate from a home without metadata; existing single-home discovery tests pass unchanged.
- [x] 2.2 Add the containment rule for SQLite-provided rollout paths (design decision 4). Verify with a unit test in which a copied home's `state_5.sqlite` holds absolute paths into another directory: the corpus and targeted paths read only that home's own files.
- [x] 2.3 Route exact and project worksets with more than one home to the combined full scan, and keep the single-home targeted lookup. In the targeted lookup, keep each requested record's parent for analysis, and fall back to the full scan when a requested record's parent was not found. Verify with unit tests: root R and child C1 in A with grandchild G only in B gives {R, C1, G}; a single-home targeted build for fork child C, with parent P present, has P in the index but not in C's descendants.

## 3. Report context, caches, and names

- [x] 3.1 Replace `Rc<RolloutCache>` with per-home caches held by `Homes`, selected by `record.home` for analysis and by home for discovery. Write the notices of each cache, with a sanitized home label when more than one home is selected. Verify with a test in which one home's cache is unwritable: that home emits its notice, and the other home's cache entries are still written and reused on a second run.
- [x] 3.2 Hold one `Names` per home with holding-home-first lookup. Make `RolloutNotFound` list every home and use "selected Codex homes" in `usage_export` when several homes are selected. Make `--match-title` resolve over the combined roots. Verify with tests: a root copied into two homes matches a title once; a name present only in the non-holding home is still shown; with three homes, the title fallback is the same for every argument order; the not-found error names both homes.
- [x] 3.3 Thread the home set through the `report` branch of `src/main.rs` for exact, title, corpus, project, and JSONL modes. Verify with `tests/report_cli.rs` integration tests: an exact report with the parent in A and the subagent child in B includes the child; `--all --json` over a copied rollout counts it once and produces the same JSON for `A,B` and `B,A`; `--jsonl --all` over two homes emits each rollout's events once; a legacy fork child in B whose parent is in A suppresses its copied prefix and keeps the first genuine request; `report <fork-child>` and `report <fork-child> --jsonl`, run on one home and on two, exclude the copied prefix and leave out the parent's usage; one home whose `sessions` directory cannot be read gives a report with incomplete totals and a warning naming that home (Unix only; skip where permissions cannot be enforced); a mistyped entry fails before any output; every existing `report --codex-home` test passes unchanged, except tests that encode the fork-child bug.

## 4. Project resolution across homes

- [x] 4.1 Load Desktop state for each home and resolve references into a `Target` that groups projects by case-insensitive name across homes, keeping the existing ambiguity errors within a home and between distinct names. Project ownership of a path keeps today's exact canonical-path equality. Verify with tests: `--project .` run from a subdirectory of a source root does not select that project; "Billing" with different IDs in A and B resolves to one project with the union of source roots; two "Billing" projects in A are ambiguous; a project present only in A still includes B's unassigned roots beneath its source root; a reference that matches in no home is not found.
- [x] 4.2 Classify each unique root in every holding home with inclusion precedence, and count it once. Derive the target of a task-scoped `--project` from the root's holding home. Verify with tests: a copied root that A assigns to the project and B marks projectless is counted once as a direct assignment; an unrelated unassigned root outside every source root changes no counter; a root held only by B is decided by B's state alone; `report R --project` with R assigned in A reports the same-named project's roots from both homes; a holding home without metadata gives `MetadataUnavailable`.

## 5. Documentation and release

- [x] 5.1 Edit the existing home-resolution and cache sentences in `USERS.md` (around line 59) rather than adding a section. Add one example `report --all --codex-homes A,B` command and at most three sentences: each rollout counts once, and the newest copy wins; same-named Desktop Projects merge across homes; totals are not the sum of the single-home reports. Verify by running the example against fixture homes.
- [x] 5.2 Mark step 5 and decision 6 of `docs/plans/ccusage-accounting-pricing-and-multi-home-handoff.md` as superseded by this change, and record the kept `--codex-home` option and the 1.10.0 release. In `docs/research/ccusage-codex-primary-source-findings.md`, update the "Storage roots" row of the capability table and the local-resolution paragraph. Note that ccusage deduplicates by (home, relative path), so copies across homes count twice there, while this tool counts each rollout ID once. Verify by reading the rendered Markdown.
- [x] 5.3 Run `just bump minor`, add a `CHANGELOG.md` entry for 1.10.0, and verify with `just test-version` and `just version-verify`.

## 6. Integration checks

- [x] 6.1 Run `just fmt`, `cargo clippy --all-targets -- -D warnings`, `just test`, and `just package`. Record each command and its result in the PR.
- [x] 6.2 Using a disposable copy of a real home and the original, compare `report --all --json` for `--codex-home ORIG` against `--codex-homes ORIG,COPY`. Totals must be identical when the copy has no new activity, and no Codex file in either home may change other than the cache, checked with a before-and-after file listing and hashes. Do not retain identifiers or paths from this check.

## Workflow follow-up

- Carry the release PR through merge, then verify the public GitHub release and the merged Homebrew formula PR before reporting the change shipped.
- Archive the change after release verification.
