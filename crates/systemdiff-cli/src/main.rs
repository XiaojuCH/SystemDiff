#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use systemdiff_core::Snapshot;
use systemdiff_diff::{DiffOptions, diff_snapshots};
use systemdiff_report::{write_json, write_terminal};
use systemdiff_windows::mvp_collector_plans;

#[derive(Debug, Parser)]
#[command(
    name = "systemdiff",
    version,
    about = "Compare versioned Windows system evidence",
    long_about = "SystemDiff is in bootstrap. The diff command works with draft synthetic snapshots; operating-system collection is not implemented yet."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Compare two draft SystemDiff snapshot files.
    Diff {
        /// Render the versioned JSON diff instead of terminal text.
        #[arg(long)]
        json: bool,

        /// Include observations that did not change.
        #[arg(long)]
        include_unchanged: bool,

        /// Snapshot captured before the observed action.
        before: PathBuf,

        /// Snapshot captured after the observed action.
        after: PathBuf,
    },

    /// List the planned MVP Collectors and implementation state.
    Collectors,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Command::Diff {
            json,
            include_unchanged,
            before,
            after,
        } => {
            let before = load_snapshot(&before)?;
            let after = load_snapshot(&after)?;
            let diff = diff_snapshots(&before, &after, DiffOptions { include_unchanged })?;

            let stdout = io::stdout();
            let mut output = stdout.lock();
            if json {
                write_json(&mut output, &diff)?;
            } else {
                write_terminal(&mut output, &diff)?;
            }
        }
        Command::Collectors => {
            for plan in mvp_collector_plans() {
                println!(
                    "{} v{}: {:?} — {}",
                    plan.descriptor.id,
                    plan.descriptor.version,
                    plan.implementation,
                    plan.descriptor.description
                );
            }
        }
    }
    Ok(())
}

fn load_snapshot(path: &Path) -> Result<Snapshot, CliError> {
    let bytes = fs::read(path).map_err(|error| {
        CliError(format!(
            "failed to read snapshot {}: {error}",
            path.display()
        ))
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        CliError(format!(
            "failed to parse snapshot {}: {error}",
            path.display()
        ))
    })
}

#[derive(Debug)]
struct CliError(String);

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for CliError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_diff_command() {
        let cli =
            Cli::try_parse_from(["systemdiff", "diff", "--json", "before.json", "after.json"])
                .expect("diff command must parse");

        assert!(matches!(
            cli.command,
            Command::Diff {
                json: true,
                include_unchanged: false,
                ..
            }
        ));
    }

    #[test]
    fn snapshot_command_is_not_advertised_before_collectors_exist() {
        assert!(Cli::try_parse_from(["systemdiff", "snapshot"]).is_err());
    }
}
