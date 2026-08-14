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

use clap::{Parser, error::ErrorKind};
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
    run_with_writer(cli, &mut io::stdout().lock())
}

fn run_with_writer(cli: Cli, writer: &mut impl Write) -> Result<(), AppError> {
    match cli.command {
        Command::Report(args) => {
            let report = report::build(&args.thread_id, &args.codex_home()?)?;
            let rendered = if args.json {
                format!("{}\n", output::json(&report)?)
            } else {
                output::human(&report)
            };
            writer
                .write_all(rendered.as_bytes())
                .map_err(|source| AppError::WriteReport { source })?;
            Ok(())
        }
        Command::Update(args) => {
            let home = args.codex_home()?;
            let options = args.options();
            let result = update::run(&home, &options)?;
            write_update_output(writer, &result, options.apply)?;
            Ok(())
        }
    }
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
