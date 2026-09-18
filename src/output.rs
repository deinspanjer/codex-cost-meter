use std::{fmt::Write, path::Path};

use serde::Serialize;

use crate::{
    report::{ModelReport, ModelTierReport, PricingReport, ProjectReport, Report, StatsReport},
    title::{compact_tokens, strip_canonical_suffix},
};

pub(crate) fn human(report: &Report) -> String {
    let rollout = &report.rollout;
    let kind = safe_text(&rollout.rollout_type);
    let name = rollout
        .thread_name
        .as_deref()
        .map(strip_canonical_suffix)
        .filter(|name| !name.trim().is_empty())
        .map(safe_text);
    let identifier = short_identifier(&rollout.rollout_id);
    let heading = if kind == "root" {
        name.unwrap_or_else(|| "Unnamed task".into())
    } else {
        match name {
            Some(name) => format!("{name} · {}", display_kind(&kind)),
            None => display_kind(&kind),
        }
    };
    let project = rollout
        .project
        .as_deref()
        .map(project_display)
        .unwrap_or_else(|| "unknown project".into());
    let pair_label = if kind == "root" {
        "Most root turns"
    } else {
        "Most turns"
    };
    let model = rollout
        .stats
        .majority_turn_model
        .as_deref()
        .map(safe_text)
        .unwrap_or_else(|| "unknown".into());
    let effort = rollout
        .stats
        .majority_reasoning_level
        .as_deref()
        .map(safe_text)
        .unwrap_or_else(|| "unknown".into());
    let mut rendered = format!(
        "{heading} · {identifier}\n{project} · {} · {pair_label}: {model}/{effort}\n\n",
        counted(report.tree.rollout_count, "rollout", "rollouts")
    );

    let mut rows = vec![(
        if kind == "root" { "Root" } else { "Selected" },
        &rollout.stats,
    )];
    if report.tree.rollout_count > 1 {
        rows.push(("All agents", &report.tree));
    }
    rendered.push_str(&stats_table("Scope", rows));
    append_models(&mut rendered, &report.by_model);
    if report.tree.rollout_count > 1 {
        rendered.push_str("\nAll-agent turn time sums overlapping work.\n");
    } else {
        rendered.push('\n');
    }
    append_footer(
        &mut rendered,
        &report.pricing,
        &report.tree,
        &report.incomplete_input_warnings,
    );
    rendered
}

pub(crate) fn project_human(report: &ProjectReport) -> String {
    let selection = &report.selection;
    let corpus = selection.resolver == "corpus";
    let range = date_range(report);
    let mut rendered = if corpus {
        format!(
            "All Codex rollouts · {}\n{range}\n",
            counted(report.tree.rollout_count, "rollout", "rollouts")
        )
    } else {
        let tasks = report.by_rollout_type.get("root").map_or_else(
            || fallback_task_count(selection),
            |stats| stats.rollout_count,
        );
        format!(
            "{} · {} · {}\n{range}\n",
            safe_text(&selection.target),
            counted(tasks, "task", "tasks"),
            counted(report.tree.rollout_count, "rollout", "rollouts")
        )
    };

    if stats_are_empty(&report.tree) {
        rendered.push_str("No usage in the selected scope or range.\n");
        return rendered;
    }

    rendered.push('\n');
    rendered.push_str(&stats_table("Scope", [("Total", &report.tree)]));
    append_models(&mut rendered, &report.by_model);
    if !report.by_rollout_type.is_empty() {
        rendered.push_str("\nBy rollout type\n");
        rendered.push_str(&stats_table(
            "Type",
            report
                .by_rollout_type
                .iter()
                .map(|(kind, stats)| (kind.as_str(), stats)),
        ));
    }
    if !report.groups.is_empty() {
        rendered.push_str("\nBy group\n");
        rendered.push_str(&group_table(report));
    }
    rendered.push('\n');
    append_selection_notes(&mut rendered, selection, corpus);
    append_footer(
        &mut rendered,
        &report.pricing,
        &report.tree,
        &report.incomplete_input_warnings,
    );
    rendered
}

fn group_table(report: &ProjectReport) -> String {
    let mut table = vec![stats_headers("Group")];
    for group in &report.groups {
        let mut label = group.period.clone();
        if let Some(kind) = &group.rollout_type {
            let _ = write!(label, " / {}", safe_text(kind));
        }
        if let Some(model) = &group.model {
            let _ = write!(label, " / {}", safe_text(model));
        }
        table.push(stats_row(&label, &group.stats));
    }
    text_table(&table)
}

pub(crate) fn json(report: &impl Serialize) -> Result<String, serde_json::Error> {
    serde_json::to_string(report)
}

fn stats_table<'a>(
    label: &str,
    rows: impl IntoIterator<Item = (&'a str, &'a StatsReport)>,
) -> String {
    let mut table = vec![stats_headers(label)];
    table.extend(
        rows.into_iter()
            .map(|(scope, stats)| stats_row(scope, stats)),
    );
    text_table(&table)
}

fn append_models(
    rendered: &mut String,
    by_model: &std::collections::BTreeMap<String, ModelReport>,
) {
    if show_model_section(by_model) {
        rendered.push_str("\nBy model\n");
        rendered.push_str(&model_table_from_models(by_model));
    }
}

fn model_table_from_models(by_model: &std::collections::BTreeMap<String, ModelReport>) -> String {
    let mut models = by_model.iter().collect::<Vec<_>>();
    models.sort_by(|(left_name, left), (right_name, right)| {
        right
            .known_model_cost_usd
            .total_cmp(&left.known_model_cost_usd)
            .then_with(|| left_name.cmp(right_name))
    });
    let mut table = vec![model_headers()];
    for (name, model) in models {
        let modes = human_modes(model);
        let label = if let [mode] = modes.as_slice()
            && mode.label != "Standard"
        {
            format!("{} [{}]", safe_text(name), mode.label)
        } else {
            safe_text(name)
        };
        table.push(model_row(&label, model));
        if modes.len() > 1 {
            table.extend(
                modes
                    .iter()
                    .map(|mode| model_tier_row(&mode.label, &mode.detail)),
            );
        }
    }
    text_table(&table)
}

fn stats_headers(label: &str) -> Vec<String> {
    [
        label,
        "Turns",
        "Input (cached)",
        "Output (reasoning)",
        "Turn time",
        "Est. cost",
    ]
    .map(str::to_owned)
    .to_vec()
}

fn stats_row(scope: &str, stats: &StatsReport) -> Vec<String> {
    vec![
        safe_text(scope),
        human_turns(stats),
        nested_tokens(stats.input_tokens, stats.input_cache_read_tokens),
        nested_tokens(stats.output_tokens, stats.reasoning_tokens),
        human_duration(stats.total_turn_duration_seconds),
        human_cost(stats.estimated_cost_usd, stats.known_model_cost_usd),
    ]
}

fn model_headers() -> Vec<String> {
    [
        "Model",
        "Turns",
        "Input",
        "Output",
        "Turn time",
        "Est. cost",
    ]
    .map(str::to_owned)
    .to_vec()
}

fn model_row(model: &str, stats: &ModelReport) -> Vec<String> {
    vec![
        safe_text(model),
        human_number(stats.turns as u64),
        compact_tokens(stats.input_tokens),
        compact_tokens(stats.output_tokens),
        human_duration(stats.total_turn_duration_seconds),
        human_cost(stats.estimated_cost_usd, stats.known_model_cost_usd),
    ]
}

fn model_tier_row(label: &str, stats: &ModelTierReport) -> Vec<String> {
    vec![
        format!("↳ {label}"),
        String::new(),
        compact_tokens(stats.input_tokens),
        compact_tokens(stats.output_tokens),
        String::new(),
        human_cost(stats.estimated_cost_usd, stats.known_model_cost_usd),
    ]
}

struct HumanMode {
    label: String,
    detail: ModelTierReport,
}

fn human_modes(model: &ModelReport) -> Vec<HumanMode> {
    let mut modes = Vec::new();
    let standard = model.by_service_tier.get("standard");
    let assumed = model.by_service_tier.get("assumed_standard");
    if standard.is_some() || assumed.is_some() {
        modes.push(HumanMode {
            label: "Standard".into(),
            detail: merge_tiers([standard, assumed].into_iter().flatten()),
        });
    }
    if let Some(fast) = model.by_service_tier.get("fast") {
        modes.push(HumanMode {
            label: "Fast".into(),
            detail: merge_tiers([fast]),
        });
    }
    for (tier, detail) in &model.by_service_tier {
        if !matches!(tier.as_str(), "standard" | "assumed_standard" | "fast") {
            modes.push(HumanMode {
                label: format!("Tier unavailable ({})", safe_text(tier)),
                detail: merge_tiers([detail]),
            });
        }
    }
    modes
}

fn show_model_section(by_model: &std::collections::BTreeMap<String, ModelReport>) -> bool {
    if by_model.len() != 1 {
        return !by_model.is_empty();
    }
    let model = by_model.values().next().expect("one model");
    let modes = human_modes(model);
    modes.len() > 1 || modes.first().is_some_and(|mode| mode.label != "Standard")
}

fn merge_tiers<'a>(tiers: impl IntoIterator<Item = &'a ModelTierReport>) -> ModelTierReport {
    tiers.into_iter().fold(
        ModelTierReport {
            input_tokens: 0,
            input_cache_write_tokens: 0,
            input_cache_read_tokens: 0,
            reasoning_tokens: 0,
            output_tokens: 0,
            estimated_cost_usd: Some(0.0),
            known_model_cost_usd: 0.0,
        },
        |mut total, tier| {
            total.input_tokens = total.input_tokens.saturating_add(tier.input_tokens);
            total.input_cache_write_tokens = total
                .input_cache_write_tokens
                .saturating_add(tier.input_cache_write_tokens);
            total.input_cache_read_tokens = total
                .input_cache_read_tokens
                .saturating_add(tier.input_cache_read_tokens);
            total.reasoning_tokens = total.reasoning_tokens.saturating_add(tier.reasoning_tokens);
            total.output_tokens = total.output_tokens.saturating_add(tier.output_tokens);
            total.estimated_cost_usd = total
                .estimated_cost_usd
                .zip(tier.estimated_cost_usd)
                .map(|(left, right)| left + right);
            total.known_model_cost_usd += tier.known_model_cost_usd;
            total
        },
    )
}

fn human_turns(stats: &StatsReport) -> String {
    let turns = human_number(stats.turns as u64);
    if stats.incomplete_turns == 0 {
        turns
    } else {
        format!(
            "{turns} ({} incomplete)",
            human_number(stats.incomplete_turns as u64)
        )
    }
}

fn nested_tokens(total: u64, component: u64) -> String {
    format!("{} ({})", compact_tokens(total), compact_tokens(component))
}

fn short_identifier(value: &str) -> String {
    let safe = safe_text(value);
    safe.chars().take(8).collect()
}

fn project_display(value: &str) -> String {
    Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(safe_text)
        .unwrap_or_else(|| safe_text(value))
}

fn display_kind(value: &str) -> String {
    let value = value.replace('_', " ");
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => "Rollout".into(),
    }
}

fn counted(value: usize, singular: &str, plural: &str) -> String {
    format!(
        "{} {}",
        human_number(value as u64),
        if value == 1 { singular } else { plural }
    )
}

fn date_range(report: &ProjectReport) -> String {
    match (&report.date_range.since, &report.date_range.through) {
        (None, None) => "Lifetime".into(),
        (since, through) => format!(
            "{} through {}",
            since.as_deref().unwrap_or("unbounded"),
            through.as_deref().unwrap_or("unbounded")
        ),
    }
}

fn fallback_task_count(selection: &crate::report::ProjectSelection) -> usize {
    selection
        .direct_assignments
        .saturating_add(selection.workspace_fallbacks)
        .saturating_add(selection.projectless_threads)
}

fn stats_are_empty(stats: &StatsReport) -> bool {
    stats.turns == 0
        && stats.input_tokens == 0
        && stats.input_cache_write_tokens == 0
        && stats.input_cache_read_tokens == 0
        && stats.output_tokens == 0
        && stats.reasoning_tokens == 0
        && stats.known_model_cost_usd == 0.0
}

fn append_selection_notes(
    rendered: &mut String,
    selection: &crate::report::ProjectSelection,
    corpus: bool,
) {
    if corpus {
        return;
    }
    if selection.incomplete_root_reports > 0 || selection.unpriced_root_reports > 0 {
        let _ = writeln!(
            rendered,
            "Task trees: {} incomplete · {} with partial cost.",
            human_number(selection.incomplete_root_reports as u64),
            human_number(selection.unpriced_root_reports as u64)
        );
    }
    if selection.missing_source_roots > 0
        || selection.projectless_exclusions > 0
        || selection.other_project_exclusions > 0
    {
        let _ = writeln!(
            rendered,
            "Excluded: {} missing source · {} projectless · {} other project.",
            human_number(selection.missing_source_roots as u64),
            human_number(selection.projectless_exclusions as u64),
            human_number(selection.other_project_exclusions as u64)
        );
    }
}

fn append_footer(
    rendered: &mut String,
    pricing: &PricingReport,
    stats: &StatsReport,
    warnings: &[String],
) {
    let _ = writeln!(
        rendered,
        "Estimated API list cost using pricing dated {}.",
        safe_text(&pricing.as_of)
    );
    if stats.assumed_standard_tokens > 0 {
        let _ = writeln!(
            rendered,
            "Tier missing for {} tokens; Standard pricing assumed.",
            compact_tokens(stats.assumed_standard_tokens)
        );
    }
    if stats.estimated_cost_usd.is_none() {
        rendered.push_str("+ is the known priced minimum; the complete estimate is unavailable.\n");
    }
    if !warnings.is_empty() {
        rendered.push_str("Input warnings:\n");
        for warning in warnings {
            let _ = writeln!(rendered, "  - {}", safe_text(warning));
        }
    }
}

fn human_number(value: u64) -> String {
    let digits = value.to_string();
    let first = digits.len() % 3;
    let mut rendered = String::with_capacity(digits.len() + (digits.len() - 1) / 3);
    if first > 0 {
        rendered.push_str(&digits[..first]);
    }
    for index in (first..digits.len()).step_by(3) {
        if !rendered.is_empty() {
            rendered.push(',');
        }
        rendered.push_str(&digits[index..index + 3]);
    }
    rendered
}

fn human_duration(seconds: f64) -> String {
    let mut remaining = seconds;
    let days = (remaining / 86_400.0).floor() as u64;
    remaining -= days as f64 * 86_400.0;
    let hours = (remaining / 3_600.0).floor() as u64;
    remaining -= hours as f64 * 3_600.0;
    let minutes = (remaining / 60.0).floor() as u64;
    remaining -= minutes as f64 * 60.0;

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    if remaining > 0.0 || parts.is_empty() {
        parts.push(format!("{remaining:.1}s"));
    }
    parts.join(" ")
}

fn human_cost(estimated: Option<f64>, known: f64) -> String {
    match estimated {
        Some(cost) => format!("${cost:.2}"),
        None => format!("${known:.2}+"),
    }
}

fn safe_text(value: &str) -> String {
    value
        .split(|character: char| character.is_control() || character.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn text_table(rows: &[Vec<String>]) -> String {
    let widths = (0..rows.first().map_or(0, Vec::len))
        .map(|column| {
            rows.iter()
                .map(|row| cell_width(&row[column]))
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let line = row
                .iter()
                .enumerate()
                .map(|(column, cell)| {
                    format!(
                        "{cell}{}",
                        " ".repeat(widths[column].saturating_sub(cell_width(cell)))
                    )
                })
                .collect::<Vec<_>>()
                .join("  ");
            if index == 0 {
                format!(
                    "{line}\n{}",
                    widths
                        .iter()
                        .map(|width| "-".repeat(*width))
                        .collect::<Vec<_>>()
                        .join("  ")
                )
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn cell_width(value: &str) -> usize {
    value.chars().count()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json as json_value};

    use super::{human, json, project_human};
    use crate::report::{
        DateRangeReport, ModelProxyPointReport, ModelReport, ModelTierReport, PricingReport,
        ProjectReport, ProjectSelection, Report, RolloutReport, StatsReport,
    };

    fn stats(cost: Option<f64>, known_cost: f64) -> StatsReport {
        StatsReport {
            rollout_count: 1,
            majority_turn_model: Some("gpt-5.6-terra".into()),
            majority_reasoning_level: Some("high".into()),
            input_tokens: 12_000,
            input_cache_write_tokens: 500,
            input_cache_read_tokens: 2_000,
            reasoning_tokens: 1_000,
            output_tokens: 3_000,
            turns: 3,
            completed_or_aborted_turns: 2,
            incomplete_turns: 1,
            total_turn_duration_seconds: 65.25,
            estimated_cost_usd: cost,
            known_model_cost_usd: known_cost,
            unpriced_models: BTreeMap::new(),
            unpriced_service_tiers: BTreeMap::new(),
            assumed_standard_tokens: 990,
            unattributed_usage_tokens: None,
            incomplete_input: cost.is_none(),
        }
    }

    fn report() -> Report {
        let mut by_model = BTreeMap::new();
        by_model.insert(
            "cheap".into(),
            ModelReport {
                turns: 1,
                input_tokens: 10,
                input_cache_write_tokens: 0,
                input_cache_read_tokens: 0,
                reasoning_tokens: 0,
                output_tokens: 10,
                total_turn_duration_seconds: 10.0,
                estimated_cost_usd: Some(0.02),
                known_model_cost_usd: 0.02,
                by_service_tier: BTreeMap::new(),
            },
        );
        by_model.insert(
            "gpt-5.6-terra".into(),
            ModelReport {
                turns: 2,
                input_tokens: 11_990,
                input_cache_write_tokens: 500,
                input_cache_read_tokens: 2_000,
                reasoning_tokens: 1_000,
                output_tokens: 2_990,
                total_turn_duration_seconds: 55.25,
                estimated_cost_usd: Some(0.15),
                known_model_cost_usd: 0.15,
                by_service_tier: BTreeMap::from([
                    (
                        "standard".into(),
                        ModelTierReport {
                            input_tokens: 6_000,
                            input_cache_write_tokens: 250,
                            input_cache_read_tokens: 1_000,
                            reasoning_tokens: 500,
                            output_tokens: 1_495,
                            estimated_cost_usd: Some(0.04),
                            known_model_cost_usd: 0.04,
                        },
                    ),
                    (
                        "fast".into(),
                        ModelTierReport {
                            input_tokens: 5_000,
                            input_cache_write_tokens: 250,
                            input_cache_read_tokens: 1_000,
                            reasoning_tokens: 500,
                            output_tokens: 1_495,
                            estimated_cost_usd: Some(0.10),
                            known_model_cost_usd: 0.10,
                        },
                    ),
                    (
                        "assumed_standard".into(),
                        ModelTierReport {
                            input_tokens: 990,
                            input_cache_write_tokens: 0,
                            input_cache_read_tokens: 0,
                            reasoning_tokens: 0,
                            output_tokens: 0,
                            estimated_cost_usd: Some(0.01),
                            known_model_cost_usd: 0.01,
                        },
                    ),
                ]),
            },
        );
        let mut proxies = BTreeMap::new();
        proxies.insert("gpt-5.6".into(), "gpt-5.6-terra".into());
        proxies.insert("codex-auto-review".into(), "gpt-5.6-luna".into());
        let model_proxy_histories = BTreeMap::from([
            (
                "gpt-5.6".into(),
                vec![ModelProxyPointReport {
                    target: "gpt-5.6-terra".into(),
                    effective_from: None,
                }],
            ),
            (
                "codex-auto-review".into(),
                vec![
                    ModelProxyPointReport {
                        target: "gpt-5.4".into(),
                        effective_from: None,
                    },
                    ModelProxyPointReport {
                        target: "gpt-5.6-luna".into(),
                        effective_from: Some("2026-07-30".into()),
                    },
                ],
            ),
        ]);
        let mut tree = stats(None, 0.17);
        tree.rollout_count = 2;
        Report {
            rollout: RolloutReport {
                rollout_id: "root".into(),
                rollout_type: "root".into(),
                project: Some("/tmp/project".into()),
                thread_name: Some("Rollout stats".into()),
                total_subagent_spawns: 1,
                total_subagent_turn_duration_seconds: 10.0,
                stats: stats(Some(0.17), 0.17),
            },
            tree,
            by_model,
            by_rollout_type: BTreeMap::new(),
            pricing: PricingReport {
                basis: "API list pricing; applied rollout tier (served tier unavailable); per request model/context; output includes reasoning",
                as_of: "2026-08-13".into(),
                source: "https://example.invalid/prices".into(),
                model_proxies: proxies,
                model_proxy_histories,
            },
            incomplete_input_warnings: vec!["some usage could not be priced".into()],
        }
    }

    #[test]
    fn human_renders_scopes_models_pricing_and_partial_costs() {
        let rendered = human(&report());

        assert!(rendered.starts_with(
            "Rollout stats · root\nproject · 2 rollouts · Most root turns: gpt-5.6-terra/high\n"
        ));
        assert!(rendered.contains("Root"));
        assert!(rendered.contains("All agents"));
        assert!(rendered.contains("12K (2K)"));
        assert!(rendered.contains("3K (1K)"));
        assert!(rendered.contains("$0.17+"));
        assert!(rendered.contains("gpt-5.6-terra"));
        assert!(rendered.contains("↳ Standard"));
        assert!(rendered.contains("↳ Fast"));
        assert!(rendered.contains("Tier missing for 990 tokens; Standard pricing assumed."));
        let models = rendered.split_once("By model\n").unwrap().1;
        assert!(models.find("gpt-5.6-terra").unwrap() < models.find("cheap").unwrap());
        assert!(!models.lines().any(|line| line.starts_with("Total")));
        assert!(!rendered.contains("Pricing sources:"));
        assert!(!rendered.contains("Model proxies:"));
        assert!(rendered.contains("Estimated API list cost using pricing dated 2026-08-13."));
        assert!(rendered.contains("All-agent turn time sums overlapping work."));
    }

    #[test]
    fn human_collapses_single_service_modes_but_json_keeps_tiers_separate() {
        let mut report = report();
        let model = report.by_model.get_mut("gpt-5.6-terra").unwrap();
        model.by_service_tier.remove("fast");

        let rendered = human(&report);
        assert!(rendered.contains("gpt-5.6-terra"));
        assert!(!rendered.contains("[Standard"));
        assert!(!rendered.contains("↳ Standard"));
        let structured: Value = serde_json::from_str(&json(&report).unwrap()).unwrap();
        assert!(structured["by_model"]["gpt-5.6-terra"]["by_service_tier"]["standard"].is_object());
        assert!(
            structured["by_model"]["gpt-5.6-terra"]["by_service_tier"]["assumed_standard"]
                .is_object()
        );

        report.by_model.remove("cheap");
        assert!(!human(&report).contains("By model"));

        let model = report.by_model.get_mut("gpt-5.6-terra").unwrap();
        let fast = model.by_service_tier.remove("standard").unwrap();
        model.by_service_tier.clear();
        model.by_service_tier.insert("fast".into(), fast);
        let rendered = human(&report);
        assert!(rendered.contains("gpt-5.6-terra [Fast]"));
        assert!(rendered.contains("By model"));
        assert!(!rendered.contains("↳ Fast"));
    }

    #[test]
    fn human_mixed_modes_align_and_bound_pricing_provenance() {
        let mut report = report();
        report.pricing.source =
            "https://developers.openai.com/api/docs/pricing, https://openai.com/api-fast-mode/"
                .into();
        let rendered = human(&report);
        let model = rendered
            .lines()
            .find(|line| line.starts_with("gpt-5.6-terra "))
            .unwrap();
        let standard = rendered
            .lines()
            .find(|line| line.starts_with("↳ Standard"))
            .unwrap();
        let fast = rendered
            .lines()
            .find(|line| line.starts_with("↳ Fast"))
            .unwrap();
        let column = |line: &str, value: &str| line[..line.find(value).unwrap()].chars().count();
        assert_eq!(column(model, "12K"), column(standard, "7K"));
        assert_eq!(column(model, "12K"), column(fast, "5K"));
        assert!(!rendered.contains("Pricing sources:"));
    }

    #[test]
    fn human_preserves_unavailable_mode_in_mixed_output() {
        let mut report = report();
        let model = report.by_model.get_mut("gpt-5.6-terra").unwrap();
        model.by_service_tier.remove("fast");
        let unavailable = model.by_service_tier.remove("standard").unwrap();
        model.by_service_tier.insert("flex".into(), unavailable);

        let rendered = human(&report);
        assert!(rendered.contains("↳ Standard"));
        assert!(rendered.contains("↳ Tier unavailable (flex)"));
    }

    #[test]
    fn redundant_and_empty_model_sections_are_omitted() {
        let mut report = report();
        report.by_model.clear();
        let rendered = human(&report);
        assert!(!rendered.contains("By model"));

        let mut project = ProjectReport {
            selection: ProjectSelection {
                target: "Project".into(),
                resolver: "project_name",
                missing_source_roots: 0,
                direct_assignments: 0,
                workspace_fallbacks: 0,
                projectless_threads: 0,
                projectless_exclusions: 0,
                other_project_exclusions: 0,
                incomplete_root_reports: 0,
                unpriced_root_reports: 0,
            },
            date_range: DateRangeReport {
                since: None,
                through: None,
                group_by: Vec::new(),
            },
            tree: report.tree,
            by_model: BTreeMap::new(),
            by_rollout_type: BTreeMap::new(),
            groups: Vec::new(),
            pricing: report.pricing,
            incomplete_input_warnings: Vec::new(),
        };
        let rendered = project_human(&project);
        assert!(!rendered.contains("By model"));
        project.tree.turns = 0;
        project.tree.input_tokens = 0;
        project.tree.input_cache_write_tokens = 0;
        project.tree.input_cache_read_tokens = 0;
        project.tree.output_tokens = 0;
        project.tree.reasoning_tokens = 0;
        project.tree.known_model_cost_usd = 0.0;
        project.selection.resolver = "corpus";
        let empty = project_human(&project);
        assert!(empty.contains("No usage in the selected scope or range."));
        assert!(!empty.contains("Scope"));
        assert!(!empty.contains("Estimated API"));
    }

    #[test]
    fn human_sanitizes_untrusted_single_line_labels() {
        let mut report = report();
        let forged = "\x1b[31m\nforged\r".to_owned();
        report.rollout.rollout_id = forged.clone();
        report.rollout.project = Some(forged.clone());
        report.rollout.thread_name = Some(forged.clone());
        report.by_model.insert(
            forged,
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
            },
        );

        let rendered = human(&report);

        assert!(!rendered.contains('\x1b'));
        assert!(!rendered.contains('\r'));
        assert!(!rendered.contains("\nforged"));
    }

    #[test]
    fn human_cleans_task_identity_and_keeps_noncanonical_titles() {
        let mut report = report();
        report.rollout.rollout_id = "01a0b189-574f-7a21-8de4-546018584e81".into();
        report.rollout.project = Some("/tmp/work/platform-operator-console".into());
        report.rollout.thread_name = Some("Fix scrolling · $332.50 · ⇥361.2M · ↦783.6K".into());

        let rendered = human(&report);

        assert!(rendered.starts_with(
            "Fix scrolling · 01a0b189\nplatform-operator-console · 2 rollouts · Most root turns:"
        ));
        assert!(!rendered.contains("$332.50"));

        report.rollout.thread_name = Some("Keep this · $332.5".into());
        assert!(human(&report).starts_with("Keep this · $332.5 · 01a0b189"));

        report.rollout.thread_name = None;
        assert!(human(&report).starts_with("Unnamed task · 01a0b189"));
    }

    #[test]
    fn human_labels_direct_non_root_reports() {
        let mut report = report();
        report.rollout.rollout_type = "subagent".into();
        report.rollout.rollout_id = "short-id".into();
        report.rollout.thread_name = None;
        report.rollout.total_subagent_spawns = 0;
        report.tree = stats(Some(0.17), 0.17);
        report.tree.rollout_count = 1;

        let rendered = human(&report);

        assert!(rendered.starts_with(
            "Subagent · short-id\nproject · 1 rollout · Most turns: gpt-5.6-terra/high"
        ));
        assert!(rendered.contains("Selected"));
        assert!(!rendered.contains("All agents"));
    }

    #[test]
    fn human_uses_nested_token_columns() {
        let mut report = report();
        report.rollout.stats.input_cache_write_tokens = 0;
        report.tree.input_cache_write_tokens = 0;
        for model in report.by_model.values_mut() {
            model.input_cache_write_tokens = 0;
        }

        let rendered = human(&report);
        assert!(rendered.contains("Input (cached)"));
        assert!(rendered.contains("Output (reasoning)"));
        assert!(!rendered.contains("Cache write"));
    }

    #[test]
    fn human_turns_only_calls_out_incomplete_work() {
        let mut stats = stats(Some(0.17), 0.17);
        assert_eq!(super::human_turns(&stats), "3 (1 incomplete)");
        stats.completed_or_aborted_turns = 3;
        stats.incomplete_turns = 0;
        assert_eq!(super::human_turns(&stats), "3");
    }

    #[test]
    fn human_duration_uses_days_hours_and_nonzero_portions() {
        assert_eq!(super::human_duration(0.5), "0.5s");
        assert_eq!(super::human_duration(65.25), "1m 5.2s");
        assert_eq!(super::human_duration(111_349.2), "1d 6h 55m 49.2s");
    }

    #[test]
    fn project_human_keeps_material_qualifications_visible() {
        let report = report();
        let mut project = ProjectReport {
            selection: ProjectSelection {
                target: "Project".into(),
                resolver: "project_name",
                missing_source_roots: 0,
                direct_assignments: 1,
                workspace_fallbacks: 0,
                projectless_threads: 0,
                projectless_exclusions: 0,
                other_project_exclusions: 0,
                incomplete_root_reports: 1,
                unpriced_root_reports: 0,
            },
            date_range: DateRangeReport {
                since: Some("2026-08-01".into()),
                through: Some("2026-08-31".into()),
                group_by: Vec::new(),
            },
            tree: report.tree,
            by_model: report.by_model,
            by_rollout_type: BTreeMap::from([("root".into(), {
                let mut roots = stats(Some(0.17), 0.17);
                roots.rollout_count = 1;
                roots
            })]),
            groups: Vec::new(),
            pricing: report.pricing,
            incomplete_input_warnings: vec!["some input was incomplete".into()],
        };
        project.tree.input_tokens = 15_133_186_105;
        project.tree.turns = 8_458;
        project.tree.completed_or_aborted_turns = 8_189;
        project.tree.incomplete_turns = 269;
        project.selection.incomplete_root_reports = 12;

        let rendered = project_human(&project);

        assert!(
            rendered.starts_with("Project · 1 task · 2 rollouts\n2026-08-01 through 2026-08-31")
        );
        assert!(rendered.contains("Input warnings:\n  - some input was incomplete"));
        assert!(rendered.contains("15.1B"));
        assert!(rendered.contains("269 incomplete"));
        assert!(rendered.contains("Task trees: 12 incomplete · 0 with partial cost."));
        assert!(rendered.contains("+ is the known priced minimum"));
        assert!(!rendered.contains("Model proxies:"));

        project.selection.resolver = "corpus";
        let corpus = project_human(&project);
        assert!(corpus.starts_with("All Codex rollouts"));
        assert!(!corpus.contains("Most root turns"));
    }

    #[test]
    fn json_preserves_structured_report_values() {
        let rendered = json(&report()).unwrap();
        let actual: Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(actual["rollout"]["rollout_id"], json_value!("root"));
        assert_eq!(actual["tree"]["estimated_cost_usd"], Value::Null);
        assert_eq!(
            actual["by_model"]["gpt-5.6-terra"]["known_model_cost_usd"],
            json_value!(0.15)
        );
        assert_eq!(actual["pricing"]["as_of"], json_value!("2026-08-13"));
        assert_eq!(
            actual["pricing"]["model_proxies"]["codex-auto-review"],
            json_value!("gpt-5.6-luna")
        );
        assert_eq!(
            actual["pricing"]["model_proxy_histories"]["codex-auto-review"][0],
            json_value!({"target": "gpt-5.4"})
        );
        assert_eq!(
            actual["pricing"]["model_proxy_histories"]["codex-auto-review"][1],
            json_value!({"target": "gpt-5.6-luna", "effective_from": "2026-07-30"})
        );
    }
}
