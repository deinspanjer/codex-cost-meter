mod cli;
mod output;
mod pricing;
mod report;
mod rollout;
mod session_index;
mod title;

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
    }
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
