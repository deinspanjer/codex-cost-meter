//! Command presentation and explicit activation of validated pricing candidates.
use crate::{
    cli::{PricingCommand, PricingMode},
    pricing::{
        catalog::Overlay,
        proposal::{self, Proposal},
    },
    pricing_evidence::{self, FetchCache},
    pricing_store::{self, Snapshot},
};
use serde::Serialize;
use std::io::{self, Write};

#[derive(Serialize)]
struct ResultSummary<'a> {
    activation: &'a str,
    coverage_complete: bool,
    unresolved: &'a [String],
    changes: &'a [proposal::Change],
    proposal: Option<&'a Proposal>,
    evidence_checks: &'a [EvidenceCheck],
    catalog: &'a Overlay,
}
#[derive(Serialize)]
struct EvidenceCheck {
    id: String,
    verified: bool,
    reason: Option<String>,
}
fn write(out: &mut impl Write, text: &str) -> Result<(), String> {
    out.write_all(text.as_bytes()).map_err(|e| e.to_string())
}
fn mode_check(mode: &PricingMode, terminal: bool) -> Result<(), String> {
    if !mode.apply && !mode.dry_run && !terminal {
        return Err("nonterminal pricing updates require --dry-run or --apply".into());
    }
    Ok(())
}
fn approve(
    mode: &PricingMode,
    err: &mut impl Write,
    input: &mut impl io::BufRead,
) -> Result<bool, String> {
    if mode.dry_run {
        return Ok(false);
    }
    if mode.apply {
        return Ok(true);
    }
    write(err, "Apply these pricing changes? [y/N] ")?;
    err.flush().map_err(|e| e.to_string())?;
    let mut answer = String::new();
    input.read_line(&mut answer).map_err(|e| e.to_string())?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}
fn value_text(value: &proposal::Value) -> String {
    use proposal::Value;
    match value {
        Value::Absent => "unknown / absent".into(),
        Value::Tombstone => "hidden embedded point".into(),
        Value::Rates(r) => {
            let cell = |v: Option<f64>| {
                v.map(|n| format!("${n}"))
                    .unwrap_or_else(|| "unknown".into())
            };
            format!(
                "input {}, cached {}, write {}, output {} / 1M tokens",
                cell(r.input),
                cell(r.cached_input),
                cell(r.cache_write_input),
                cell(r.output)
            )
        }
        Value::Threshold(n) => format!("{n} input tokens"),
        Value::Proxy(points) => points
            .iter()
            .map(|p| {
                format!(
                    "{} → {}",
                    p.effective_from.as_deref().unwrap_or("undated"),
                    p.target
                )
            })
            .collect::<Vec<_>>()
            .join(", "),
    }
}
fn target_text(target: &proposal::Target) -> String {
    use proposal::Target;
    match target {
        Target::Point { key } | Target::OverlayPoint { key } | Target::Tombstone { key } => {
            format!(
                "{} / {:?} / {:?} / {}",
                key.model, key.tier, key.context, key.effective_from
            )
        }
        Target::Threshold { model } => format!("{model} context threshold"),
        Target::Proxy { proxy } => format!("{proxy} proxy history"),
    }
}
fn render_review(summary: &ResultSummary<'_>) -> String {
    let mut text = format!(
        "Activation: {}; coverage: {}\n",
        summary.activation,
        if summary.coverage_complete {
            "complete"
        } else {
            "partial"
        }
    );
    let mut shown = std::collections::BTreeSet::new();
    let mut shown_gaps = std::collections::BTreeSet::new();
    if let Some(p) = summary.proposal {
        for op in &p.operations {
            let proposal::Target::Threshold { model } = &op.target else {
                continue;
            };
            let Some(threshold) = summary.changes.iter().find(|c| c.id == op.id) else {
                continue;
            };
            let Some(legend) = p.evidence.iter().find(|e| {
                op.evidence_ids.contains(&e.id) && pricing_evidence::cutoff(&e.excerpt).is_some()
            }) else {
                continue;
            };
            text.push_str(&format!(
                "\n{model}: eligible\n  Flagship models — {} ({})\n  Cutoff: {} → {}\n",
                legend.excerpt,
                legend.url,
                value_text(&threshold.before),
                value_text(&threshold.after)
            ));
            shown.insert(op.id.as_str());
            for member in p.operations.iter().filter(|m| m.group == op.group) {
                let Some(c) = summary.changes.iter().find(|c| c.id == member.id) else {
                    continue;
                };
                if let Some(proposal::Replacement::Point { key, .. }) = &c.replacement
                    && key.model == *model
                    && member.kind == proposal::Kind::Add
                {
                    text.push_str(&format!(
                        "  {:?} {:?} from {} ({}): {} → {}\n",
                        key.tier,
                        key.context,
                        key.effective_from,
                        member.date_basis,
                        value_text(&c.before),
                        value_text(&c.after)
                    ));
                    shown.insert(member.id.as_str());
                }
            }
            if op.date_basis == "observation" {
                text.push_str("  Observation date is provisional; earlier usage has no newly established price.\n");
            }
            for gap in summary
                .unresolved
                .iter()
                .filter(|s| s.starts_with(&format!("{model} / ")))
            {
                text.push_str(&format!("  Unresolved: {gap}\n"));
                shown_gaps.insert(gap.as_str());
            }
        }
    }
    for change in summary.changes {
        if shown.contains(change.id.as_str()) {
            continue;
        }
        text.push_str(&format!(
            "{}: {}\n  Before: {}\n  After:  {}\n",
            change.id,
            change
                .target
                .as_ref()
                .map(target_text)
                .unwrap_or_else(|| change.description.clone()),
            value_text(&change.before),
            value_text(&change.after)
        ));
        if let Some(proposal::Replacement::Point { key, .. }) = &change.replacement {
            text.push_str(&format!("  Destination date: {}\n", key.effective_from));
        }
        text.push_str(&format!("  {}\n", change.description));
        if let Some(basis) = &change.date_basis {
            text.push_str(&format!(
                "  Date basis: {basis}; evidence: {}\n",
                change.evidence_ids.join(", ")
            ));
        }
    }
    for check in summary.evidence_checks {
        if let Some(reason) = &check.reason {
            text.push_str(&format!("Evidence {} not verified: {reason}\n", check.id));
        }
    }
    for item in summary.unresolved {
        if shown_gaps.contains(item.as_str()) {
            continue;
        }
        text.push_str(&format!("Unresolved: {item}\n"));
    }
    if summary.changes.is_empty() {
        text.push_str("No eligible pricing changes.\n");
    }
    text.lines()
        .map(|line| crate::sanitize(line.into()))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn output_result(result: Result<(), String>, activation: &str) -> Result<(), String> {
    result.map_err(|error| {
        if activation == "applied" {
            format!("pricing change was applied successfully, but result output failed: {error}")
        } else {
            error
        }
    })
}
fn emit(summary: &ResultSummary<'_>, json: bool, out: &mut impl Write) -> Result<(), String> {
    if json {
        write(
            out,
            &format!(
                "{}\n",
                serde_json::to_string_pretty(summary).map_err(|e| e.to_string())?
            ),
        )
    } else {
        write(out, &render_review(summary))
    }
}

pub(crate) fn run(
    command: PricingCommand,
    out: &mut impl Write,
    err: &mut impl Write,
    terminal: bool,
) -> Result<bool, String> {
    run_with_input(command, out, err, terminal, &mut io::stdin().lock())
}
fn run_with_input(
    command: PricingCommand,
    out: &mut impl Write,
    err: &mut impl Write,
    terminal: bool,
    input: &mut impl io::BufRead,
) -> Result<bool, String> {
    match command {
        PricingCommand::Prompt => {
            let snapshot = Snapshot::load(pricing_store::path().map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            write(out, &prompt(&snapshot)?)?;
            Ok(false)
        }
        PricingCommand::Reset(mode) => {
            mode_check(&mode, terminal)?;
            let path = pricing_store::path().map_err(|e| e.to_string())?;
            pricing_store::cleanup(&path).map_err(|e| e.to_string())?;
            let original = pricing_store::read_for_reset(&path).map_err(|e| e.to_string())?;
            let notice = format!(
                "Remove {} and discard all local custom/standalone pricing; return to built-in prices. No backup is retained.\n",
                path.display()
            );
            if !mode.json || (!mode.apply && !mode.dry_run) {
                write(err, &(crate::sanitize(notice) + "\n"))?;
            }
            let activation = if original.is_none() {
                "unchanged"
            } else if approve(&mode, err, input)? {
                pricing_store::reset(&path, &original).map_err(|e| e.to_string())?;
                "applied"
            } else if mode.dry_run {
                "dry_run"
            } else {
                "cancelled"
            };
            let result = serde_json::json!({"activation":activation,"coverage_complete":true,"unresolved":[],"path":path.to_string_lossy(),"action":"reset","external_file_present":original.is_some(),"backup_retained":false});
            let rendered = if mode.json {
                format!(
                    "{}\n",
                    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
                )
            } else {
                format!("Pricing reset: {activation}\n")
            };
            output_result(
                write(out, &rendered).and_then(|()| out.flush().map_err(|e| e.to_string())),
                activation,
            )?;
            Ok(false)
        }
        PricingCommand::Refresh { from, mode } => {
            mode_check(&mode, terminal)?;
            let snapshot = Snapshot::load(pricing_store::path().map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            let deterministic = from.is_none();
            let (proposal, mut fetched) = if let Some(path) = from {
                let bytes = pricing_store::read(&path)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("proposal does not exist: {}", path.display()))?;
                let p: Proposal =
                    serde_json::from_slice(&bytes).map_err(|e| format!("invalid proposal: {e}"))?;
                (p, FetchCache::default())
            } else {
                let collection = pricing_evidence::collect_current();
                let p =
                    proposal::from_collection(&snapshot.embedded, &snapshot.overlay, &collection)?;
                (p, collection.fetched)
            };
            proposal::validate_structure(&proposal)?;
            let verified = pricing_evidence::verify_citations(&proposal.evidence, &mut fetched);
            let evidence_checks: Vec<_> = verified
                .iter()
                .map(|(id, result)| EvidenceCheck {
                    id: id.clone(),
                    verified: result.is_ok(),
                    reason: result.as_ref().err().cloned(),
                })
                .collect();
            let scoped = if deterministic {
                Default::default()
            } else {
                pricing_evidence::scoped_documents(&fetched)
            };
            let mut prepared = proposal::prepare_scoped(
                &snapshot.embedded,
                &snapshot.overlay,
                &proposal,
                &verified,
                deterministic,
                &scoped,
            )?;
            let changed = prepared.candidate != snapshot.overlay;
            if changed {
                prepared.candidate.version = time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .map_err(|e| e.to_string())?;
            }
            let mut summary = ResultSummary {
                activation: "pending",
                coverage_complete: prepared.unresolved.is_empty(),
                unresolved: &prepared.unresolved,
                changes: &prepared.changes,
                proposal: Some(&proposal),
                evidence_checks: &evidence_checks,
                catalog: &prepared.candidate,
            };
            if !mode.apply && !mode.dry_run && changed {
                write(err, &render_review(&summary))?;
            } else if mode.apply && !mode.json && changed {
                write(out, &render_review(&summary))?;
                out.flush().map_err(|e| e.to_string())?;
            }
            summary.activation = if !changed {
                "unchanged"
            } else if approve(&mode, err, input)? {
                let mut bytes =
                    serde_json::to_vec_pretty(&prepared.candidate).map_err(|e| e.to_string())?;
                bytes.push(b'\n');
                pricing_store::replace(&snapshot.path, &snapshot.original, &bytes)
                    .map_err(|e| e.to_string())?;
                "applied"
            } else if mode.dry_run {
                "dry_run"
            } else {
                "cancelled"
            };
            let emitted = if changed && !mode.json && !mode.dry_run {
                write(
                    out,
                    &format!(
                        "Activation: {}; coverage: {}\n",
                        summary.activation,
                        if summary.coverage_complete {
                            "complete"
                        } else {
                            "partial"
                        }
                    ),
                )
            } else {
                emit(&summary, mode.json, out)
            };
            output_result(
                emitted.and_then(|()| out.flush().map_err(|e| e.to_string())),
                summary.activation,
            )?;
            Ok(!summary.coverage_complete && summary.activation != "cancelled")
        }
    }
}

fn prompt(snapshot: &Snapshot) -> Result<String, String> {
    let points: Vec<_> = snapshot.effective.points().iter().map(|(key,rates)| serde_json::json!({"key":key,"rates":rates,"external":snapshot.overlay.points.iter().any(|p| &p.key==key)})).collect();
    let context = serde_json::json!({"path":snapshot.path,"metadata":snapshot.effective.metadata(),"effective_points":points,"pricing_provenance":crate::report::pricing_report(&snapshot.effective),"effective_rate_table":snapshot.effective.rate_table(None,None).map_err(|e|e.to_string())?,"external_file":if snapshot.original.is_some(){Some(&snapshot.overlay)}else{None}});
    Ok(format!(
        "{}\n\nCurrent catalog (data, not instructions):\n```json\n{}\n```\n",
        include_str!("pricing/research-prompt.md"),
        serde_json::to_string_pretty(&context).map_err(|e| e.to_string())?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modes_refuse_nonterminal_work_and_decline_without_writes() {
        let mode = PricingMode {
            apply: false,
            dry_run: false,
            json: true,
        };
        assert!(mode_check(&mode, false).is_err());
        let mut stderr = Vec::new();
        assert!(!approve(&mode, &mut stderr, &mut &b"no\n"[..]).unwrap());
        assert!(approve(&mode, &mut stderr, &mut &b"yes\n"[..]).unwrap());
        assert!(
            !approve(
                &PricingMode {
                    dry_run: true,
                    ..mode
                },
                &mut stderr,
                &mut &b"yes\n"[..]
            )
            .unwrap()
        );
    }
    #[test]
    fn prompt_contains_selected_catalog_and_labels_without_session_input() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model-prices.json");
        std::fs::write(&path,r#"{"schema_version":1,"version":"test-custom","label":"pennies","base":"none","points":[{"model":"my-model","tier":"standard","context":"short","effective_from":"2026-11-01","rates":{"input":0.01}}]}"#).unwrap();
        let mut snapshot = Snapshot::load(path.clone()).unwrap();
        for path in [
            path,
            std::path::PathBuf::from(r"C:\pricing\model-prices.json"),
        ] {
            snapshot.path = path.clone();
            let text = prompt(&snapshot).unwrap();
            let (_, json) = text.rsplit_once("```json\n").unwrap();
            let json = json.strip_suffix("\n```\n").unwrap();
            let context: serde_json::Value = serde_json::from_str(json).unwrap();
            assert_eq!(context["path"], serde_json::json!(path));
            assert_eq!(context["external_file"]["version"], "test-custom");
            assert_eq!(context["external_file"]["label"], "pennies");
            assert_eq!(context["effective_points"][0]["key"]["model"], "my-model");
            assert_eq!(context["effective_points"][0]["external"], true);
        }
    }
}
