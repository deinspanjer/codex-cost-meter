use std::fmt::Write;

use serde::Serialize;

use crate::report::{ModelReport, ProjectReport, Report, StatsReport};

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

    rendered.push_str("\nModels\n");
    rendered.push_str(&model_table(report, show_cache_write));

    let _ = writeln!(
        rendered,
        "\nAgent-turn time: {} (agent time can overlap).",
        human_duration(rollout.total_subagent_turn_duration_seconds)
    );
    let _ = writeln!(
        rendered,
        "Pricing as of: {}",
        safe_text(&report.pricing.as_of)
    );
    let _ = writeln!(
        rendered,
        "Pricing source: {}",
        safe_text(&report.pricing.source)
    );
    if !report.pricing.model_proxies.is_empty() {
        rendered.push_str("Model proxies:\n");
        for (model, target) in &report.pricing.model_proxies {
            let _ = writeln!(rendered, "  {} -> {}", safe_text(model), safe_text(target));
        }
    }
    if !report.incomplete_input_warnings.is_empty() {
        rendered.push_str("Incomplete input:\n");
        for warning in &report.incomplete_input_warnings {
            let _ = writeln!(rendered, "  - {}", safe_text(warning));
        }
    }
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
            .any(|model| model.input_cache_write_tokens > 0);
    let mut rendered = format!(
        "Codex project report\nProject: {}\nResolver: {}\nThreads: {} direct, {} workspace fallback, {} projectless, {} projectless excluded, {} other-project excluded\n",
        safe_text(&selection.target),
        safe_text(selection.resolver),
        human_number(selection.direct_assignments as u64),
        human_number(selection.workspace_fallbacks as u64),
        human_number(selection.projectless_threads as u64),
        human_number(selection.projectless_exclusions as u64),
        human_number(selection.other_project_exclusions as u64),
    );
    rendered.push_str("\nLifetime\n");
    rendered.push_str(&stats_table(show_cache_write, [("Project", &report.tree)]));
    rendered.push_str("\nModels\n");
    rendered.push_str(&model_table_from_models(
        &report.by_model,
        &report.tree,
        show_cache_write,
    ));
    let _ = writeln!(
        rendered,
        "\nPricing as of: {}",
        safe_text(&report.pricing.as_of)
    );
    let _ = writeln!(
        rendered,
        "Pricing source: {}",
        safe_text(&report.pricing.source)
    );
    if selection.incomplete_threads > 0 || selection.unpriced_threads > 0 {
        let _ = writeln!(
            rendered,
            "Incomplete threads: {}   Unpriced threads: {}",
            human_number(selection.incomplete_threads as u64),
            human_number(selection.unpriced_threads as u64),
        );
    }
    if !report.incomplete_input_warnings.is_empty() {
        rendered.push_str("Input warnings:\n");
        for warning in &report.incomplete_input_warnings {
            let _ = writeln!(rendered, "  - {}", safe_text(warning));
        }
    }
    rendered
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

fn model_table(report: &Report, show_cache_write: bool) -> String {
    model_table_from_models(&report.by_model, &report.tree, show_cache_write)
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
    table.extend(
        models
            .iter()
            .map(|(name, model)| model_row(name, model, show_cache_write)),
    );
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
        human_number(stats.input_tokens),
        human_number(stats.input_cache_read_tokens),
    ];
    if show_cache_write {
        row.push(human_number(stats.input_cache_write_tokens));
    }
    row.extend([
        human_number(stats.output_tokens),
        human_number(stats.reasoning_tokens),
        human_duration(stats.total_turn_duration_seconds),
        human_cost(stats.estimated_cost_usd, stats.known_model_cost_usd),
    ]);
    row
}

fn model_row(model: &str, stats: &ModelReport, show_cache_write: bool) -> Vec<String> {
    let mut row = vec![
        safe_text(model),
        human_number(stats.turns as u64),
        human_number(stats.input_tokens),
        human_number(stats.input_cache_read_tokens),
    ];
    if show_cache_write {
        row.push(human_number(stats.input_cache_write_tokens));
    }
    row.extend([
        human_number(stats.output_tokens),
        human_number(stats.reasoning_tokens),
        "-".into(),
        human_cost(stats.estimated_cost_usd, stats.known_model_cost_usd),
    ]);
    row
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
    if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else {
        format!("{}m {:.1}s", (seconds / 60.0).floor(), seconds % 60.0)
    }
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
        .map(|column| rows.iter().map(|row| row[column].len()).max().unwrap_or(0))
        .collect::<Vec<_>>();
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let line = row
                .iter()
                .enumerate()
                .map(|(column, cell)| format!("{cell:width$}", width = widths[column]))
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json as json_value};

    use super::{human, json, project_human};
    use crate::report::{
        ModelReport, PricingReport, ProjectReport, ProjectSelection, Report, RolloutReport,
        StatsReport,
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
                estimated_cost_usd: Some(0.02),
                known_model_cost_usd: 0.02,
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
                estimated_cost_usd: None,
                known_model_cost_usd: 0.15,
            },
        );
        let mut proxies = BTreeMap::new();
        proxies.insert("gpt-5.6".into(), "gpt-5.6-terra".into());
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
                basis: "standard API list pricing; per request/turn model; output includes reasoning",
                as_of: "2026-08-13".into(),
                source: "https://example.invalid/prices".into(),
                model_proxies: proxies,
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
        assert!(rendered.contains("gpt-5.6-terra"));
        let models = rendered.split_once("Models\n").unwrap().1;
        assert!(models.find("gpt-5.6-terra").unwrap() < models.find("cheap").unwrap());
        assert!(rendered.contains("Total"));
        assert!(rendered.contains("gpt-5.6 -> gpt-5.6-terra"));
        assert!(rendered.contains("Pricing as of: 2026-08-13"));
        assert!(rendered.contains("cache read is included in input"));
        assert!(rendered.contains("reasoning is included in output"));
        assert!(rendered.contains("agent time can overlap"));
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
                estimated_cost_usd: Some(0.0),
                known_model_cost_usd: 0.0,
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
    fn project_human_keeps_nonzero_cache_write_usage_visible() {
        let report = report();
        let project = ProjectReport {
            selection: ProjectSelection {
                target: "Project".into(),
                resolver: "project_name",
                missing_source_roots: 0,
                direct_assignments: 1,
                workspace_fallbacks: 0,
                projectless_threads: 0,
                projectless_exclusions: 0,
                other_project_exclusions: 0,
                incomplete_threads: 1,
                unpriced_threads: 0,
            },
            tree: report.tree,
            by_model: report.by_model,
            pricing: report.pricing,
            incomplete_input_warnings: vec!["some input was incomplete".into()],
        };

        let rendered = project_human(&project);

        assert!(rendered.contains("Cache write"));
        assert!(rendered.contains("Input warnings:\n  - some input was incomplete"));
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
    }
}
