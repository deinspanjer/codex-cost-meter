mod cache;
mod cli;
mod costs;
mod credits;
mod date_filter;
mod output;
mod pricing;
mod pricing_command;
#[path = "pricing/evidence.rs"]
mod pricing_evidence;
mod pricing_store;
mod progress;
mod project;
mod quota;
mod report;
mod rollout;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod schedule;
mod session_index;
mod thread_name;
mod title;
mod update;
mod usage_export;

use std::{
    error::Error,
    io::{self, IsTerminal, Write},
    process::ExitCode,
    rc::Rc,
};

use clap::{CommandFactory, error::ErrorKind};
use thiserror::Error as ThisError;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::{
    cli::ScheduleCommand,
    schedule::{InstallOptions, Paths, ScheduleError, ScheduledRunError},
};
use crate::{
    cli::{Cli, Command},
    report::ReportError,
};

#[derive(Debug, ThisError)]
enum AppError {
    #[error("{0}")]
    Valuation(String),
    #[error(transparent)]
    Title(#[from] title::TitleError),
    #[error("pricing refresh has unresolved items")]
    PricingPartial,
    #[error("{0}")]
    PricingCommand(String),
    #[error(transparent)]
    Export(#[from] usage_export::ExportError),
    #[error(transparent)]
    Pricing(#[from] pricing::PricingError),
    #[error(transparent)]
    Cli(#[from] cli::CliError),
    #[error(transparent)]
    DateFilter(#[from] date_filter::Error),
    #[error(transparent)]
    Report(#[from] ReportError),
    #[error(transparent)]
    Project(#[from] project::ProjectError),
    #[error(transparent)]
    Update(#[from] update::UpdateError),
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[error(transparent)]
    Schedule(#[from] ScheduleError),
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[error(transparent)]
    Scheduled(#[from] ScheduledRunError),
    #[error("could not render report: {0}")]
    Render(#[from] serde_json::Error),
    #[error("could not write report")]
    WriteReport {
        #[source]
        source: io::Error,
    },
    #[error("could not write completion script")]
    WriteCompletions {
        #[source]
        source: io::Error,
    },
    #[error("could not write update output")]
    WriteUpdateOutput {
        #[source]
        source: io::Error,
    },
    #[error("update applied successfully to {updated} task(s), but could not write update output")]
    UpdateOutputAfterApply {
        updated: usize,
        #[source]
        source: io::Error,
    },
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[error("could not write schedule output")]
    WriteScheduleOutput {
        #[source]
        source: io::Error,
    },
}

fn main() -> ExitCode {
    let cli = match parse_cli() {
        Ok(cli) => cli,
        Err(error) => {
            let exit_code = u8::try_from(error.exit_code()).unwrap_or(1);
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                let _ = error.print();
            } else {
                eprintln!("{}", sanitize(error.to_string()));
            }
            return ExitCode::from(exit_code);
        }
    };

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(AppError::PricingPartial) => ExitCode::from(3),
        Err(error) => {
            eprintln!("{}", error_chain(&error));
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), AppError> {
    let stderr = io::stderr();
    let terminal = stderr.is_terminal();
    run_with_writers(cli, &mut io::stdout().lock(), &mut stderr.lock(), terminal)
}

fn run_with_writers(
    cli: Cli,
    writer: &mut impl Write,
    error_writer: &mut impl Write,
    stderr_is_terminal: bool,
) -> Result<(), AppError> {
    let selection = cli.cost_metrics;
    let window = cli.quota_window;
    let config_path = cli.quota_config;
    match cli.command {
        Command::QuotaConfig(_) => {
            let path = quota::resolve(config_path.as_deref()).map_err(AppError::Valuation)?;
            quota::init(&path).map_err(AppError::Valuation)?;
            writeln!(writer, "created {} (single-account owner-derived Pro 20x weekly example; edit to match your policy)", path.display())
                .map_err(|source| AppError::WriteReport { source })
        }
        Command::Pricing(args) => {
            let partial = pricing_command::run(
                args.command,
                writer,
                error_writer,
                stderr_is_terminal && io::stdin().is_terminal(),
            )
            .map_err(AppError::PricingCommand)?;
            if partial {
                Err(AppError::PricingPartial)
            } else {
                Ok(())
            }
        }
        Command::Rates(args) => {
            let catalog = valuation_catalog(selection.clone(), window, config_path.as_deref())?;
            let table = match catalog.rate_table(args.model.as_deref(), args.as_of) {
                Ok(table) => table,
                Err(pricing::PricingError::NoRates { .. })
                    if !selection.0.contains(&costs::Metric::ApiUsd) =>
                {
                    let mut table = catalog.rate_table(None, args.as_of)?;
                    table.rows.clear();
                    table
                }
                Err(error) => return Err(error.into()),
            };
            let rendered = if args.json {
                let mut value = serde_json::to_value(&table)?;
                value["cost_metrics"] = serde_json::json!(selection.0);
                let mut rates = serde_json::Map::new();
                for metric in &selection.0 {
                    let value = match metric {
                        costs::Metric::ApiUsd => serde_json::to_value(&table)?,
                        costs::Metric::Credits => credit_rates(args.model.as_deref(), args.as_of),
                        costs::Metric::Quota => catalog
                            .cost_context
                            .quota
                            .as_ref()
                            .expect("quota loaded")
                            .provenance(window),
                    };
                    rates.insert(metric.name().into(), value);
                }
                value["costs"] = rates.into();
                format!("{}\n", output::json(&value)?)
            } else {
                let mut output = String::new();
                for metric in &selection.0 {
                    match metric {
                        costs::Metric::ApiUsd => output.push_str(&output::rates_human(&table)),
                        costs::Metric::Credits => output.push_str(&credit_rates_human(
                            &credit_rates(args.model.as_deref(), args.as_of),
                        )),
                        costs::Metric::Quota => output.push_str(&format!(
                            "Estimated {} usage (pp), not an official quota card\n{}\n",
                            window.name(),
                            catalog
                                .cost_context
                                .quota
                                .as_ref()
                                .expect("quota loaded")
                                .provenance(window)
                        )),
                    }
                }
                output
            };
            writer
                .write_all(rendered.as_bytes())
                .map_err(|source| AppError::WriteReport { source })
        }
        Command::Completions(args) => {
            let shell = match args.shell {
                cli::CompletionShell::Bash => clap_complete::Shell::Bash,
                cli::CompletionShell::Zsh => clap_complete::Shell::Zsh,
            };
            let mut script = Vec::new();
            clap_complete::generate(shell, &mut Cli::command(), "codex-cost-meter", &mut script);
            writer
                .write_all(&script)
                .map_err(|source| AppError::WriteCompletions { source })
        }
        Command::Report(mut args) => {
            let catalog = valuation_catalog(selection.clone(), window, config_path.as_deref())?;
            let filter = args.date_filter()?;
            let home = args.codex_home()?;
            let cache = Rc::new(cache::RolloutCache::open(&home, args.refresh));
            let mut progress =
                progress::Progress::new(error_writer, args.progress, stderr_is_terminal);
            if let Some(query) = &args.match_title {
                args.thread_id = Some(report::resolve_title(
                    &home,
                    query,
                    Rc::clone(&cache),
                    &mut progress,
                )?);
            }
            if args.jsonl {
                let result = (|| -> Result<(), AppError> {
                    if args.all {
                        let mut context = report::ReportContext::new_corpus_cached_with_progress(
                            &home,
                            &filter,
                            Rc::clone(&cache),
                            &mut progress,
                        )
                        .map_err(ReportError::from)?;
                        context.catalog = catalog.clone();
                        usage_export::write(
                            &context,
                            usage_export::Selection::All,
                            &filter,
                            writer,
                        )?;
                    } else if args.project.is_some() || args.thread_id.is_none() {
                        let mut scope = project::resolve_with_progress(
                            &home,
                            args.thread_id.as_deref(),
                            args.project.as_deref().unwrap_or(""),
                            &filter,
                            &mut progress,
                            Rc::clone(&cache),
                        )?;
                        scope.context.catalog = catalog.clone();
                        usage_export::write(
                            &scope.context,
                            usage_export::Selection::Project(&scope.selection, &scope.thread_ids),
                            &filter,
                            writer,
                        )?;
                    } else if let Some(id) = args.thread_id.as_deref() {
                        let mut context = report::ReportContext::new_for_cached_with_progress(
                            &home,
                            &[id.into()],
                            Rc::clone(&cache),
                            &mut progress,
                        )
                        .map_err(ReportError::from)?;
                        context.catalog = catalog.clone();
                        usage_export::write(
                            &context,
                            usage_export::Selection::Thread(id),
                            &filter,
                            writer,
                        )?;
                    }
                    Ok(())
                })();
                progress.finish();
                drop(progress);
                write_cache_notices(error_writer, &cache);
                return result;
            }
            let rendered = (|| -> Result<String, AppError> {
                if args.all {
                    let mut context = report::ReportContext::new_corpus_cached_with_progress(
                        &home,
                        &filter,
                        Rc::clone(&cache),
                        &mut progress,
                    )
                    .map_err(ReportError::from)?;
                    context.catalog = catalog.clone();
                    let report = context.build_corpus_with_progress(&filter, &mut progress)?;
                    if args.json {
                        Ok(format!("{}\n", output::json(&report)?))
                    } else {
                        Ok(output::project_human(&report))
                    }
                } else if let Some(project_ref) = args.project {
                    let report = project::build_with_progress(
                        &home,
                        args.thread_id.as_deref(),
                        &project_ref,
                        &filter,
                        &mut progress,
                        Rc::clone(&cache),
                        &catalog,
                    )?;
                    if args.json {
                        Ok(format!("{}\n", output::json(&report)?))
                    } else {
                        Ok(output::project_human(&report))
                    }
                } else if let Some(thread_id) = args.thread_id {
                    let report = report::build_with_progress(
                        &thread_id,
                        &home,
                        &mut progress,
                        Rc::clone(&cache),
                        &filter,
                        &catalog,
                    )?;
                    if args.json {
                        Ok(format!("{}\n", output::json(&report)?))
                    } else {
                        Ok(output::human(&report))
                    }
                } else {
                    let report = project::build_with_progress(
                        &home,
                        None,
                        "",
                        &filter,
                        &mut progress,
                        Rc::clone(&cache),
                        &catalog,
                    )?;
                    if args.json {
                        Ok(format!("{}\n", output::json(&report)?))
                    } else {
                        Ok(output::project_human(&report))
                    }
                }
            })();
            progress.finish();
            drop(progress);
            let rendered = rendered?;
            write_cache_notices(error_writer, &cache);
            writer
                .write_all(rendered.as_bytes())
                .map_err(|source| AppError::WriteReport { source })?;
            Ok(())
        }
        Command::Update(args) => {
            let options = args.options(&selection, window)?;
            let catalog = valuation_catalog(
                options.title_format.requested(),
                window,
                config_path.as_deref(),
            )?;
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            warn_schedule(&options.title_format, &selection, &catalog, error_writer);
            let home = args.codex_home()?;
            let cache = Rc::new(cache::RolloutCache::open(&home, args.refresh));
            let result = update::run_cached(&home, &options, Rc::clone(&cache), &catalog)?;
            write_cache_notices(error_writer, &cache);
            write_update_output(writer, &result, options.apply)?;
            Ok(())
        }
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        Command::Schedule(args) => run_schedule(
            args.command,
            writer,
            error_writer,
            &selection,
            window,
            config_path.as_deref(),
        ),
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        Command::Uninstall => {
            let paths = schedule_paths()?;
            schedule::uninstall(&paths)?;
            writer
                .write_all(b"schedule state and current executable removed\n")
                .map_err(|source| AppError::WriteScheduleOutput { source })
        }
        #[cfg(target_os = "windows")]
        Command::Uninstall => {
            let paths = schedule_paths()?;
            schedule::uninstall(&paths)?;
            writer
                .write_all(b"schedule state removed; executable deletion scheduled\n")
                .map_err(|source| AppError::WriteScheduleOutput { source })
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn run_schedule(
    command: ScheduleCommand,
    writer: &mut impl Write,
    error_writer: &mut impl Write,
    selection: &costs::Selection,
    window: quota::Window,
    config_path: Option<&std::path::Path>,
) -> Result<(), AppError> {
    match command {
        ScheduleCommand::Install(args) => {
            let schedule_options = args.options();
            let recipe = schedule_options
                .update_options(selection, window)?
                .title_format;
            let catalog = valuation_catalog(recipe.requested(), window, config_path)?;
            let paths = schedule_paths()?;
            let options = InstallOptions {
                valuation_arguments: {
                    let mut arguments = vec![
                        "--cost-metric".into(),
                        selection.text(),
                        "--quota-window".into(),
                        window.name().into(),
                    ];
                    if let Some(quota) = &catalog.cost_context.quota {
                        arguments.extend([
                            "--quota-config".into(),
                            quota.path.to_string_lossy().into_owned(),
                        ]);
                    }
                    arguments
                },
                executable: std::env::current_exe()
                    .map_err(|source| ScheduleError::CurrentExecutable { source })?,
                codex_home: schedule_options.codex_home()?,
                idle_minutes: schedule_options.idle_minutes(),
                limit: schedule_options.limit(),
                max_runtime: schedule_options.max_runtime(),
                max_width: schedule_options.max_width(),
                title_metrics: recipe.recipe(),
                reprice_before: schedule_options.reprice_before(),
            };
            schedule::install(&paths, &options)?;
            writer
                .write_all(schedule_install_output(&paths).as_bytes())
                .map_err(|source| AppError::WriteScheduleOutput { source })
        }
        ScheduleCommand::Status => {
            let inspection = schedule::inspect(&schedule_paths()?)?;
            #[cfg(target_os = "macos")]
            let mut output = format!(
                "installed: {}\nloaded: {}\n",
                yes_no(inspection.installed),
                yes_no(inspection.loaded),
            );
            #[cfg(target_os = "linux")]
            let mut output = format!(
                "installed: {}\nactive: {}\n",
                yes_no(inspection.installed),
                yes_no(inspection.active),
            );
            #[cfg(target_os = "windows")]
            let mut output = format!("registered: {}\n", yes_no(inspection.registered));
            if let Some(settings) = &inspection.settings {
                output.push_str(&format!("settings: {}\n", sanitize(settings.description())));
            } else {
                output.push_str("settings: absent or indeterminate\n");
            }
            if let Some(status) = inspection.status {
                let last_run = status
                    .last_run_at
                    .map(|timestamp| {
                        timestamp
                            .format(&time::format_description::well_known::Rfc3339)
                            .expect("OffsetDateTime always formats as RFC 3339")
                    })
                    .unwrap_or_else(|| "never".into());
                output.push_str(&format!(
                    "last run: {}\nresult: {}\nconsecutive failures: {}\npaused: {}\nremediation: {}\n",
                    last_run,
                    schedule::result_code(status.result),
                    status.consecutive_failures,
                    yes_no(status.paused),
                    status.remediation,
                ));
            } else {
                output.push_str("last run: never\n");
            }
            writer
                .write_all(output.as_bytes())
                .map_err(|source| AppError::WriteScheduleOutput { source })
        }
        ScheduleCommand::Resume => {
            schedule::resume(&schedule_paths()?)?;
            writer
                .write_all(b"schedule resumed\n")
                .map_err(|source| AppError::WriteScheduleOutput { source })
        }
        ScheduleCommand::Remove => {
            schedule::remove(&schedule_paths()?)?;
            writer
                .write_all(b"schedule removed\n")
                .map_err(|source| AppError::WriteScheduleOutput { source })
        }
        ScheduleCommand::Run(args) => {
            let paths = schedule_paths()?;
            let schedule_options = args.options();
            let options = schedule_options.update_options(selection, window)?;
            let catalog = valuation_catalog(options.title_format.requested(), window, config_path)?;
            let home = schedule_options.codex_home()?;
            let cache = Rc::new(cache::RolloutCache::open(&home, false));
            let result = schedule::run_scheduled_cached(
                &paths,
                &home,
                &options,
                writer,
                Rc::clone(&cache),
                &catalog,
            );
            result?;
            write_cache_notices(error_writer, &cache);
            Ok(())
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn warn_schedule(
    recipe: &title::TitleFormat,
    selection: &costs::Selection,
    catalog: &pricing::Catalog,
    writer: &mut impl Write,
) {
    let manual = schedule::Settings {
        selection: selection.text(),
        recipe: recipe.recipe(),
        window: recipe.window,
        config: catalog.cost_context.quota.as_ref().map(|q| q.path.clone()),
    };
    let inspection = schedule_paths()
        .map_err(|error| error.to_string())
        .and_then(|paths| schedule::inspect(&paths).map_err(|error| error.to_string()));
    let installed = match inspection {
        Ok(inspection) => {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            let installed = inspection.installed;
            #[cfg(target_os = "windows")]
            let installed = inspection.registered;
            Ok(if !installed {
                schedule::TitleSettings::Absent
            } else if let Some(settings) = inspection.settings {
                schedule::TitleSettings::Known(settings)
            } else {
                schedule::TitleSettings::Indeterminate
            })
        }
        Err(error) => Err(error),
    };
    if let Some(notice) = schedule::settings_warning(&manual, installed) {
        let _ = writeln!(writer, "warning: {}", sanitize(notice));
    }
}

fn valuation_catalog(
    selection: costs::Selection,
    window: quota::Window,
    path: Option<&std::path::Path>,
) -> Result<pricing::Catalog, AppError> {
    let quota = if selection.0.contains(&costs::Metric::Quota) {
        Some(Rc::new(
            quota::Snapshot::load(quota::resolve(path).map_err(AppError::Valuation)?)
                .map_err(AppError::Valuation)?,
        ))
    } else {
        None
    };
    let mut catalog = pricing_store::Snapshot::load(pricing_store::path()?)?.effective;
    catalog.cost_context = costs::Context {
        selection,
        quota,
        window,
    };
    Ok(catalog)
}

fn credit_rates(model: Option<&str>, at: Option<time::Date>) -> serde_json::Value {
    let cards = credits::cards();
    let mut rows = std::collections::BTreeMap::new();
    for card in cards
        .iter()
        .filter(|card| at.is_none_or(|at| at.to_string().as_str() >= &card.observed_at[..10]))
    {
        for row in card
            .rows
            .iter()
            .filter(|row| model.is_none_or(|model| model == row.model))
        {
            let mut value = serde_json::to_value(row).expect("finite embedded rates");
            value["observed_at"] = serde_json::json!(card.observed_at);
            value["source"] = serde_json::json!(card.source);
            rows.insert(row.model.clone(), value);
        }
    }
    serde_json::json!({
        "unit":"credits_per_million_tokens",
        "catalog_observed":"2026-11-19",
        "coverage":"token-only; observation-boundary estimates; unknown historical cutovers, special workloads and context above 272000 incomplete",
        "speed_values":"multipliers relative to Standard; null means unsupported",
        "rows":rows.into_values().collect::<Vec<_>>()
    })
}

fn credit_rates_human(rates: &serde_json::Value) -> String {
    let mut output = String::from(
        "Estimated credit cost: credits per million tokens (independent of USD)\nModel | Observed | Input | Cached input | Output | Fast × | Ultrafast ×\n",
    );
    for row in rates["rows"].as_array().expect("credit rows") {
        let cells = [
            "model",
            "observed_at",
            "input",
            "cached_input",
            "output",
            "fast",
            "ultrafast",
        ]
        .map(|key| {
            if row[key].is_null() {
                "?".into()
            } else if let Some(value) = row[key].as_str() {
                value.into()
            } else {
                row[key].to_string()
            }
        });
        output.push_str(&cells.join(" | "));
        output.push('\n');
    }
    output.push_str("No separate cache-write or reasoning fee. Observation dates are not established billing cutovers; token-only coverage is limited to gross input <=272000. See docs/research/credit-rate-evidence.md for sources and gaps.\n");
    output
}

fn write_cache_notices(writer: &mut impl Write, cache: &cache::RolloutCache) {
    for notice in cache.take_notices() {
        let _ = writeln!(writer, "{}", sanitize(notice));
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(target_os = "macos")]
fn schedule_paths() -> Result<Paths, cli::CliError> {
    Ok(Paths::new(&cli::user_home()?))
}

#[cfg(target_os = "linux")]
fn schedule_paths() -> Result<Paths, cli::CliError> {
    Ok(Paths::new(&cli::user_home()?))
}

#[cfg(target_os = "windows")]
fn schedule_paths() -> Result<Paths, cli::CliError> {
    Ok(Paths::new(&cli::local_app_data()?))
}

#[cfg(target_os = "macos")]
fn schedule_install_output(paths: &Paths) -> String {
    format!("schedule installed: {}\n", paths.plist().display())
}

#[cfg(target_os = "linux")]
fn schedule_install_output(paths: &Paths) -> String {
    format!("schedule installed: {}\n", paths.timer().display())
}

#[cfg(target_os = "windows")]
fn schedule_install_output(_: &Paths) -> String {
    "schedule registered\n".into()
}

fn parse_cli() -> Result<Cli, clap::Error> {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    if arguments.iter().any(|argument| {
        argument
            .as_encoded_bytes()
            .iter()
            .any(|byte| byte.is_ascii_control())
    }) {
        return Err(clap::Error::raw(
            ErrorKind::InvalidValue,
            "command-line argument contains a control character",
        ));
    }
    Cli::parse_arguments(arguments)
}

fn update_output(result: &update::UpdateResult, apply: bool) -> String {
    let mut rendered = result
        .proposals
        .iter()
        .fold(String::new(), |mut output, proposal| {
            output.push_str(&format!(
                "{}: {} -> {}\n",
                sanitize(proposal.id.clone()),
                sanitize(proposal.old_title.clone()),
                sanitize(proposal.new_title.clone())
            ));
            output
        });
    if result.skipped_rollouts > 0 {
        rendered.push_str(&format!(
            "warning: skipped {} task(s) with missing, unreadable, or malformed rollouts\n",
            result.skipped_rollouts
        ));
    }
    if apply {
        rendered.push_str(&format!("updated {} task(s)\n", result.proposals.len()));
    } else {
        rendered.push_str("dry run; pass --apply to write\n");
    }
    rendered
}

fn write_update_output(
    writer: &mut impl Write,
    result: &update::UpdateResult,
    apply: bool,
) -> Result<(), AppError> {
    writer
        .write_all(update_output(result, apply).as_bytes())
        .map_err(|source| {
            if apply {
                AppError::UpdateOutputAfterApply {
                    updated: result.proposals.len(),
                    source,
                }
            } else {
                AppError::WriteUpdateOutput { source }
            }
        })
}

fn error_chain(error: &dyn Error) -> String {
    let mut messages = vec![sanitize(error.to_string())];
    let mut source = error.source();
    while let Some(error) = source {
        messages.push(sanitize(error.to_string()));
        source = error.source();
    }
    messages.join(": ")
}

fn sanitize(value: String) -> String {
    value
        .split(|character: char| character.is_control() || character.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use super::*;

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("stdout failed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn completion_write_failure_reports_io_error() {
        let cli = Cli::parse_arguments(["codex-cost-meter", "completions", "bash"]).unwrap();
        let error = run_with_writers(cli, &mut FailingWriter, &mut Vec::new(), false).unwrap_err();
        assert!(matches!(error, AppError::WriteCompletions { .. }));
    }

    #[test]
    fn applied_update_write_failure_reports_completed_count() {
        let result = update::UpdateResult {
            proposals: vec![update::ProposedUpdate {
                id: "root".into(),
                old_title: "Old title".into(),
                new_title: "New title".into(),
            }],
            skipped_rollouts: 0,
        };

        let error = write_update_output(&mut FailingWriter, &result, true).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("update applied successfully to 1 task(s)")
        );
        assert!(error.to_string().contains("could not write update output"));
    }
}
