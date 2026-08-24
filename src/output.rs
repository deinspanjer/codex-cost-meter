use std::fmt::Write;

use serde::Serialize;

use crate::{
    report::{ModelReport, ModelTierReport, PricingReport, ProjectReport, Report, StatsReport},
    title::compact_tokens,
};

pub(crate) fn human(report: &Report) -> String {
    let rollout = &report.rollout;
    let primary_model = rollout
        .stats
        .majority_turn_model
        .as_deref()
        .map(safe_text)
        .unwrap_or_else(|| "unknown".into());
    let primary_effort = rollout
        .stats
        .majority_reasoning_level
        .as_deref()
        .map(safe_text)
        .unwrap_or_else(|| "unknown".into());
    let mut rendered = format!(
        "Codex rollout {}\nProject: {}\nName: {}\nType: {}   Primary: {} / {}   Descendants: {}\n",
        safe_text(&rollout.rollout_id),
        rollout
            .project
            .as_deref()
            .map(safe_text)
            .unwrap_or_else(|| "unknown".into()),
        rollout
            .thread_name
            .as_deref()
            .map(safe_text)
            .unwrap_or_else(|| "unnamed".into()),
        safe_text(&rollout.rollout_type),
        primary_model,
        primary_effort,
        human_number(rollout.total_subagent_spawns as u64),
    );
    let show_cache_write = report.tree.input_cache_write_tokens > 0
        || rollout.stats.input_cache_write_tokens > 0
        || report
            .by_model
            .values()
            .any(|model| model.input_cache_write_tokens > 0);

    rendered.push_str("\nScope\n");
    rendered.push_str(&stats_table(
        show_cache_write,
        [("Root", &rollout.stats), ("Whole tree", &report.tree)],
    ));

    append_models(
        &mut rendered,
        &report.by_model,
        &report.tree,
        show_cache_write,
    );

    let _ = writeln!(
        rendered,
        "\nAgent-turn time: {} (agent time can overlap).",
        human_duration(rollout.total_subagent_turn_duration_seconds)
    );
    append_pricing(&mut rendered, &report.pricing);
    if !report.incomplete_input_warnings.is_empty() {
        rendered.push_str("Incomplete input:\n");
        for warning in &report.incomplete_input_warnings {
            let _ = writeln!(rendered, "  - {}", safe_text(warning));
        }
    }
    append_cost_note(&mut rendered, &report.tree);
    rendered.push_str(
        "Notes: cache read is included in input; reasoning is included in output; agent time can overlap.\n",
    );
    rendered
}

pub(crate) fn project_human(report: &ProjectReport) -> String {
    let selection = &report.selection;
    let show_cache_write = report.tree.input_cache_write_tokens > 0
        || report
            .by_model
            .values()
            .any(|model| model.input_cache_write_tokens > 0)
        || report
            .groups
            .iter()
            .any(|group| group.stats.input_cache_write_tokens > 0);
    let title = if selection.resolver == "corpus" {
        "Codex corpus report"
    } else {
        "Codex project report"
    };
    let mut rendered = if selection.resolver == "corpus" {
        format!(
            "{}\nScope: {}\nRollouts: {}\n",
            title,
            safe_text(&selection.target),
            human_number(report.tree.rollout_count as u64),
        )
    } else {
        format!(
            "{}\nScope: {}\nResolver: {}\nThreads: {} direct, {} workspace fallback, {} projectless, {} projectless excluded, {} other-project excluded\n",
            title,
            safe_text(&selection.target),
            safe_text(selection.resolver),
            human_number(selection.direct_assignments as u64),
            human_number(selection.workspace_fallbacks as u64),
            human_number(selection.projectless_threads as u64),
            human_number(selection.projectless_exclusions as u64),
            human_number(selection.other_project_exclusions as u64),
        )
    };
    let range = match (&report.date_range.since, &report.date_range.through) {
        (None, None) => "Lifetime".into(),
        (since, through) => format!(
            "Selected range ({} through {})",
            since.as_deref().unwrap_or("unbounded"),
            through.as_deref().unwrap_or("unbounded")
        ),
    };
    let scope_label = if selection.resolver == "corpus" {
        "Corpus"
    } else {
        "Project"
    };
    let _ = writeln!(rendered, "\n{range}");
    rendered.push_str(&stats_table(
        show_cache_write,
        [(scope_label, &report.tree)],
    ));
    append_models(
        &mut rendered,
        &report.by_model,
        &report.tree,
        show_cache_write,
    );
    if !report.by_rollout_type.is_empty() {
        rendered.push_str("\nRollout types\n");
        rendered.push_str(&stats_table(
            show_cache_write,
            report
                .by_rollout_type
                .iter()
                .map(|(kind, stats)| (kind.as_str(), stats)),
        ));
    }
    if !report.groups.is_empty() {
        rendered.push_str("\nGroups\n");
        rendered.push_str(&group_table(report, show_cache_write));
    }
    rendered.push('\n');
    append_pricing(&mut rendered, &report.pricing);
    if selection.resolver != "corpus"
        && (selection.incomplete_root_reports > 0 || selection.unpriced_root_reports > 0)
    {
        let _ = writeln!(
            rendered,
            "Incomplete root reports: {}   Unpriced root reports: {}",
            human_number(selection.incomplete_root_reports as u64),
            human_number(selection.unpriced_root_reports as u64),
        );
        let _ = writeln!(
            rendered,
            "An incomplete root report has incomplete input in it or a descendant; turn counts are shown in Lifetime."
        );
    }
    if !report.incomplete_input_warnings.is_empty() {
        rendered.push_str("Input warnings:\n");
        for warning in &report.incomplete_input_warnings {
            let _ = writeln!(rendered, "  - {}", safe_text(warning));
        }
    }
    append_cost_note(&mut rendered, &report.tree);
    rendered
        .push_str("Notes: model and aggregate durations are agent-turn time and can overlap.\n");
    rendered
}

const HUMAN_LINE_WIDTH: usize = 116;

fn append_pricing(rendered: &mut String, pricing: &PricingReport) {
    let _ = writeln!(rendered, "Pricing as of: {}", safe_text(&pricing.as_of));
    append_wrapped_field(rendered, "Pricing basis: ", &safe_text(pricing.basis));
    rendered.push_str("Pricing sources:\n");
    for source in pricing.source.split(',') {
        append_wrapped_field(rendered, "  - ", &safe_text(source));
    }
    append_model_proxies(rendered, pricing);
}

fn append_wrapped_field(rendered: &mut String, prefix: &str, value: &str) {
    let content_width = HUMAN_LINE_WIDTH.saturating_sub(prefix.chars().count());
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in value.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > content_width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    for (index, line) in lines.into_iter().enumerate() {
        let label = if index == 0 {
            prefix.to_owned()
        } else {
            " ".repeat(prefix.chars().count())
        };
        let _ = writeln!(rendered, "{label}{line}");
    }
}

fn append_model_proxies(rendered: &mut String, pricing: &PricingReport) {
    if pricing.model_proxy_histories.is_empty() {
        return;
    }
    rendered.push_str("Model proxies:\n");
    for (model, points) in &pricing.model_proxy_histories {
        if let [point] = points.as_slice()
            && point.effective_from.is_none()
        {
            let _ = writeln!(
                rendered,
                "  {} -> {}",
                safe_text(model),
                safe_text(&point.target)
            );
            continue;
        }
        for (index, point) in points.iter().enumerate() {
            match &point.effective_from {
                Some(date) => {
                    let _ = writeln!(
                        rendered,
                        "  {} from {} -> {}",
                        safe_text(model),
                        safe_text(date),
                        safe_text(&point.target)
                    );
                }
                None => {
                    let next_date = points
                        .get(index + 1)
                        .and_then(|next| next.effective_from.as_deref())
                        .unwrap_or("earliest dated change");
                    let _ = writeln!(
                        rendered,
                        "  {} before {} -> {}",
                        safe_text(model),
                        safe_text(next_date),
                        safe_text(&point.target)
                    );
                }
            }
        }
        if model == "codex-auto-review" {
            append_wrapped_field(
                rendered,
                "  Note: ",
                "codex-auto-review boundaries are announcement-date estimates, not observed routing or billing cutovers.",
            );
        }
    }
}

fn group_table(report: &ProjectReport, show_cache_write: bool) -> String {
    let mut table = vec![stats_headers("Group", show_cache_write)];
    for group in &report.groups {
        let mut label = group.period.clone();
        if let Some(kind) = &group.rollout_type {
            let _ = write!(label, " / {}", safe_text(kind));
        }
        if let Some(model) = &group.model {
            let _ = write!(label, " / {}", safe_text(model));
        }
        table.push(stats_row(&label, &group.stats, show_cache_write));
    }
    text_table(&table)
}

pub(crate) fn json(report: &impl Serialize) -> Result<String, serde_json::Error> {
    serde_json::to_string(report)
}

fn stats_table<'a>(
    show_cache_write: bool,
    rows: impl IntoIterator<Item = (&'a str, &'a StatsReport)>,
) -> String {
    let mut table = vec![stats_headers("Scope", show_cache_write)];
    table.extend(
        rows.into_iter()
            .map(|(scope, stats)| stats_row(scope, stats, show_cache_write)),
    );
    text_table(&table)
}

fn append_models(
    rendered: &mut String,
    by_model: &std::collections::BTreeMap<String, ModelReport>,
    total: &StatsReport,
    show_cache_write: bool,
) {
    rendered.push_str("\nModels\n");
    if by_model.is_empty() {
        rendered.push_str("No model usage.\n");
    } else {
        rendered.push_str(&model_table_from_models(by_model, total, show_cache_write));
    }
}

fn model_table_from_models(
    by_model: &std::collections::BTreeMap<String, ModelReport>,
    total: &StatsReport,
    show_cache_write: bool,
) -> String {
    let mut models = by_model.iter().collect::<Vec<_>>();
    models.sort_by(|(left_name, left), (right_name, right)| {
        right
            .known_model_cost_usd
            .total_cmp(&left.known_model_cost_usd)
            .then_with(|| left_name.cmp(right_name))
    });
    let mut table = vec![stats_headers("Model", show_cache_write)];
    for (name, model) in models {
        let modes = human_modes(model);
        let label = if let [mode] = modes.as_slice() {
            format!("{} [{}]", safe_text(name), mode.label)
        } else {
            safe_text(name)
        };
        table.push(model_row(&label, model, show_cache_write));
        if modes.len() > 1 {
            table.extend(
                modes
                    .iter()
                    .map(|mode| model_tier_row(&mode.label, &mode.detail, show_cache_write)),
            );
        }
    }
    table.push(stats_row("Total", total, show_cache_write));
    text_table(&table)
}

fn stats_headers(label: &str, show_cache_write: bool) -> Vec<String> {
    let mut headers = vec![label, "Turns", "Input", "Cache read"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if show_cache_write {
        headers.push("Cache write".into());
    }
    headers.extend(["Output", "Reasoning", "Duration", "Cost"].map(str::to_owned));
    headers
}

fn stats_row(scope: &str, stats: &StatsReport, show_cache_write: bool) -> Vec<String> {
    let mut row = vec![
        safe_text(scope),
        format!(
            "{} ({} complete, {} incomplete)",
            human_number(stats.turns as u64),
            human_number(stats.completed_or_aborted_turns as u64),
            human_number(stats.incomplete_turns as u64),
        ),
        compact_tokens(stats.input_tokens),
        compact_tokens(stats.input_cache_read_tokens),
    ];
    if show_cache_write {
        row.push(compact_tokens(stats.input_cache_write_tokens));
    }
    row.extend([
        compact_tokens(stats.output_tokens),
        compact_tokens(stats.reasoning_tokens),
        human_duration(stats.total_turn_duration_seconds),
        human_cost(stats.estimated_cost_usd, stats.known_model_cost_usd),
    ]);
    row
}

fn model_row(model: &str, stats: &ModelReport, show_cache_write: bool) -> Vec<String> {
    let mut row = vec![
        safe_text(model),
        human_number(stats.turns as u64),
        compact_tokens(stats.input_tokens),
        compact_tokens(stats.input_cache_read_tokens),
    ];
    if show_cache_write {
        row.push(compact_tokens(stats.input_cache_write_tokens));
    }
    row.extend([
        compact_tokens(stats.output_tokens),
        compact_tokens(stats.reasoning_tokens),
        human_duration(stats.total_turn_duration_seconds),
        human_cost(stats.estimated_cost_usd, stats.known_model_cost_usd),
    ]);
    row
}

fn model_tier_row(label: &str, stats: &ModelTierReport, show_cache_write: bool) -> Vec<String> {
    let mut row = vec![
        format!("↳ {label}"),
        String::new(),
        compact_tokens(stats.input_tokens),
        compact_tokens(stats.input_cache_read_tokens),
    ];
    if show_cache_write {
        row.push(compact_tokens(stats.input_cache_write_tokens));
    }
    row.extend([
        compact_tokens(stats.output_tokens),
        compact_tokens(stats.reasoning_tokens),
        String::new(),
        human_cost(stats.estimated_cost_usd, stats.known_model_cost_usd),
    ]);
    row
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
        let assumed_usage = assumed.is_some_and(tier_has_usage);
        modes.push(HumanMode {
            label: if assumed_usage {
                "Standard*"
            } else {
                "Standard"
            }
            .into(),
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

fn tier_has_usage(tier: &ModelTierReport) -> bool {
    tier.input_tokens > 0
        || tier.input_cache_write_tokens > 0
        || tier.input_cache_read_tokens > 0
        || tier.reasoning_tokens > 0
        || tier.output_tokens > 0
        || tier.known_model_cost_usd > 0.0
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

fn append_cost_note(rendered: &mut String, stats: &StatsReport) {
    if stats.assumed_standard_tokens > 0 {
        let _ = writeln!(
            rendered,
            "* Standard includes {} tokens without a recorded tier that were priced as Standard.",
            human_number(stats.assumed_standard_tokens)
        );
        rendered.push_str(
            "Tier history: Fast became generally available in Codex CLI 0.111.0 on 2026-03-05; applied-tier snapshots were not persisted until 0.144.0 on 2026-07-09.\n",
        );
    }
    if stats.estimated_cost_usd.is_none() {
        rendered.push_str(
            "Cost note: + means known lower-bound cost; the complete estimate is unavailable.\n",
        );
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
            tree: stats(None, 0.17),
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

        assert!(rendered.contains("Codex rollout root\nProject: /tmp/project\nName: Rollout stats\nType: root   Primary: gpt-5.6-terra / high   Descendants: 1"));
        assert!(rendered.contains("Root"));
        assert!(rendered.contains("Whole tree"));
        assert!(rendered.contains("Cache write"));
        assert!(rendered.contains("$0.17+"));
        assert!(rendered.contains(
            "Pricing basis: API list pricing; applied rollout tier (served tier unavailable)"
        ));
        assert!(rendered.contains("gpt-5.6-terra"));
        assert!(rendered.contains("↳ Standard*"));
        assert!(rendered.contains("↳ Fast"));
        assert!(!rendered.contains('⚡'));
        assert!(rendered.contains(
            "* Standard includes 990 tokens without a recorded tier that were priced as Standard."
        ));
        let models = rendered.split_once("Models\n").unwrap().1;
        assert!(models.find("gpt-5.6-terra").unwrap() < models.find("cheap").unwrap());
        assert!(rendered.contains("Total"));
        assert!(rendered.contains("gpt-5.6 -> gpt-5.6-terra"));
        assert!(rendered.contains("codex-auto-review before 2026-07-30 -> gpt-5.4"));
        assert!(rendered.contains("codex-auto-review from 2026-07-30 -> gpt-5.6-luna"));
        assert!(
            rendered
                .contains("announcement-date estimates, not observed routing or billing cutovers")
        );
        assert!(rendered.contains("Pricing as of: 2026-08-13"));
        assert!(rendered.contains("cache read is included in input"));
        assert!(rendered.contains("reasoning is included in output"));
        assert!(rendered.contains("agent time can overlap"));
    }

    #[test]
    fn human_collapses_single_service_modes_but_json_keeps_tiers_separate() {
        let mut report = report();
        let model = report.by_model.get_mut("gpt-5.6-terra").unwrap();
        model.by_service_tier.remove("fast");

        let rendered = human(&report);
        assert!(rendered.contains("gpt-5.6-terra [Standard*]"));
        assert!(!rendered.contains("↳ Standard"));
        let structured: Value = serde_json::from_str(&json(&report).unwrap()).unwrap();
        assert!(structured["by_model"]["gpt-5.6-terra"]["by_service_tier"]["standard"].is_object());
        assert!(
            structured["by_model"]["gpt-5.6-terra"]["by_service_tier"]["assumed_standard"]
                .is_object()
        );

        let model = report.by_model.get_mut("gpt-5.6-terra").unwrap();
        let assumed = model.by_service_tier.remove("assumed_standard").unwrap();
        assert!(human(&report).contains("gpt-5.6-terra [Standard]"));

        let model = report.by_model.get_mut("gpt-5.6-terra").unwrap();
        let fast = model.by_service_tier.remove("standard").unwrap();
        model.by_service_tier.clear();
        model
            .by_service_tier
            .insert("assumed_standard".into(), assumed);
        assert!(human(&report).contains("gpt-5.6-terra [Standard*]"));

        let model = report.by_model.get_mut("gpt-5.6-terra").unwrap();
        model.by_service_tier.clear();
        model.by_service_tier.insert("fast".into(), fast);
        let rendered = human(&report);
        assert!(rendered.contains("gpt-5.6-terra [Fast]"));
        assert!(!rendered.contains("↳ Fast"));
        assert!(!rendered.contains('⚡'));
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
            .find(|line| line.starts_with("↳ Standard*"))
            .unwrap();
        let fast = rendered
            .lines()
            .find(|line| line.starts_with("↳ Fast"))
            .unwrap();
        let column = |line: &str, value: &str| line[..line.find(value).unwrap()].chars().count();
        assert_eq!(column(model, "12K"), column(standard, "7K"));
        assert_eq!(column(model, "12K"), column(fast, "5K"));
        assert!(rendered.contains("Pricing sources:\n  - https://developers.openai.com/api/docs/pricing\n  - https://openai.com/api-fast-mode/"));
        assert!(
            rendered
                .lines()
                .all(|line| !line.starts_with("Pricing basis:") || line.chars().count() <= 116)
        );
    }

    #[test]
    fn human_preserves_unavailable_mode_in_mixed_output() {
        let mut report = report();
        let model = report.by_model.get_mut("gpt-5.6-terra").unwrap();
        model.by_service_tier.remove("fast");
        let unavailable = model.by_service_tier.remove("standard").unwrap();
        model.by_service_tier.insert("flex".into(), unavailable);

        let rendered = human(&report);
        assert!(rendered.contains("↳ Standard*"));
        assert!(rendered.contains("↳ Tier unavailable (flex)"));
    }

    #[test]
    fn empty_models_render_an_explicit_state() {
        let mut report = report();
        report.by_model.clear();
        let rendered = human(&report);
        let models = rendered.split_once("Models\n").unwrap().1;
        assert!(models.starts_with("No model usage.\n"));
        assert!(!models.starts_with("Model "));

        let mut project = ProjectReport {
            selection: ProjectSelection {
                target: "Project".into(),
                resolver: "project_name".into(),
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
        let models = rendered.split_once("Models\n").unwrap().1;
        assert!(models.starts_with("No model usage.\n"));
        project.selection.resolver = "corpus".into();
        assert!(project_human(&project).contains("Models\nNo model usage.\n"));
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
    fn human_omits_cache_write_column_when_unused() {
        let mut report = report();
        report.rollout.stats.input_cache_write_tokens = 0;
        report.tree.input_cache_write_tokens = 0;
        for model in report.by_model.values_mut() {
            model.input_cache_write_tokens = 0;
        }

        assert!(!human(&report).contains("Cache write"));
    }

    #[test]
    fn human_duration_uses_days_hours_and_nonzero_portions() {
        assert_eq!(super::human_duration(0.5), "0.5s");
        assert_eq!(super::human_duration(65.25), "1m 5.2s");
        assert_eq!(super::human_duration(111_349.2), "1d 6h 55m 49.2s");
    }

    #[test]
    fn project_human_keeps_nonzero_cache_write_usage_visible() {
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
                since: None,
                through: None,
                group_by: Vec::new(),
            },
            tree: report.tree,
            by_model: report.by_model,
            by_rollout_type: BTreeMap::new(),
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

        assert!(rendered.contains("Cache write"));
        assert!(rendered.contains("Input warnings:\n  - some input was incomplete"));
        assert!(rendered.contains("15.1B"));
        assert!(rendered.contains("269 incomplete"));
        assert!(rendered.contains("Incomplete root reports: 12"));
        assert!(rendered.contains("+ means known lower-bound cost"));
        assert!(rendered.contains("gpt-5.6 -> gpt-5.6-terra"));
        assert!(rendered.contains("codex-auto-review before 2026-07-30 -> gpt-5.4"));
        assert!(rendered.contains("codex-auto-review from 2026-07-30 -> gpt-5.6-luna"));
        assert!(
            rendered
                .contains("announcement-date estimates, not observed routing or billing cutovers")
        );

        project.selection.resolver = "corpus".into();
        let corpus = project_human(&project);
        assert!(corpus.contains("gpt-5.6 -> gpt-5.6-terra"));
        assert!(corpus.contains("codex-auto-review before 2026-07-30 -> gpt-5.4"));
        assert!(corpus.contains("codex-auto-review from 2026-07-30 -> gpt-5.6-luna"));
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
