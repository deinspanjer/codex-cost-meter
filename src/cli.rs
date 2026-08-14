use std::{env, path::PathBuf};

use clap::{Args, Parser, Subcommand};
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(name = "codex-cost-meter")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Report(ReportArgs),
}

#[derive(Debug, Args)]
pub(crate) struct ReportArgs {
    pub(crate) thread_id: String,
    #[arg(long)]
    pub(crate) json: bool,
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
        if let Some(home) = &self.codex_home {
            return Ok(home.clone());
        }
        if let Some(home) = env::var_os("CODEX_HOME") {
            return Ok(home.into());
        }
        let home = env::var_os("HOME").ok_or(CliError::HomeNotSet)?;
        Ok(PathBuf::from(home).join(".codex"))
    }
}
