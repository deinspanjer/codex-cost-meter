mod cli;
mod output;
mod pricing;
mod report;
mod rollout;
mod session_index;
mod title;
mod update;

use std::{
    error::Error,
    io::{self, Write},
    process::ExitCode,
};

use clap::Parser;
use thiserror::Error as ThisError;

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
    Update(#[from] update::UpdateError),
    #[error("could not render report: {0}")]
    Render(#[from] serde_json::Error),
    #[error("could not write report: {0}")]
    Write(#[from] io::Error),
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", error_chain(&error));
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), AppError> {
    match cli.command {
        Command::Report(args) => {
            let report = report::build(&args.thread_id, &args.codex_home()?)?;
            let rendered = if args.json {
                format!("{}\n", output::json(&report)?)
            } else {
                output::human(&report)
            };
            io::stdout().lock().write_all(rendered.as_bytes())?;
            Ok(())
        }
        Command::Update(args) => {
            let home = args.codex_home()?;
            let options = args.options();
            let result = update::run(&home, &options)?;
            let rendered = update_output(&result, options.apply);
            io::stdout().lock().write_all(rendered.as_bytes())?;
            Ok(())
        }
    }
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
