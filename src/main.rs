mod cache;
mod cli;
mod output;
mod pricing;
mod progress;
mod project;
mod report;
mod rollout;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod schedule;
mod session_index;
mod title;
mod update;

use std::{
    error::Error,
    io::{self, IsTerminal, Write},
    process::ExitCode,
    rc::Rc,
};

use clap::{Parser, error::ErrorKind};
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
    #[error(transparent)]
    Cli(#[from] cli::CliError),
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
    match cli.command {
        Command::Report(args) => {
            let home = args.codex_home()?;
            let cache = Rc::new(cache::RolloutCache::open(&home, args.refresh));
            let mut progress =
                progress::Progress::new(error_writer, args.progress, stderr_is_terminal);
            let rendered = (|| -> Result<String, AppError> {
                if let Some(project_ref) = args.project {
                    let report = project::build_with_progress(
                        &home,
                        args.thread_id.as_deref(),
                        &project_ref,
                        &mut progress,
                        Rc::clone(&cache),
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
                        &mut progress,
                        Rc::clone(&cache),
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
            let home = args.codex_home()?;
            let cache = Rc::new(cache::RolloutCache::open(&home, args.refresh));
            let options = args.options();
            let result = update::run_cached(&home, &options, Rc::clone(&cache))?;
            write_cache_notices(error_writer, &cache);
            write_update_output(writer, &result, options.apply)?;
            Ok(())
        }
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        Command::Schedule(args) => run_schedule(args.command, writer, error_writer),
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
) -> Result<(), AppError> {
    match command {
        ScheduleCommand::Install(args) => {
            let schedule_options = args.options();
            let paths = schedule_paths()?;
            let options = InstallOptions {
                executable: std::env::current_exe()
                    .map_err(|source| ScheduleError::CurrentExecutable { source })?,
                codex_home: schedule_options.codex_home()?,
                idle_minutes: schedule_options.idle_minutes(),
                limit: schedule_options.limit(),
                max_runtime: schedule_options.max_runtime(),
                max_width: schedule_options.max_width(),
                title_metrics: schedule_options.title_metrics().into(),
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
            let home = schedule_options.codex_home()?;
            let cache = Rc::new(cache::RolloutCache::open(&home, false));
            let result = schedule::run_scheduled_cached(
                &paths,
                &home,
                &schedule_options.update_options(),
                writer,
                Rc::clone(&cache),
            );
            result?;
            write_cache_notices(error_writer, &cache);
            Ok(())
        }
    }
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
    Cli::try_parse_from(arguments)
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
    fn applied_update_write_failure_reports_completed_count() {
        let result = update::UpdateResult {
            proposals: vec![update::ProposedUpdate {
                id: "root".into(),
                old_title: "Old title".into(),
                new_title: "New title".into(),
            }],
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
