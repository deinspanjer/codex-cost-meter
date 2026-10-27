use std::{
    collections::{BTreeSet, HashMap},
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::json;
use thiserror::Error;
use time::format_description::well_known::Rfc3339;

use crate::{
    date_filter::Filter,
    report::{
        Aggregate, ProjectSelection, ReportContext, ReportError, pricing_report, service_tier_key,
    },
    rollout::analysis::{analyze_observed, replay_prefix_len},
};

pub(crate) enum Selection<'a> {
    Thread(&'a str),
    Project(&'a ProjectSelection, &'a [String]),
    All,
}

#[derive(Debug, Error)]
pub(crate) enum ExportError {
    #[error(transparent)]
    Report(#[from] ReportError),
    #[error(transparent)]
    Date(#[from] crate::date_filter::Error),
    #[error("could not serialize usage export: {0}")]
    Json(#[from] serde_json::Error),
    #[error("could not write usage export: {0}")]
    Write(#[from] io::Error),
    #[error("could not format usage timestamp: {0}")]
    Timestamp(#[from] time::error::Format),
}

#[derive(Serialize)]
struct UsageRow<'a> {
    r#type: &'static str,
    thread_id: &'a str,
    parent_thread_id: Option<&'a str>,
    segment_file: &'a Path,
    source_line: u64,
    ordinal: Option<u64>,
    turn_id: Option<&'a str>,
    timestamp: Option<String>,
    timestamp_source: &'static str,
    model: &'a str,
    service_tier: &'a str,
    input_tokens: u64,
    input_cache_read_tokens: u64,
    input_cache_write_tokens: u64,
    output_tokens: u64,
    reasoning_tokens: u64,
    estimated_cost_usd: Option<f64>,
    known_model_cost_usd: f64,
}

fn line(writer: &mut impl Write, value: &impl Serialize) -> Result<(), ExportError> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn write(
    context: &ReportContext,
    selection: Selection<'_>,
    filter: &Filter,
    writer: &mut impl Write,
) -> Result<(), ExportError> {
    let (roots, selected) = match selection {
        Selection::Thread(id) => {
            if context.index.record(id).is_none() {
                return Err(ReportError::RolloutNotFound {
                    thread_id: id.into(),
                    home: "selected Codex home".into(),
                }
                .into());
            }
            (
                Some(vec![id.to_owned()]),
                json!({"scope":"thread","thread_id":id}),
            )
        }
        Selection::Project(selection, roots) => (
            Some(roots.to_vec()),
            json!({"scope":"project","target":selection.target,"resolver":selection.resolver,"root_thread_ids":roots,"missing_source_roots":selection.missing_source_roots}),
        ),
        Selection::All => (None, json!({"scope":"corpus"})),
    };
    let ids = match roots {
        None => context
            .index
            .records()
            .map(|record| record.id.clone())
            .collect::<BTreeSet<_>>(),
        Some(roots) => roots
            .into_iter()
            .flat_map(|id| {
                let descendants = context.index.descendants(&id).unwrap_or_default();
                std::iter::once(id).chain(descendants)
            })
            .collect(),
    };
    line(
        writer,
        &json!({"type":"metadata","schema_version":1,"selection":selected,"date_range":{"since":filter.since.map(|date|date.to_string()),"through":filter.through.map(|date|date.to_string())},"pricing":pricing_report(&context.catalog),"record_basis":"normalized token_count usage events; not guaranteed one-to-one API requests","source_line_basis":"1-based physical line after decompression"}),
    )?;
    let mut total = Aggregate::default();
    let mut warnings = context
        .index
        .warnings()
        .iter()
        .map(|warning| {
            format!(
                "rollout scan could not read {} ({})",
                warning.path.display(),
                warning.error
            )
        })
        .collect::<Vec<_>>();
    if context.index.malformed_lines_skipped() > 0 {
        warnings.push("rollout scan skipped malformed JSONL records".into());
    }
    if context.index.oversized_lines_skipped() > 0 {
        warnings.push("rollout scan skipped oversized JSONL records".into());
    }
    let discovery_incomplete = !warnings.is_empty();
    if selected
        .get("missing_source_roots")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        > 0
    {
        warnings.push("configured Desktop Project source roots unavailable; direct assignments and recorded workspace roots remain included".into());
    }
    let mut rows = 0_u64;
    let mut excluded_untimestamped = 0_u64;
    let mut paths = HashMap::<PathBuf, PathBuf>::new();
    for id in ids {
        let Some(record) = context.index.record(&id) else {
            total.add_unreadable();
            warnings.push(format!("selected rollout {id} could not be read"));
            continue;
        };
        let replay = record
            .parent_id
            .as_deref()
            .and_then(|parent| context.index.record(parent))
            .map_or(0, |parent| replay_prefix_len(parent, record));
        let mut failure = None;
        let stats = analyze_observed(
            record,
            replay,
            Some(&mut |event, source| {
                if failure.is_some() {
                    return;
                }
                let result = (|| -> Result<(), ExportError> {
                    if filter.is_filtered() && event.at.is_none() {
                        excluded_untimestamped += 1;
                    }
                    if !filter.includes(event.at)? {
                        return Ok(());
                    }
                    let cost = context.catalog.cost(
                        &event.model,
                        event.at,
                        &event.service_tier,
                        event.usage,
                    );
                    let path = paths.entry(source.path.to_owned()).or_insert_with(|| {
                        source
                            .path
                            .canonicalize()
                            .unwrap_or_else(|_| source.path.to_owned())
                    });
                    line(
                        writer,
                        &UsageRow {
                            r#type: "usage",
                            thread_id: &id,
                            parent_thread_id: record.parent_id.as_deref(),
                            segment_file: path,
                            source_line: source.line,
                            ordinal: source.ordinal,
                            turn_id: source.turn_id,
                            timestamp: event.at.map(|at| at.format(&Rfc3339)).transpose()?,
                            timestamp_source: source.timestamp_source,
                            model: &event.model,
                            service_tier: service_tier_key(&event.service_tier),
                            input_tokens: event.usage.input,
                            input_cache_read_tokens: event.usage.cached_input,
                            input_cache_write_tokens: event.usage.cache_write_input,
                            output_tokens: event.usage.output,
                            reasoning_tokens: event.reasoning_output,
                            estimated_cost_usd: cost.complete,
                            known_model_cost_usd: cost.known,
                        },
                    )?;
                    rows += 1;
                    Ok(())
                })();
                if let Err(error) = result {
                    failure = Some(error);
                }
            }),
        );
        if let Some(error) = failure {
            return Err(error);
        }
        match stats {
            Ok(stats) => {
                if stats.incomplete_usage || stats.unattributed_tokens > 0 {
                    warnings.push(format!("rollout {id} contains incomplete or unattributed usage; inspect summary completeness"));
                }
                if filter.is_filtered()
                    && stats
                        .turn_events
                        .iter()
                        .any(|turn| turn.started_at.is_none())
                {
                    warnings.push(format!(
                        "rollout {id} contains turns without a usable timestamp"
                    ));
                }
                total.add_filtered(&stats, &context.catalog, Some(filter))?;
            }
            Err(source) => return Err(ReportError::SelectedRolloutUnreadable { id, source }.into()),
        }
    }
    if excluded_untimestamped > 0 {
        warnings.push("date-filtered export excluded usage without a usable timestamp".into());
    }
    let mut stats = total.report();
    if discovery_incomplete {
        stats.incomplete_input = true;
        stats.estimated_cost_usd = None;
    }
    line(
        writer,
        &json!({"type":"summary","usage_rows":rows,"excluded_untimestamped_usage_events":excluded_untimestamped,"stats":stats,"incomplete_input_warnings":warnings}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date_filter::Options;

    struct BrokenAfterHeader(Vec<u8>);

    impl Write for BrokenAfterHeader {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0.contains(&b'\n') {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "consumer closed"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn failed_usage_write_returns_error_without_a_success_summary() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join("sessions")).unwrap();
        std::fs::write(home.path().join("sessions/root.jsonl"), concat!(
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"root\",\"cwd\":\"/project\"}}\n",
            "{\"type\":\"turn_context\",\"payload\":{\"model\":\"gpt-6.1-terra\"}}\n",
            "{\"type\":\"event_msg\",\"timestamp\":\"2026-10-26T12:00:00Z\",\"payload\":{\"type\":\"token_count\",\"info\":{\"last_token_usage\":{\"input_tokens\":10,\"total_tokens\":10}}}}\n"
        )).unwrap();
        let context = ReportContext::new(home.path()).unwrap();
        let filter = Filter::try_from(Options {
            since: None,
            through: None,
            group_by: vec![],
            include_empty: false,
        })
        .unwrap();
        let mut writer = BrokenAfterHeader(Vec::new());
        let error = write(&context, Selection::All, &filter, &mut writer).unwrap_err();
        assert!(error.to_string().contains("consumer closed"));
        let metadata: serde_json::Value = serde_json::from_slice(&writer.0).unwrap();
        assert_eq!(metadata["type"], "metadata");
    }
}
