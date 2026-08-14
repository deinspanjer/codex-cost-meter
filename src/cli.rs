use std::{env, path::PathBuf, time::Duration};

use clap::{ArgGroup, Args, Parser, Subcommand};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    title::{MetricList, TitleFormat},
    update::UpdateOptions,
};

#[derive(Debug, Parser)]
#[command(name = "codex-cost-meter")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Report(ReportArgs),
    Update(UpdateArgs),
    Schedule(ScheduleArgs),
    Uninstall,
}

#[derive(Debug, Args)]
pub(crate) struct ScheduleArgs {
    #[command(subcommand)]
    pub(crate) command: ScheduleCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ScheduleCommand {
    Install(ScheduleInstallArgs),
    Status,
    Resume,
    Remove,
    #[command(hide = true)]
    Run(ScheduleRunArgs),
}

#[derive(Debug, Args)]
pub(crate) struct ScheduleInstallArgs {
    #[command(flatten)]
    options: ScheduledUpdateArgs,
}

#[derive(Debug, Args)]
pub(crate) struct ScheduleRunArgs {
    #[command(flatten)]
    options: ScheduledUpdateArgs,
    #[arg(long, required = true)]
    apply: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ScheduledUpdateArgs {
    #[arg(long, default_value_t = 15, value_parser = parse_positive_u64)]
    idle_minutes: u64,
    #[arg(long, default_value_t = 500, value_parser = parse_positive_usize)]
    limit: usize,
    #[arg(long, default_value = "4m", value_parser = parse_runtime)]
    max_runtime: Duration,
    #[arg(long, value_parser = parse_reprice_before)]
    reprice_before: Option<OffsetDateTime>,
    #[arg(long, default_value_t = 65, value_parser = parse_positive_usize)]
    max_width: usize,
    #[arg(long, default_value = "cost,total-tokens", value_parser = parse_metric_text)]
    title_metrics: String,
    #[arg(long)]
    codex_home: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct ReportArgs {
    pub(crate) thread_id: String,
    #[arg(long)]
    pub(crate) json: bool,
    #[arg(long)]
    codex_home: Option<PathBuf>,
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("selection")
        .args(["thread_ids", "title_matches", "idle_minutes"])
        .required(true)
        .multiple(true)
))]
pub(crate) struct UpdateArgs {
    #[arg(long = "thread-id", conflicts_with = "idle_minutes")]
    pub(crate) thread_ids: Vec<String>,
    #[arg(long = "match-title", conflicts_with = "idle_minutes")]
    pub(crate) title_matches: Vec<String>,
    #[arg(long, value_parser = parse_positive_u64)]
    pub(crate) idle_minutes: Option<u64>,
    #[arg(long, default_value_t = 20, value_parser = parse_positive_usize, requires = "idle_minutes")]
    pub(crate) limit: usize,
    #[arg(long, value_parser = parse_runtime)]
    pub(crate) max_runtime: Option<Duration>,
    #[arg(long, value_parser = parse_reprice_before, requires = "idle_minutes", conflicts_with_all = ["thread_ids", "title_matches"])]
    pub(crate) reprice_before: Option<OffsetDateTime>,
    #[arg(long)]
    pub(crate) apply: bool,
    #[arg(long, default_value_t = 65, value_parser = parse_positive_usize)]
    pub(crate) max_width: usize,
    #[arg(long, default_value = "cost,total-tokens", value_parser = parse_metrics)]
    pub(crate) title_metrics: MetricList,
    #[arg(long)]
    codex_home: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub(crate) enum CliError {
    #[error("could not resolve default Codex home: HOME is not set")]
    HomeNotSet,
}

impl ReportArgs {
    pub(crate) fn codex_home(&self) -> Result<PathBuf, CliError> {
        resolve_codex_home(&self.codex_home)
    }
}

impl UpdateArgs {
    pub(crate) fn codex_home(&self) -> Result<PathBuf, CliError> {
        resolve_codex_home(&self.codex_home)
    }

    pub(crate) fn options(&self) -> UpdateOptions {
        UpdateOptions {
            thread_ids: self.thread_ids.clone(),
            title_matches: self.title_matches.clone(),
            idle_minutes: self.idle_minutes,
            limit: self.limit,
            max_runtime: self.max_runtime,
            reprice_before: self.reprice_before,
            apply: self.apply,
            title_format: TitleFormat::new(self.max_width, self.title_metrics.clone()),
        }
    }
}

impl ScheduleInstallArgs {
    pub(crate) fn options(&self) -> &ScheduledUpdateArgs {
        &self.options
    }
}

impl ScheduleRunArgs {
    pub(crate) fn options(&self) -> &ScheduledUpdateArgs {
        debug_assert!(self.apply);
        &self.options
    }
}

impl ScheduledUpdateArgs {
    pub(crate) fn codex_home(&self) -> Result<PathBuf, CliError> {
        resolve_codex_home(&self.codex_home)
    }

    pub(crate) fn update_options(&self) -> UpdateOptions {
        UpdateOptions {
            thread_ids: Vec::new(),
            title_matches: Vec::new(),
            idle_minutes: Some(self.idle_minutes),
            limit: self.limit,
            max_runtime: Some(self.max_runtime),
            reprice_before: self.reprice_before,
            apply: true,
            title_format: TitleFormat::new(
                self.max_width,
                parse_metrics(&self.title_metrics).expect("clap validates title metrics"),
            ),
        }
    }

    pub(crate) fn idle_minutes(&self) -> u64 {
        self.idle_minutes
    }

    pub(crate) fn limit(&self) -> usize {
        self.limit
    }

    pub(crate) fn max_runtime(&self) -> Duration {
        self.max_runtime
    }

    pub(crate) fn max_width(&self) -> usize {
        self.max_width
    }

    pub(crate) fn title_metrics(&self) -> &str {
        &self.title_metrics
    }

    pub(crate) fn reprice_before(&self) -> Option<OffsetDateTime> {
        self.reprice_before
    }
}

pub(crate) fn user_home() -> Result<PathBuf, CliError> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(CliError::HomeNotSet)
}

fn resolve_codex_home(codex_home: &Option<PathBuf>) -> Result<PathBuf, CliError> {
    if let Some(home) = codex_home {
        return Ok(home.clone());
    }
    if let Some(home) = env::var_os("CODEX_HOME") {
        return Ok(home.into());
    }
    Ok(user_home()?.join(".codex"))
}

fn parse_positive_u64(value: &str) -> Result<u64, String> {
    value
        .parse()
        .ok()
        .filter(|value: &u64| *value > 0)
        .ok_or_else(|| "must be a positive integer".into())
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    value
        .parse()
        .ok()
        .filter(|value: &usize| *value > 0)
        .ok_or_else(|| "must be a positive integer".into())
}

fn parse_runtime(value: &str) -> Result<Duration, String> {
    let (value, multiplier) = value
        .strip_suffix('m')
        .map_or((value, 1), |value| (value, 60));
    parse_positive_u64(value)
        .and_then(|value| {
            value
                .checked_mul(multiplier)
                .ok_or_else(|| "runtime is too large".into())
        })
        .map(Duration::from_secs)
}

fn parse_reprice_before(value: &str) -> Result<OffsetDateTime, String> {
    let timestamp = if let Ok(timestamp) = OffsetDateTime::parse(value, &Rfc3339) {
        timestamp
    } else {
        let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
            .expect("static date format must parse");
        time::Date::parse(value, &format)
            .map(|date| date.with_time(time::Time::MIDNIGHT).assume_utc())
            .map_err(|_| "must be an ISO date or RFC 3339 timestamp with a timezone".to_owned())?
    };
    if timestamp > OffsetDateTime::now_utc() {
        return Err("cannot be in the future".into());
    }
    Ok(timestamp)
}

fn parse_metrics(value: &str) -> Result<MetricList, String> {
    value
        .parse::<MetricList>()
        .map_err(|error| error.to_string())
}

fn parse_metric_text(value: &str) -> Result<String, String> {
    parse_metrics(value).map(|_| value.into())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command, ScheduleCommand, UpdateArgs};
    use crate::title::MetricList;

    fn update(arguments: &[&str]) -> UpdateArgs {
        let mut command = vec!["codex-cost-meter", "update"];
        command.extend_from_slice(arguments);
        match Cli::try_parse_from(command) {
            Ok(Cli {
                command: Command::Update(arguments),
            }) => arguments,
            Ok(_) => panic!("expected update arguments"),
            Err(error) => panic!("expected valid arguments: {error}"),
        }
    }

    #[test]
    fn parses_explicit_selection_and_defaults() {
        let arguments = update(&["--thread-id", "root", "--match-title", "billing"]);

        assert_eq!(arguments.thread_ids, ["root"]);
        assert_eq!(arguments.title_matches, ["billing"]);
        assert_eq!(arguments.idle_minutes, None);
        assert_eq!(arguments.limit, 20);
        assert_eq!(arguments.max_runtime, None);
        assert_eq!(arguments.max_width, 65);
        assert_eq!(arguments.title_metrics, MetricList::default());
        assert!(!arguments.apply);
    }

    #[test]
    fn parses_idle_selection_runtime_metrics_and_cutoff() {
        let arguments = update(&[
            "--idle-minutes",
            "15",
            "--limit",
            "2",
            "--max-runtime",
            "5m",
            "--reprice-before",
            "2026-08-13",
            "--max-width",
            "80",
            "--title-metrics",
            "output-tokens,cost,input-tokens",
            "--apply",
        ]);

        assert_eq!(arguments.idle_minutes, Some(15));
        assert_eq!(arguments.limit, 2);
        assert_eq!(arguments.max_runtime.unwrap().as_secs(), 300);
        assert_eq!(arguments.max_width, 80);
        assert_eq!(
            arguments.title_metrics,
            "output-tokens,cost,input-tokens"
                .parse::<MetricList>()
                .unwrap()
        );
        assert!(arguments.apply);

        let seconds = update(&["--thread-id", "root", "--max-runtime", "90"]);
        assert_eq!(seconds.max_runtime.unwrap().as_secs(), 90);
        assert_eq!(
            MetricList::default(),
            "cost,total-tokens".parse::<MetricList>().unwrap()
        );
    }

    #[test]
    fn rejects_invalid_or_incompatible_selection_values() {
        for arguments in [
            vec![],
            vec!["--thread-id", "root", "--idle-minutes", "15"],
            vec!["--idle-minutes", "0"],
            vec!["--idle-minutes", "1", "--limit", "0"],
            vec!["--idle-minutes", "1", "--max-width", "0"],
            vec!["--thread-id", "root", "--max-runtime", "0"],
            vec!["--thread-id", "root", "--max-runtime", "3h"],
            vec!["--thread-id", "root", "--reprice-before", "2026-08-13"],
            vec!["--idle-minutes", "1", "--reprice-before", "2030-01-01"],
            vec![
                "--idle-minutes",
                "1",
                "--reprice-before",
                "2026-08-13T12:00:00",
            ],
            vec!["--thread-id", "root", "--title-metrics", "cost,cost"],
        ] {
            let mut command = vec!["codex-cost-meter", "update"];
            command.extend(arguments.iter().copied());
            assert!(Cli::try_parse_from(command).is_err(), "{arguments:?}");
        }
    }

    #[test]
    fn parses_schedule_commands_and_rejects_unsupported_selection() {
        for arguments in [
            vec!["schedule", "install"],
            vec![
                "schedule",
                "install",
                "--idle-minutes",
                "7",
                "--limit",
                "3",
                "--max-runtime",
                "90",
                "--max-width",
                "80",
                "--title-metrics",
                "output-tokens,cost",
                "--reprice-before",
                "2026-08-13",
                "--codex-home",
                "/tmp/codex",
            ],
            vec!["schedule", "run", "--idle-minutes", "15", "--apply"],
        ] {
            let mut command = vec!["codex-cost-meter"];
            command.extend(arguments.iter().copied());
            assert!(Cli::try_parse_from(command).is_ok(), "{arguments:?}");
        }

        for arguments in [
            vec!["schedule", "install", "--thread-id", "root"],
            vec!["schedule", "install", "--match-title", "billing"],
            vec!["schedule", "install", "--idle-minutes", "0"],
            vec!["schedule", "install", "--limit", "0"],
            vec!["schedule", "install", "--max-width", "0"],
            vec!["schedule", "install", "--title-metrics", "unknown"],
            vec!["schedule", "run", "--idle-minutes", "15"],
            vec!["schedule", "unknown"],
        ] {
            let mut command = vec!["codex-cost-meter"];
            command.extend(arguments.iter().copied());
            assert!(Cli::try_parse_from(command).is_err(), "{arguments:?}");
        }
    }

    #[test]
    fn schedule_arguments_preserve_the_installed_update_contract() {
        let parse = |arguments: &[&str]| {
            let mut command = vec!["codex-cost-meter", "schedule"];
            command.extend_from_slice(arguments);
            match Cli::try_parse_from(command).unwrap().command {
                Command::Schedule(arguments) => arguments.command,
                _ => panic!("expected schedule arguments"),
            }
        };

        let ScheduleCommand::Install(defaults) = parse(&["install"]) else {
            panic!("expected install arguments");
        };
        let defaults = defaults.options();
        assert_eq!(defaults.idle_minutes, 15);
        assert_eq!(defaults.limit, 500);
        assert_eq!(defaults.max_runtime.as_secs(), 240);
        assert_eq!(defaults.max_width, 65);
        assert_eq!(defaults.title_metrics, "cost,total-tokens");
        assert_eq!(defaults.reprice_before, None);
        assert_eq!(defaults.codex_home, None);

        let ScheduleCommand::Install(overrides) = parse(&[
            "install",
            "--idle-minutes",
            "7",
            "--limit",
            "3",
            "--max-runtime",
            "90",
            "--max-width",
            "80",
            "--title-metrics",
            "output-tokens,cost",
            "--reprice-before",
            "2026-08-13",
            "--codex-home",
            "/tmp/codex",
        ]) else {
            panic!("expected install arguments");
        };
        let overrides = overrides.options();
        assert_eq!(overrides.idle_minutes, 7);
        assert_eq!(overrides.limit, 3);
        assert_eq!(overrides.max_runtime.as_secs(), 90);
        assert_eq!(overrides.max_width, 80);
        assert_eq!(overrides.title_metrics, "output-tokens,cost");
        assert_eq!(
            overrides.reprice_before.unwrap().date().to_string(),
            "2026-08-13"
        );
        assert_eq!(
            overrides.codex_home.as_deref(),
            Some(std::path::Path::new("/tmp/codex"))
        );

        let ScheduleCommand::Run(run) = parse(&["run", "--idle-minutes", "15", "--apply"]) else {
            panic!("expected internal run arguments");
        };
        assert!(run.apply);
        assert_eq!(run.options().max_runtime().as_secs(), 240);
    }
}
