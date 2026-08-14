use std::{
    collections::{BTreeMap, HashMap},
    io,
    path::Path,
};

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    pricing::{Catalog, PricingError, Usage},
    rollout::{
        analysis::{AnalysisError, RolloutStats, analyze},
        discovery::{RolloutIndex, read_jsonl},
    },
};

#[derive(Debug, Error)]
pub(crate) enum ReportError {
    #[error("rollout not found under {home}: {thread_id}")]
    RolloutNotFound { thread_id: String, home: String },
    #[error("selected rollout {id} could not be read: {source}")]
    SelectedRolloutUnreadable {
        id: String,
        #[source]
        source: AnalysisError,
    },
    #[error(transparent)]
    Pricing(#[from] PricingError),
}

#[derive(Debug, Serialize)]
pub(crate) struct Report {
    pub rollout: RolloutReport,
    pub tree: StatsReport,
    pub by_model: BTreeMap<String, ModelReport>,
    pub by_rollout_type: BTreeMap<String, StatsReport>,
    pub pricing: PricingReport,
    pub incomplete_input_warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RolloutReport {
    pub rollout_id: String,
    pub rollout_type: String,
    pub project: Option<String>,
    pub thread_name: Option<String>,
    pub total_subagent_spawns: usize,
    pub total_subagent_turn_duration_seconds: f64,
    #[serde(flatten)]
    pub stats: StatsReport,
}

#[derive(Debug, Serialize)]
pub(crate) struct StatsReport {
    pub rollout_count: usize,
    pub majority_turn_model: Option<String>,
    pub majority_reasoning_level: Option<String>,
    pub input_tokens: u64,
    pub input_cache_write_tokens: u64,
    pub input_cache_read_tokens: u64,
    pub reasoning_tokens: u64,
    pub output_tokens: u64,
    pub turns: usize,
    pub completed_or_aborted_turns: usize,
    pub incomplete_turns: usize,
    pub total_turn_duration_seconds: f64,
    pub estimated_cost_usd: Option<f64>,
    pub known_model_cost_usd: f64,
    pub unpriced_models: BTreeMap<String, u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unattributed_usage_tokens: Option<u64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub incomplete_input: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModelReport {
    pub turns: usize,
    pub input_tokens: u64,
    pub input_cache_write_tokens: u64,
    pub input_cache_read_tokens: u64,
    pub reasoning_tokens: u64,
    pub output_tokens: u64,
    pub estimated_cost_usd: Option<f64>,
    pub known_model_cost_usd: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct PricingReport {
    pub basis: &'static str,
    pub as_of: String,
    pub source: String,
    pub model_proxies: BTreeMap<String, String>,
}

#[derive(Default)]
struct Aggregate {
    rollout_count: usize,
    usage: Usage,
    reasoning_output: u64,
    turns: usize,
    ended_turns: usize,
    duration_seconds: f64,
    turn_models: HashMap<(String, String), usize>,
    known_cost: f64,
    unpriced_models: BTreeMap<String, u64>,
    unattributed_tokens: u64,
    incomplete: bool,
    models: BTreeMap<String, ModelAggregate>,
}

#[derive(Default)]
struct ModelAggregate {
    usage: Usage,
    reasoning_output: u64,
    turns: usize,
    known_cost: f64,
    incomplete: bool,
}

impl Aggregate {
    fn add(&mut self, stats: &RolloutStats, catalog: &Catalog) {
        self.rollout_count = self.rollout_count.saturating_add(1);
        self.usage.input = self.usage.input.saturating_add(stats.known_usage.input);
        self.usage.cached_input = self
            .usage
            .cached_input
            .saturating_add(stats.known_usage.cached_input);
        self.usage.cache_write_input = self
            .usage
            .cache_write_input
            .saturating_add(stats.known_usage.cache_write_input);
        self.usage.output = self.usage.output.saturating_add(stats.known_usage.output);
        self.reasoning_output = self.reasoning_output.saturating_add(stats.reasoning_output);
        self.turns = self.turns.saturating_add(stats.turns);
        self.ended_turns = self.ended_turns.saturating_add(stats.ended_turns);
        self.duration_seconds += stats.duration.as_seconds_f64();
        self.unattributed_tokens = self
            .unattributed_tokens
            .saturating_add(stats.unattributed_tokens);
        self.incomplete |= stats.incomplete_usage || stats.unattributed_tokens > 0;

        for ((model, effort), count) in &stats.turn_models {
            *self
                .turn_models
                .entry((model.clone(), effort.clone()))
                .or_default() += count;
            self.models.entry(model.clone()).or_default().turns += count;
        }
        for event in &stats.events {
            let model = self.models.entry(event.model.clone()).or_default();
            model.usage.input = model.usage.input.saturating_add(event.usage.input);
            model.usage.cached_input = model
                .usage
                .cached_input
                .saturating_add(event.usage.cached_input);
            model.usage.cache_write_input = model
                .usage
                .cache_write_input
                .saturating_add(event.usage.cache_write_input);
            model.usage.output = model.usage.output.saturating_add(event.usage.output);
            model.reasoning_output = model
                .reasoning_output
                .saturating_add(event.reasoning_output);

            let cost = catalog.cost(&event.model, event.at, event.usage);
            self.known_cost += cost.known;
            model.known_cost += cost.known;
            if cost.complete.is_none() {
                let tokens = event.usage.input.saturating_add(event.usage.output);
                *self.unpriced_models.entry(event.model.clone()).or_default() += tokens;
                model.incomplete = true;
                self.incomplete = true;
            }
        }
    }

    fn add_unreadable(&mut self) {
        self.rollout_count = self.rollout_count.saturating_add(1);
        self.incomplete = true;
    }

    fn report(self) -> StatsReport {
        let majority = self
            .turn_models
            .iter()
            .max_by(|(left, left_count), (right, right_count)| {
                left_count.cmp(right_count).then_with(|| right.cmp(left))
            })
            .map(|((model, effort), _)| (model.clone(), effort.clone()));
        let estimated_cost_usd = (!self.incomplete).then(|| round(self.known_cost, 8));
        StatsReport {
            rollout_count: self.rollout_count,
            majority_turn_model: majority.as_ref().map(|(model, _)| model.clone()),
            majority_reasoning_level: majority.map(|(_, effort)| effort),
            input_tokens: self.usage.input,
            input_cache_write_tokens: self.usage.cache_write_input,
            input_cache_read_tokens: self.usage.cached_input,
            reasoning_tokens: self.reasoning_output,
            output_tokens: self.usage.output,
            turns: self.turns,
            completed_or_aborted_turns: self.ended_turns,
            incomplete_turns: self.turns.saturating_sub(self.ended_turns),
            total_turn_duration_seconds: round(self.duration_seconds, 3),
            estimated_cost_usd,
            known_model_cost_usd: round(self.known_cost, 8),
            unpriced_models: self.unpriced_models,
            unattributed_usage_tokens: (self.unattributed_tokens > 0)
                .then_some(self.unattributed_tokens),
            incomplete_input: self.incomplete,
        }
    }

    fn model_reports(&self) -> BTreeMap<String, ModelReport> {
        self.models
            .iter()
            .map(|(name, model)| {
                (
                    name.clone(),
                    ModelReport {
                        turns: model.turns,
                        input_tokens: model.usage.input,
                        input_cache_write_tokens: model.usage.cache_write_input,
                        input_cache_read_tokens: model.usage.cached_input,
                        reasoning_tokens: model.reasoning_output,
                        output_tokens: model.usage.output,
                        estimated_cost_usd: (!model.incomplete).then(|| round(model.known_cost, 8)),
                        known_model_cost_usd: round(model.known_cost, 8),
                    },
                )
            })
            .collect()
    }
}

pub(crate) fn build(thread_id: &str, codex_home: &Path) -> Result<Report, ReportError> {
    let index = RolloutIndex::build(codex_home).unwrap_or_else(|error| match error {});
    let catalog = Catalog::embedded()?;
    build_with_index(thread_id, codex_home, &index, &catalog)
}

fn build_with_index(
    thread_id: &str,
    codex_home: &Path,
    index: &RolloutIndex,
    catalog: &Catalog,
) -> Result<Report, ReportError> {
    let root = index
        .record(thread_id)
        .ok_or_else(|| ReportError::RolloutNotFound {
            thread_id: thread_id.into(),
            home: codex_home.display().to_string(),
        })?;
    let descendants = index.descendants(thread_id).unwrap_or_default();
    let mut root_stats = Aggregate::default();
    let mut children_stats = Aggregate::default();
    let mut tree_stats = Aggregate::default();
    let mut by_rollout_type = BTreeMap::<String, Aggregate>::new();
    let mut warnings = index
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

    if index.malformed_lines_skipped() > 0 {
        warnings.push("rollout scan skipped malformed JSONL records".into());
        tree_stats.incomplete = true;
    }
    if index.oversized_lines_skipped() > 0 {
        warnings.push("rollout scan skipped oversized JSONL records".into());
        tree_stats.incomplete = true;
    }
    if !index.warnings().is_empty() {
        tree_stats.incomplete = true;
    }

    for id in std::iter::once(thread_id).chain(descendants.iter().map(String::as_str)) {
        let record = index.record(id).expect("indexed descendant must exist");
        let rollout_type = record.kind.report_type();
        let type_stats = by_rollout_type.entry(rollout_type).or_default();
        match analyze(record) {
            Ok(stats) => {
                tree_stats.add(&stats, catalog);
                type_stats.add(&stats, catalog);
                if id == thread_id {
                    root_stats.add(&stats, catalog);
                } else {
                    children_stats.add(&stats, catalog);
                }
            }
            Err(source) if id == thread_id => {
                return Err(ReportError::SelectedRolloutUnreadable {
                    id: thread_id.into(),
                    source,
                });
            }
            Err(_) => {
                tree_stats.add_unreadable();
                type_stats.add_unreadable();
                children_stats.add_unreadable();
                warnings.push(format!("descendant rollout {id} could not be read"));
            }
        }
    }

    let by_model = tree_stats.model_reports();
    let tree = tree_stats.report();
    let root_stats = root_stats.report();
    let children_stats = children_stats.report();
    Ok(Report {
        rollout: RolloutReport {
            rollout_id: thread_id.into(),
            rollout_type: root.kind.report_type(),
            project: root.cwd.clone(),
            thread_name: latest_thread_name(codex_home, thread_id, &mut warnings),
            total_subagent_spawns: descendants.len(),
            total_subagent_turn_duration_seconds: children_stats.total_turn_duration_seconds,
            stats: root_stats,
        },
        tree,
        by_model,
        by_rollout_type: by_rollout_type
            .into_iter()
            .map(|(kind, stats)| (kind, stats.report()))
            .collect(),
        pricing: PricingReport {
            basis: "standard API list pricing; per request/turn model; output includes reasoning",
            as_of: catalog.as_of().into(),
            source: catalog.source().into(),
            model_proxies: catalog
                .proxies()
                .iter()
                .map(|(model, target)| (model.clone(), target.clone()))
                .collect(),
        },
        incomplete_input_warnings: warnings,
    })
}

fn latest_thread_name(
    codex_home: &Path,
    thread_id: &str,
    warnings: &mut Vec<String>,
) -> Option<String> {
    let path = codex_home.join("session_index.jsonl");
    let mut latest = None::<(OffsetDateTime, String)>;
    let mut malformed = false;
    let result = read_jsonl(&path, |line| {
        let Ok(value) = serde_json::from_slice::<Value>(line) else {
            malformed = true;
            return;
        };
        let Some(object) = value.as_object() else {
            malformed = true;
            return;
        };
        if object.get("id").and_then(Value::as_str) != Some(thread_id) {
            return;
        }
        let Some(name) = object.get("thread_name").and_then(Value::as_str) else {
            return;
        };
        if name.is_empty() {
            return;
        }
        let Some(updated_at) = object.get("updated_at").and_then(Value::as_str) else {
            malformed = true;
            return;
        };
        let Ok(updated_at) = OffsetDateTime::parse(updated_at, &Rfc3339) else {
            malformed = true;
            return;
        };
        if latest
            .as_ref()
            .is_none_or(|(current, _)| updated_at >= *current)
        {
            latest = Some((updated_at, name.into()));
        }
    });
    let mut invalid = malformed;
    match result {
        Ok(summary) if summary.oversized_lines_skipped > 0 => {
            warnings.push("session index skipped oversized JSONL records".into());
            invalid = true;
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
        Err(_) => {
            warnings.push("session index could not be read".into());
            return None;
        }
    }
    if malformed {
        warnings.push("session index contains malformed JSONL records".into());
    }
    (!invalid).then(|| latest.map(|(_, name)| name)).flatten()
}

fn round(value: f64, decimal_places: i32) -> f64 {
    let scale = 10_f64.powi(decimal_places);
    (value * scale).round() / scale
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::{Value, json};
    use tempfile::TempDir;

    use super::{ReportError, build, build_with_index};
    use crate::{pricing::Catalog, rollout::discovery::RolloutIndex};

    fn write_jsonl(home: &TempDir, relative: &str, rows: &[Value]) {
        let path = home.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            rows.iter()
                .map(|row| format!("{row}\n"))
                .collect::<String>(),
        )
        .unwrap();
    }

    fn rollout(
        id: &str,
        parent: Option<&str>,
        source: Value,
        model: &str,
        input: u64,
    ) -> Vec<Value> {
        let mut payload = json!({"id": id, "source": source, "cwd": "/tmp/project"});
        if let Some(parent) = parent {
            payload["parent_thread_id"] = json!(parent);
        }
        vec![
            json!({
                "type": "session_meta",
                "timestamp": "2026-08-13T12:00:00Z",
                "payload": payload,
            }),
            json!({"type": "turn_context", "payload": {"model": model, "effort": "high"}}),
            json!({
                "type": "event_msg",
                "timestamp": "2026-08-13T12:00:00Z",
                "payload": {
                    "type": "token_count",
                    "info": {
                        "last_token_usage": {"input_tokens": input, "total_tokens": input},
                    },
                },
            }),
        ]
    }

    fn fixture_home() -> TempDir {
        let home = TempDir::new().unwrap();
        write_jsonl(
            &home,
            "sessions/root.jsonl",
            &rollout("root", None, json!("cli"), "gpt-5.6-terra", 100),
        );
        write_jsonl(
            &home,
            "archived_sessions/child.jsonl",
            &rollout(
                "child",
                Some("root"),
                json!({"subagent": {"other": "guardian"}}),
                "unpriced-model",
                20,
            ),
        );
        write_jsonl(
            &home,
            "session_index.jsonl",
            &[
                json!({"id": "root", "thread_name": "Old name", "updated_at": "2026-08-13T12:00:00Z"}),
                json!({"id": "root", "thread_name": "Newest name", "updated_at": "2026-08-13T12:01:00Z"}),
            ],
        );
        home
    }

    #[test]
    fn aggregates_root_and_descendant_usage_with_latest_name() {
        let home = fixture_home();

        let report = build("root", home.path()).unwrap();

        assert_eq!(report.rollout.thread_name.as_deref(), Some("Newest name"));
        assert_eq!(report.rollout.total_subagent_spawns, 1);
        assert_eq!(report.tree.rollout_count, 2);
        assert_eq!(report.by_rollout_type["security_review"].rollout_count, 1);
        assert_eq!(report.by_model["gpt-5.6-terra"].input_tokens, 100);
    }

    #[test]
    fn unknown_pricing_keeps_known_cost_and_marks_tree_incomplete() {
        let home = fixture_home();

        let report = build("root", home.path()).unwrap();

        assert_eq!(report.tree.estimated_cost_usd, None);
        assert!(report.tree.known_model_cost_usd > 0.0);
        assert_eq!(report.tree.unpriced_models["unpriced-model"], 20);
    }

    #[test]
    fn unreadable_descendant_keeps_a_partial_tree_report() {
        let home = fixture_home();
        let index = RolloutIndex::build(home.path()).unwrap();
        fs::remove_file(home.path().join("archived_sessions/child.jsonl")).unwrap();

        let report =
            build_with_index("root", home.path(), &index, &Catalog::embedded().unwrap()).unwrap();

        assert_eq!(report.tree.rollout_count, 2);
        assert_eq!(report.tree.estimated_cost_usd, None);
        assert!(
            report
                .incomplete_input_warnings
                .iter()
                .any(|warning| warning.contains("descendant rollout child"))
        );
    }

    #[test]
    fn selected_file_disappearing_after_discovery_is_an_error() {
        let home = fixture_home();
        let index = RolloutIndex::build(home.path()).unwrap();
        fs::remove_file(home.path().join("sessions/root.jsonl")).unwrap();

        let error = build_with_index("root", home.path(), &index, &Catalog::embedded().unwrap())
            .unwrap_err();

        assert!(matches!(
            error,
            ReportError::SelectedRolloutUnreadable { .. }
        ));
    }

    #[test]
    fn malformed_session_index_omits_the_optional_name() {
        let home = fixture_home();
        fs::write(
            home.path().join("session_index.jsonl"),
            "{\"id\":\"root\",\"thread_name\":\"Valid name\",\"updated_at\":\"2026-08-13T12:00:00Z\"}\nnot json\n",
        )
        .unwrap();

        let report = build("root", home.path()).unwrap();

        assert_eq!(report.rollout.thread_name, None);
        assert!(
            report
                .incomplete_input_warnings
                .iter()
                .any(|warning| warning.contains("session index contains malformed"))
        );
    }

    #[test]
    fn index_records_without_a_name_do_not_hide_a_valid_name() {
        let home = fixture_home();
        fs::write(
            home.path().join("session_index.jsonl"),
            "{\"id\":\"root\",\"updated_at\":\"2026-08-13T12:00:00Z\"}\n{\"id\":\"root\",\"thread_name\":\"Valid name\",\"updated_at\":\"2026-08-13T12:01:00Z\"}\n",
        )
        .unwrap();

        let report = build("root", home.path()).unwrap();

        assert_eq!(report.rollout.thread_name.as_deref(), Some("Valid name"));
        assert!(report.incomplete_input_warnings.is_empty());
    }
}
