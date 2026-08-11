#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use systemdiff_core::{Snapshot, decode_snapshot_document};
use systemdiff_diff::{DiffOptions, diff_snapshots};
use systemdiff_report::{write_json, write_terminal};
use systemdiff_windows::mvp_collector_plans;

const MAX_SNAPSHOT_INPUT_BYTES: u64 = 64 * 1024 * 1024;

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
    load_snapshot_with_limit(path, MAX_SNAPSHOT_INPUT_BYTES)
}

fn load_snapshot_with_limit(path: &Path, maximum_bytes: u64) -> Result<Snapshot, CliError> {
    let file = File::open(path).map_err(|error| {
        CliError(format!(
            "failed to open snapshot {}: {error}",
            path.display()
        ))
    })?;

    let metadata = file.metadata().map_err(|error| {
        CliError(format!(
            "failed to inspect snapshot {}: {error}",
            path.display()
        ))
    })?;
    validate_snapshot_input_size(path, metadata.len(), maximum_bytes)?;

    let initial_capacity = usize::try_from(metadata.len()).map_err(|_| {
        CliError(format!(
            "snapshot file {} cannot be represented on this platform",
            path.display()
        ))
    })?;
    let read_limit = maximum_bytes.saturating_add(1);
    let mut bytes = Vec::with_capacity(initial_capacity);
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            CliError(format!(
                "failed to read snapshot {}: {error}",
                path.display()
            ))
        })?;
    validate_snapshot_input_size(path, bytes.len() as u64, maximum_bytes)?;

    decode_snapshot_document(&bytes).map_err(|error| {
        CliError(format!(
            "failed to parse snapshot {}: {error}",
            path.display()
        ))
    })
}

fn validate_snapshot_input_size(
    path: &Path,
    actual_bytes: u64,
    maximum_bytes: u64,
) -> Result<(), CliError> {
    if actual_bytes > maximum_bytes {
        return Err(CliError(format!(
            "snapshot file {} is too large: {actual_bytes} bytes; maximum supported size is {maximum_bytes} bytes",
            path.display()
        )));
    }
    Ok(())
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
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn write_temp_snapshot(contents: &[u8]) -> PathBuf {
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "systemdiff-cli-test-{}-{sequence}.json",
            std::process::id()
        ));
        fs::write(&path, contents).expect("temporary snapshot must be written");
        path
    }

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

    #[test]
    fn snapshot_input_size_below_limit_is_accepted() {
        assert!(validate_snapshot_input_size(Path::new("snapshot.json"), 63, 64).is_ok());
    }

    #[test]
    fn snapshot_input_size_exactly_at_limit_is_accepted() {
        assert!(validate_snapshot_input_size(Path::new("snapshot.json"), 64, 64).is_ok());
    }

    #[test]
    fn snapshot_input_size_above_limit_is_rejected() {
        let error = validate_snapshot_input_size(Path::new("snapshot.json"), 65, 64)
            .expect_err("an oversized snapshot must be rejected");

        assert!(error.0.contains("is too large: 65 bytes"));
        assert!(error.0.contains("maximum supported size is 64 bytes"));
    }

    #[test]
    fn oversized_input_is_rejected_before_snapshot_deserialization() {
        let path = write_temp_snapshot(b"{}");
        let result = load_snapshot_with_limit(&path, 1);
        fs::remove_file(&path).expect("temporary snapshot must be removed");

        let error = result.expect_err("metadata above the limit must stop loading");
        assert!(error.0.contains("is too large"));
        assert!(!error.0.contains("parse snapshot"));
    }
}
