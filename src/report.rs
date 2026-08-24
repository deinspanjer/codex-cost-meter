use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    rc::Rc,
};

use serde::Serialize;
use thiserror::Error;

use crate::{
    cache::RolloutCache,
    pricing::{Catalog, PricingError, ServiceTier, Usage},
    progress::Progress,
    rollout::{
        analysis::{AnalysisError, RolloutStats, replay_prefix_len},
        discovery::RolloutIndex,
    },
    session_index::Snapshot,
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
pub(crate) struct ProjectReport {
    pub selection: ProjectSelection,
    pub tree: StatsReport,
    pub by_model: BTreeMap<String, ModelReport>,
    pub pricing: PricingReport,
    pub incomplete_input_warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProjectSelection {
    pub target: String,
    pub resolver: &'static str,
    pub missing_source_roots: usize,
    pub direct_assignments: usize,
    pub workspace_fallbacks: usize,
    pub projectless_threads: usize,
    pub projectless_exclusions: usize,
    pub other_project_exclusions: usize,
    pub incomplete_root_reports: usize,
    pub unpriced_root_reports: usize,
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
    pub unpriced_service_tiers: BTreeMap<String, u64>,
    pub assumed_standard_tokens: u64,
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
    pub total_turn_duration_seconds: f64,
    pub estimated_cost_usd: Option<f64>,
    pub known_model_cost_usd: f64,
    pub by_service_tier: BTreeMap<String, ModelTierReport>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModelTierReport {
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

pub(crate) struct ReportContext {
    codex_home: PathBuf,
    index: RolloutIndex,
    catalog: Catalog,
    session_index: Snapshot,
    cache: Rc<RolloutCache>,
}

impl ReportContext {
    #[cfg(test)]
    pub(crate) fn new(codex_home: &Path) -> Result<Self, PricingError> {
        Self::new_cached(codex_home, Rc::new(RolloutCache::open(codex_home, false)))
    }

    pub(crate) fn new_cached(
        codex_home: &Path,
        cache: Rc<RolloutCache>,
    ) -> Result<Self, PricingError> {
        Ok(Self {
            codex_home: codex_home.into(),
            index: RolloutIndex::build_cached(codex_home, &cache),
            catalog: Catalog::embedded()?,
            session_index: Snapshot::load(codex_home),
            cache,
        })
    }

    pub(crate) fn build(&self, thread_id: &str) -> Result<Report, ReportError> {
        self.build_inner(thread_id, None)
    }

    #[cfg(test)]
    pub(crate) fn new_for(codex_home: &Path, thread_ids: &[String]) -> Result<Self, PricingError> {
        Self::new_for_cached(
            codex_home,
            thread_ids,
            Rc::new(RolloutCache::open(codex_home, false)),
        )
    }

    pub(crate) fn new_for_cached(
        codex_home: &Path,
        thread_ids: &[String],
        cache: Rc<RolloutCache>,
    ) -> Result<Self, PricingError> {
        Ok(Self {
            codex_home: codex_home.into(),
            index: RolloutIndex::build_for_cached(codex_home, thread_ids, Some(&cache), || {}),
            catalog: Catalog::embedded()?,
            session_index: Snapshot::load(codex_home),
            cache,
        })
    }

    pub(crate) fn build_with_progress(
        &self,
        thread_id: &str,
        progress: &mut Progress,
    ) -> Result<Report, ReportError> {
        progress.start_analysis(self.rollout_count(thread_id));
        self.build_inner(thread_id, Some(progress))
    }

    fn build_inner(
        &self,
        thread_id: &str,
        progress: Option<&mut Progress>,
    ) -> Result<Report, ReportError> {
        build_with_state(
            thread_id,
            &self.codex_home,
            &self.index,
            &self.catalog,
            self.session_index(),
            &self.cache,
            progress,
        )
    }

    pub(crate) fn is_root(&self, thread_id: &str) -> bool {
        self.index.is_root(thread_id)
    }

    pub(crate) fn roots(&self) -> impl Iterator<Item = &crate::rollout::discovery::RolloutRecord> {
        self.index.roots()
    }

    fn rollout_count(&self, thread_id: &str) -> usize {
        self.index
            .record(thread_id)
            .map(|_| self.index.descendants(thread_id).map_or(0, |ids| ids.len()) + 1)
            .unwrap_or_default()
    }

    pub(crate) fn build_project(
        &self,
        selection: ProjectSelection,
        thread_ids: &[String],
    ) -> Result<ProjectReport, ReportError> {
        self.build_project_inner(selection, thread_ids, None)
    }

    pub(crate) fn build_project_with_progress(
        &self,
        selection: ProjectSelection,
        thread_ids: &[String],
        progress: &mut Progress,
    ) -> Result<ProjectReport, ReportError> {
        progress.start_analysis(thread_ids.iter().map(|id| self.rollout_count(id)).sum());
        self.build_project_inner(selection, thread_ids, Some(progress))
    }

    fn build_project_inner(
        &self,
        mut selection: ProjectSelection,
        thread_ids: &[String],
        mut progress: Option<&mut Progress>,
    ) -> Result<ProjectReport, ReportError> {
        let mut tree = empty_stats();
        let mut by_model = BTreeMap::new();
        let mut warnings = BTreeSet::new();
        for thread_id in thread_ids {
            let report = match self.build_inner(thread_id, progress.as_deref_mut()) {
                Ok(report) => report,
                Err(
                    ReportError::RolloutNotFound { .. }
                    | ReportError::SelectedRolloutUnreadable { .. },
                ) => {
                    selection.incomplete_root_reports += 1;
                    tree.incomplete_input = true;
                    warnings.insert("selected root rollout could not be read".into());
                    continue;
                }
                Err(error) => return Err(error),
            };
            selection.incomplete_root_reports += usize::from(report.tree.incomplete_input);
            selection.unpriced_root_reports += usize::from(
                !report.tree.unpriced_models.is_empty()
                    || !report.tree.unpriced_service_tiers.is_empty(),
            );
            merge_stats(&mut tree, &report.tree);
            for (name, model) in report.by_model {
                merge_model(by_model.entry(name).or_insert_with(empty_model), &model);
            }
            warnings.extend(report.incomplete_input_warnings);
        }
        Ok(ProjectReport {
            selection,
            tree,
            by_model,
            pricing: pricing_report(&self.catalog),
            incomplete_input_warnings: warnings.into_iter().collect(),
        })
    }

    pub(crate) fn session_index(&self) -> &Snapshot {
        &self.session_index
    }
}

impl ReportContext {
    pub(crate) fn new_cached_with_progress(
        codex_home: &Path,
        cache: Rc<RolloutCache>,
        progress: &mut Progress,
    ) -> Result<Self, PricingError> {
        progress.start_indexing();
        Ok(Self {
            codex_home: codex_home.into(),
            index: RolloutIndex::build_with_cache_progress(codex_home, Some(&cache), || {
                progress.indexed_file()
            }),
            catalog: Catalog::embedded()?,
            session_index: Snapshot::load(codex_home),
            cache,
        })
    }

    pub(crate) fn new_for_cached_with_progress(
        codex_home: &Path,
        thread_ids: &[String],
        cache: Rc<RolloutCache>,
        progress: &mut Progress,
    ) -> Result<Self, PricingError> {
        progress.start_indexing();
        Ok(Self {
            codex_home: codex_home.into(),
            index: RolloutIndex::build_for_cached(codex_home, thread_ids, Some(&cache), || {
                progress.indexed_file()
            }),
            catalog: Catalog::embedded()?,
            session_index: Snapshot::load(codex_home),
            cache,
        })
    }
}

fn empty_stats() -> StatsReport {
    StatsReport {
        rollout_count: 0,
        majority_turn_model: None,
        majority_reasoning_level: None,
        input_tokens: 0,
        input_cache_write_tokens: 0,
        input_cache_read_tokens: 0,
        reasoning_tokens: 0,
        output_tokens: 0,
        turns: 0,
        completed_or_aborted_turns: 0,
        incomplete_turns: 0,
        total_turn_duration_seconds: 0.0,
        estimated_cost_usd: Some(0.0),
        known_model_cost_usd: 0.0,
        unpriced_models: BTreeMap::new(),
        unpriced_service_tiers: BTreeMap::new(),
        assumed_standard_tokens: 0,
        unattributed_usage_tokens: None,
        incomplete_input: false,
    }
}

fn empty_model() -> ModelReport {
    ModelReport {
        turns: 0,
        input_tokens: 0,
        input_cache_write_tokens: 0,
        input_cache_read_tokens: 0,
        reasoning_tokens: 0,
        output_tokens: 0,
        total_turn_duration_seconds: 0.0,
        estimated_cost_usd: Some(0.0),
        known_model_cost_usd: 0.0,
        by_service_tier: BTreeMap::new(),
    }
}

fn merge_stats(total: &mut StatsReport, next: &StatsReport) {
    total.rollout_count += next.rollout_count;
    total.input_tokens += next.input_tokens;
    total.input_cache_write_tokens += next.input_cache_write_tokens;
    total.input_cache_read_tokens += next.input_cache_read_tokens;
    total.reasoning_tokens += next.reasoning_tokens;
    total.output_tokens += next.output_tokens;
    total.turns += next.turns;
    total.completed_or_aborted_turns += next.completed_or_aborted_turns;
    total.incomplete_turns += next.incomplete_turns;
    total.total_turn_duration_seconds += next.total_turn_duration_seconds;
    total.estimated_cost_usd = total
        .estimated_cost_usd
        .zip(next.estimated_cost_usd)
        .map(|(left, right)| left + right);
    total.known_model_cost_usd += next.known_model_cost_usd;
    for (model, tokens) in &next.unpriced_models {
        *total.unpriced_models.entry(model.clone()).or_default() += tokens;
    }
    for (tier, tokens) in &next.unpriced_service_tiers {
        *total
            .unpriced_service_tiers
            .entry(tier.clone())
            .or_default() += tokens;
    }
    total.assumed_standard_tokens = total
        .assumed_standard_tokens
        .saturating_add(next.assumed_standard_tokens);
    total.unattributed_usage_tokens = match (
        total.unattributed_usage_tokens,
        next.unattributed_usage_tokens,
    ) {
        (None, None) => None,
        (left, right) => Some(left.unwrap_or_default() + right.unwrap_or_default()),
    };
    total.incomplete_input |= next.incomplete_input;
}

fn merge_model(total: &mut ModelReport, next: &ModelReport) {
    total.turns += next.turns;
    total.input_tokens += next.input_tokens;
    total.input_cache_write_tokens += next.input_cache_write_tokens;
    total.input_cache_read_tokens += next.input_cache_read_tokens;
    total.reasoning_tokens += next.reasoning_tokens;
    total.output_tokens += next.output_tokens;
    total.total_turn_duration_seconds += next.total_turn_duration_seconds;
    total.estimated_cost_usd = total
        .estimated_cost_usd
        .zip(next.estimated_cost_usd)
        .map(|(left, right)| left + right);
    total.known_model_cost_usd += next.known_model_cost_usd;
    for (tier, next) in &next.by_service_tier {
        let total = total
            .by_service_tier
            .entry(tier.clone())
            .or_insert_with(empty_model_tier);
        total.input_tokens += next.input_tokens;
        total.input_cache_write_tokens += next.input_cache_write_tokens;
        total.input_cache_read_tokens += next.input_cache_read_tokens;
        total.reasoning_tokens += next.reasoning_tokens;
        total.output_tokens += next.output_tokens;
        total.estimated_cost_usd = total
            .estimated_cost_usd
            .zip(next.estimated_cost_usd)
            .map(|(left, right)| left + right);
        total.known_model_cost_usd += next.known_model_cost_usd;
    }
}

fn empty_model_tier() -> ModelTierReport {
    ModelTierReport {
        input_tokens: 0,
        input_cache_write_tokens: 0,
        input_cache_read_tokens: 0,
        reasoning_tokens: 0,
        output_tokens: 0,
        estimated_cost_usd: Some(0.0),
        known_model_cost_usd: 0.0,
    }
}

fn pricing_report(catalog: &Catalog) -> PricingReport {
    PricingReport {
        basis: "API list pricing; applied rollout tier (served tier unavailable); per request model/context; output includes reasoning",
        as_of: catalog.as_of().into(),
        source: catalog.source().into(),
        model_proxies: catalog
            .proxies()
            .iter()
            .map(|(model, target)| (model.clone(), target.clone()))
            .collect(),
    }
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
    unpriced_service_tiers: BTreeMap<String, u64>,
    assumed_standard_tokens: u64,
    unattributed_tokens: u64,
    incomplete: bool,
    models: BTreeMap<String, ModelAggregate>,
}

#[derive(Default)]
struct ModelAggregate {
    usage: Usage,
    reasoning_output: u64,
    turns: usize,
    duration_seconds: f64,
    known_cost: f64,
    incomplete: bool,
    by_service_tier: BTreeMap<String, ModelTierAggregate>,
}

#[derive(Default)]
struct ModelTierAggregate {
    usage: Usage,
    reasoning_output: u64,
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
        for (model, duration) in &stats.turn_durations {
            self.models
                .entry(model.clone())
                .or_default()
                .duration_seconds += duration.as_seconds_f64();
        }
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
            let tier = model
                .by_service_tier
                .entry(service_tier_key(&event.service_tier).into())
                .or_default();
            tier.usage.input = tier.usage.input.saturating_add(event.usage.input);
            tier.usage.cached_input = tier
                .usage
                .cached_input
                .saturating_add(event.usage.cached_input);
            tier.usage.cache_write_input = tier
                .usage
                .cache_write_input
                .saturating_add(event.usage.cache_write_input);
            tier.usage.output = tier.usage.output.saturating_add(event.usage.output);
            tier.reasoning_output = tier.reasoning_output.saturating_add(event.reasoning_output);

            let tokens = event.usage.input.saturating_add(event.usage.output);
            if matches!(event.service_tier, ServiceTier::AssumedStandard) {
                self.assumed_standard_tokens = self.assumed_standard_tokens.saturating_add(tokens);
            }
            if let ServiceTier::Unpriced(tier) = &event.service_tier {
                *self.unpriced_service_tiers.entry(tier.clone()).or_default() += tokens;
                model.incomplete = true;
                let tier = model
                    .by_service_tier
                    .get_mut(service_tier_key(&event.service_tier))
                    .expect("tier exists");
                tier.incomplete = true;
                self.incomplete = true;
                continue;
            }

            let cost = catalog.cost(&event.model, event.at, &event.service_tier, event.usage);
            self.known_cost += cost.known;
            model.known_cost += cost.known;
            let tier = model
                .by_service_tier
                .get_mut(service_tier_key(&event.service_tier))
                .expect("tier exists");
            tier.known_cost += cost.known;
            if cost.complete.is_none() {
                *self.unpriced_models.entry(event.model.clone()).or_default() += tokens;
                model.incomplete = true;
                tier.incomplete = true;
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
            unpriced_service_tiers: self.unpriced_service_tiers,
            assumed_standard_tokens: self.assumed_standard_tokens,
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
                        total_turn_duration_seconds: round(model.duration_seconds, 3),
                        estimated_cost_usd: (!model.incomplete).then(|| round(model.known_cost, 8)),
                        known_model_cost_usd: round(model.known_cost, 8),
                        by_service_tier: model
                            .by_service_tier
                            .iter()
                            .map(|(tier, detail)| {
                                (
                                    tier.clone(),
                                    ModelTierReport {
                                        input_tokens: detail.usage.input,
                                        input_cache_write_tokens: detail.usage.cache_write_input,
                                        input_cache_read_tokens: detail.usage.cached_input,
                                        reasoning_tokens: detail.reasoning_output,
                                        output_tokens: detail.usage.output,
                                        estimated_cost_usd: (!detail.incomplete)
                                            .then(|| round(detail.known_cost, 8)),
                                        known_model_cost_usd: round(detail.known_cost, 8),
                                    },
                                )
                            })
                            .collect(),
                    },
                )
            })
            .collect()
    }
}

fn service_tier_key(tier: &ServiceTier) -> &str {
    match tier {
        ServiceTier::Standard => "standard",
        ServiceTier::AssumedStandard => "assumed_standard",
        ServiceTier::Fast => "fast",
        ServiceTier::Unpriced(value) => value,
    }
}

#[cfg(test)]
pub(crate) fn build(thread_id: &str, codex_home: &Path) -> Result<Report, ReportError> {
    ReportContext::new_for(codex_home, &[thread_id.into()])?.build(thread_id)
}

pub(crate) fn build_with_progress(
    thread_id: &str,
    codex_home: &Path,
    progress: &mut Progress,
    cache: Rc<RolloutCache>,
) -> Result<Report, ReportError> {
    ReportContext::new_for_cached_with_progress(codex_home, &[thread_id.into()], cache, progress)?
        .build_with_progress(thread_id, progress)
}

#[cfg(test)]
fn build_with_index(
    thread_id: &str,
    codex_home: &Path,
    index: &RolloutIndex,
    catalog: &Catalog,
) -> Result<Report, ReportError> {
    build_with_state(
        thread_id,
        codex_home,
        index,
        catalog,
        &Snapshot::load(codex_home),
        &RolloutCache::open(codex_home, false),
        None,
    )
}

fn build_with_state(
    thread_id: &str,
    codex_home: &Path,
    index: &RolloutIndex,
    catalog: &Catalog,
    session_index: &Snapshot,
    cache: &RolloutCache,
    mut progress: Option<&mut Progress>,
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
        let replay_prefix_len = record
            .parent_id
            .as_deref()
            .and_then(|parent_id| index.record(parent_id))
            .map_or(0, |parent| replay_prefix_len(parent, record));
        let rollout_type = record.kind.report_type();
        let type_stats = by_rollout_type.entry(rollout_type).or_default();
        match cache.analyze_with_replay(record, replay_prefix_len) {
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
        if let Some(progress) = progress.as_deref_mut() {
            progress.analyzed_rollout();
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
            thread_name: session_index
                .is_complete()
                .then(|| {
                    session_index
                        .entry(thread_id)
                        .map(|entry| entry.name.clone())
                })
                .flatten(),
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
        pricing: pricing_report(catalog),
        incomplete_input_warnings: {
            if session_index.read_error().is_some() {
                warnings.push("session index could not be read".into());
            } else {
                if session_index.oversized_records() > 0 {
                    warnings.push("session index skipped oversized JSONL records".into());
                }
                if session_index.malformed_records() > 0 {
                    warnings.push("session index contains malformed JSONL records".into());
                }
            }
            warnings
        },
    })
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

    use super::{ProjectSelection, ReportContext, ReportError, build, build_with_index};
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
            json!({"type": "event_msg", "payload": {"type": "thread_settings_applied", "thread_settings": {"service_tier": "standard"}}}),
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

    #[cfg(unix)]
    struct PermissionsGuard {
        path: std::path::PathBuf,
        original: fs::Permissions,
    }

    #[cfg(unix)]
    impl PermissionsGuard {
        fn remove(path: &std::path::Path) -> Self {
            use std::os::unix::fs::PermissionsExt;

            let original = fs::metadata(path).unwrap().permissions();
            let mut denied = original.clone();
            denied.set_mode(0o000);
            fs::set_permissions(path, denied).unwrap();
            Self {
                path: path.to_path_buf(),
                original,
            }
        }
    }

    #[cfg(unix)]
    impl Drop for PermissionsGuard {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.path, self.original.clone());
        }
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
    fn excludes_copied_fork_usage_and_keeps_the_first_child_request() {
        let home = TempDir::new().unwrap();
        write_jsonl(
            &home,
            "sessions/root.jsonl",
            &[
                json!({"type": "session_meta", "timestamp": "2026-08-13T12:00:00Z", "payload": {"id": "root", "source": "cli"}}),
                json!({"type": "turn_context", "payload": {"turn_id": "parent-1", "model": "gpt-5.6-terra", "effort": "high"}}),
                json!({"type": "event_msg", "payload": {"type": "turn_started", "turn_id": "parent-1"}}),
                json!({"type": "event_msg", "timestamp": "2026-08-13T12:01:00Z", "payload": {"type": "token_count", "info": {
                    "last_token_usage": {"input_tokens": 100, "total_tokens": 100},
                    "total_token_usage": {"input_tokens": 100, "total_tokens": 100}
                }}}),
                json!({"type": "event_msg", "payload": {"type": "turn_complete", "turn_id": "parent-1"}}),
                json!({"type": "turn_context", "payload": {"turn_id": "parent-2", "model": "gpt-5.6-terra", "effort": "high"}}),
                json!({"type": "event_msg", "payload": {"type": "turn_started", "turn_id": "parent-2"}}),
                json!({"type": "event_msg", "timestamp": "2026-08-13T12:01:59.900Z", "payload": {"type": "token_count", "info": {
                    "last_token_usage": {"input_tokens": 100, "total_tokens": 100},
                    "total_token_usage": {"input_tokens": 200, "total_tokens": 200}
                }}}),
            ],
        );
        write_jsonl(
            &home,
            "sessions/child.jsonl",
            &[
                json!({"type": "session_meta", "timestamp": "2026-08-13T12:02:00Z", "payload": {
                    "id": "child", "forked_from_id": "root", "history_mode": "legacy", "source": "cli"
                }}),
                json!({"type": "session_meta", "timestamp": "2026-08-13T12:00:00Z", "payload": {"id": "root", "source": "cli"}}),
                json!({"type": "turn_context", "payload": {"turn_id": "parent-1", "model": "gpt-5.6-terra", "effort": "high"}}),
                json!({"type": "event_msg", "payload": {"type": "turn_started", "turn_id": "parent-1"}}),
                json!({"type": "event_msg", "timestamp": "2026-08-13T12:02:00.001Z", "payload": {"type": "token_count", "info": {
                    "last_token_usage": {"input_tokens": 100, "total_tokens": 100},
                    "total_token_usage": {"input_tokens": 100, "total_tokens": 100}
                }}}),
                json!({"type": "event_msg", "payload": {"type": "turn_complete", "turn_id": "parent-1"}}),
                json!({"type": "turn_context", "payload": {"turn_id": "child-1", "model": "gpt-5.6-sol", "effort": "high"}}),
                json!({"type": "event_msg", "payload": {"type": "turn_started", "turn_id": "child-1"}}),
                json!({"type": "event_msg", "timestamp": "2026-08-13T12:02:05Z", "payload": {"type": "token_count", "info": {
                    "last_token_usage": {
                        "input_tokens": 50, "cached_input_tokens": 20, "cache_write_input_tokens": 5,
                        "output_tokens": 10, "reasoning_output_tokens": 3, "total_tokens": 60
                    },
                    "total_token_usage": {
                        "input_tokens": 150, "cached_input_tokens": 20, "cache_write_input_tokens": 5,
                        "output_tokens": 10, "reasoning_output_tokens": 3, "total_tokens": 160
                    }
                }}}),
            ],
        );

        let report = build("root", home.path()).unwrap();

        assert_eq!(report.tree.input_tokens, 250);
        assert_eq!(report.tree.input_cache_read_tokens, 20);
        assert_eq!(report.tree.input_cache_write_tokens, 5);
        assert_eq!(report.tree.output_tokens, 10);
        assert_eq!(report.tree.reasoning_tokens, 3);
        assert_eq!(report.by_model["gpt-5.6-terra"].input_tokens, 200);
        assert_eq!(report.by_model["gpt-5.6-sol"].input_tokens, 50);

        let project = ReportContext::new(home.path())
            .unwrap()
            .build_project(
                ProjectSelection {
                    target: "fixture".into(),
                    resolver: "test",
                    missing_source_roots: 0,
                    direct_assignments: 1,
                    workspace_fallbacks: 0,
                    projectless_threads: 0,
                    projectless_exclusions: 0,
                    other_project_exclusions: 0,
                    incomplete_root_reports: 0,
                    unpriced_root_reports: 0,
                },
                &["root".into()],
            )
            .unwrap();
        assert_eq!(project.tree.input_tokens, 250);
        assert_eq!(project.by_model["gpt-5.6-sol"].input_tokens, 50);
    }

    #[test]
    fn attributes_completed_turn_duration_to_each_model() {
        let home = TempDir::new().unwrap();
        write_jsonl(
            &home,
            "sessions/root.jsonl",
            &[
                json!({
                    "type": "session_meta",
                    "timestamp": "2026-08-13T12:00:00Z",
                    "payload": {"id": "root", "source": "cli", "cwd": "/tmp/project"},
                }),
                json!({"type": "turn_context", "payload": {"turn_id": "terra", "model": "gpt-5.6-terra", "effort": "high"}}),
                json!({"type": "turn_context", "payload": {"turn_id": "sol", "model": "gpt-5.6-sol", "effort": "high"}}),
                json!({"type": "event_msg", "timestamp": "2026-08-13T12:00:01Z", "payload": {"type": "turn_started", "turn_id": "terra"}}),
                json!({"type": "event_msg", "timestamp": "2026-08-13T12:00:04Z", "payload": {"type": "turn_complete", "turn_id": "terra"}}),
                json!({"type": "event_msg", "timestamp": "2026-08-13T12:00:05Z", "payload": {"type": "turn_started", "turn_id": "sol"}}),
                json!({"type": "event_msg", "timestamp": "2026-08-13T12:00:07Z", "payload": {"type": "turn_aborted", "turn_id": "sol"}}),
            ],
        );

        let report = build("root", home.path()).unwrap();

        assert_eq!(report.tree.total_turn_duration_seconds, 5.0);
        assert_eq!(
            report.by_model["gpt-5.6-terra"].total_turn_duration_seconds,
            3.0
        );
        assert_eq!(
            report.by_model["gpt-5.6-sol"].total_turn_duration_seconds,
            2.0
        );
    }

    #[test]
    fn context_reuses_report_and_session_index_state() {
        let home = fixture_home();

        let context = ReportContext::new(home.path()).unwrap();

        assert!(context.is_root("root"));
        assert!(!context.is_root("child"));
        assert_eq!(
            context.session_index().entry("root").unwrap().name,
            "Newest name"
        );
        assert_eq!(context.build("root").unwrap().tree.rollout_count, 2);
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
    fn missing_and_unsupported_tiers_keep_only_known_cost() {
        let home = TempDir::new().unwrap();
        write_jsonl(
            &home,
            "sessions/root.jsonl",
            &[
                json!({
                    "type": "session_meta",
                    "timestamp": "2026-08-22T12:00:00Z",
                    "payload": {"id": "root", "source": "cli", "cwd": "/tmp/project"},
                }),
                json!({"type": "turn_context", "payload": {"model": "gpt-5.6-terra", "effort": "high"}}),
                json!({"type": "event_msg", "payload": {"type": "thread_settings_applied", "thread_settings": {"service_tier": "standard"}}}),
                json!({"type": "event_msg", "timestamp": "2026-08-22T12:00:01Z", "payload": {"type": "token_count", "info": {"last_token_usage": {"input_tokens": 100, "total_tokens": 100}}}}),
                json!({"type": "event_msg", "payload": {"type": "thread_settings_applied", "thread_settings": {"service_tier": "priority"}}}),
                json!({"type": "event_msg", "timestamp": "2026-08-22T12:00:02Z", "payload": {"type": "token_count", "info": {"last_token_usage": {"input_tokens": 100, "total_tokens": 100}}}}),
                json!({"type": "event_msg", "payload": {"type": "thread_settings_applied", "thread_settings": {}}}),
                json!({"type": "event_msg", "timestamp": "2026-08-22T12:00:03Z", "payload": {"type": "token_count", "info": {"last_token_usage": {"input_tokens": 100, "total_tokens": 100}}}}),
                json!({"type": "event_msg", "payload": {"type": "thread_settings_applied", "thread_settings": {"service_tier": "flex"}}}),
                json!({"type": "event_msg", "timestamp": "2026-08-22T12:00:04Z", "payload": {"type": "token_count", "info": {"last_token_usage": {"input_tokens": 100, "total_tokens": 100}}}}),
            ],
        );

        let report = build("root", home.path()).unwrap();

        assert!(report.tree.known_model_cost_usd > 0.0);
        assert_eq!(report.tree.estimated_cost_usd, None);
        assert_eq!(report.tree.unpriced_service_tiers["flex"], 100);
        assert_eq!(report.tree.assumed_standard_tokens, 100);
        assert!(report.tree.unpriced_models.is_empty());
        let model = &report.by_model["gpt-5.6-terra"];
        assert_eq!(model.input_tokens, 400);
        assert_eq!(model.by_service_tier["standard"].input_tokens, 100);
        assert_eq!(model.by_service_tier["fast"].input_tokens, 100);
        assert_eq!(model.by_service_tier["assumed_standard"].input_tokens, 100);
        assert_eq!(model.by_service_tier["flex"].input_tokens, 100);
        assert_eq!(
            model.by_service_tier["standard"].estimated_cost_usd,
            Some(0.0002)
        );
        assert_eq!(
            model.by_service_tier["fast"].estimated_cost_usd,
            Some(0.0004)
        );
        assert_eq!(
            model.by_service_tier["assumed_standard"].estimated_cost_usd,
            Some(0.0002)
        );
        assert_eq!(model.known_model_cost_usd, 0.0008);
        let tier_cost = model
            .by_service_tier
            .values()
            .map(|tier| tier.known_model_cost_usd)
            .sum::<f64>();
        assert!((tier_cost - model.known_model_cost_usd).abs() < 1e-12);
    }

    #[test]
    fn unreadable_descendant_keeps_a_partial_tree_report() {
        let home = fixture_home();
        let index = RolloutIndex::build(home.path());
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
        let index = RolloutIndex::build(home.path());
        fs::remove_file(home.path().join("sessions/root.jsonl")).unwrap();

        let error = build_with_index("root", home.path(), &index, &Catalog::embedded().unwrap())
            .unwrap_err();

        assert!(matches!(
            error,
            ReportError::SelectedRolloutUnreadable { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn selected_file_becoming_unreadable_after_discovery_is_an_error() {
        let home = fixture_home();
        let index = RolloutIndex::build(home.path());
        let selected = home.path().join("sessions/root.jsonl");
        let _guard = PermissionsGuard::remove(&selected);

        if fs::File::open(&selected).is_ok() {
            return;
        }
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
