#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use std::error::Error;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use systemdiff_core::{Snapshot, decode_snapshot_document};
use systemdiff_diff::{DiffOptions, diff_snapshots};
use systemdiff_report::{write_json, write_terminal};
use systemdiff_windows::{capture_snapshot, mvp_collector_plans};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const MAX_SNAPSHOT_INPUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SNAPSHOT_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(
    name = "systemdiff",
    version,
    about = "Compare versioned Windows system evidence",
    long_about = "Capture documented Windows Run/RunOnce startup evidence and compare versioned before/after Snapshots."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Capture the currently implemented Windows evidence Collectors.
    Snapshot {
        /// New Snapshot file to create. Existing files are never overwritten.
        #[arg(short = 'o', long)]
        output: PathBuf,
    },

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
        Command::Snapshot { output } => {
            let captured_at = OffsetDateTime::now_utc().format(&Rfc3339)?;
            if !captured_at.ends_with('Z') {
                return Err(Box::new(CliError(
                    "internal timestamp formatter did not produce canonical UTC Z".to_owned(),
                )));
            }
            let snapshot = capture_snapshot(captured_at, env!("CARGO_PKG_VERSION").to_owned())?;
            let bytes = serialize_snapshot_with_limit(&snapshot, MAX_SNAPSHOT_OUTPUT_BYTES)?;
            create_snapshot_file(&output, &bytes)?;
            println!("Created Snapshot: {}", output.display());
        }
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

fn serialize_snapshot_with_limit(
    snapshot: &Snapshot,
    maximum_bytes: usize,
) -> Result<Vec<u8>, CliError> {
    let mut output = CappedBuffer::new(maximum_bytes);
    if let Err(error) = write_json(&mut output, snapshot) {
        if output.exceeded {
            return Err(CliError(format!(
                "generated Snapshot exceeds the maximum supported size of {maximum_bytes} bytes"
            )));
        }
        return Err(CliError(format!(
            "failed to serialize generated Snapshot: {error}"
        )));
    }
    Ok(output.bytes)
}

fn create_snapshot_file(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            CliError(format!(
                "failed to create new Snapshot {}: {error}",
                path.display()
            ))
        })?;
    write_created_output(&mut file, bytes).map_err(|error| {
        CliError(format!(
            "failed to finish Snapshot {}: {error}; the newly created file may be incomplete and was not deleted automatically",
            path.display()
        ))
    })
}

fn write_created_output<W: Write>(writer: &mut W, bytes: &[u8]) -> io::Result<()> {
    writer.write_all(bytes)?;
    writer.flush()
}

struct CappedBuffer {
    bytes: Vec<u8>,
    maximum_bytes: usize,
    exceeded: bool,
}

impl CappedBuffer {
    fn new(maximum_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum_bytes,
            exceeded: false,
        }
    }
}

impl Write for CappedBuffer {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let Some(next_length) = self.bytes.len().checked_add(buffer.len()) else {
            self.exceeded = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Snapshot output size overflow",
            ));
        };
        if next_length > self.maximum_bytes {
            self.exceeded = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Snapshot output exceeds configured limit",
            ));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
    fn parses_snapshot_output_command() {
        let cli = Cli::try_parse_from(["systemdiff", "snapshot", "-o", "snapshot.json"])
            .expect("snapshot command must parse");
        assert!(matches!(
            cli.command,
            Command::Snapshot { ref output } if output == Path::new("snapshot.json")
        ));
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

    #[test]
    fn generated_snapshot_is_bounded_before_file_creation() {
        let snapshot: Snapshot =
            serde_json::from_slice(include_bytes!("../../../fixtures/snapshots/before-v1.json"))
                .expect("fixture must deserialize");
        let error = serialize_snapshot_with_limit(&snapshot, 1)
            .expect_err("a tiny generated-output limit must be enforced");
        assert!(error.0.contains("exceeds the maximum supported size"));
    }

    #[cfg(windows)]
    #[test]
    fn real_read_only_snapshot_serializes_and_reopens_with_explicit_registry_scopes() {
        let captured_at = "2026-08-11T00:00:00Z";
        let snapshot = capture_snapshot(captured_at.to_owned(), "0.0.0-test".to_owned())
            .expect("supported Windows test host must produce a read-only Snapshot");
        let bytes = serialize_snapshot_with_limit(&snapshot, MAX_SNAPSHOT_OUTPUT_BYTES)
            .expect("generated Snapshot must fit its own reader boundary");
        let reparsed = decode_snapshot_document(&bytes)
            .expect("generated Snapshot must reopen through header-first routing");

        assert_eq!(reparsed.captured_at, captured_at);
        assert_eq!(reparsed.enabled_collectors, ["windows.registry.startup"]);
        let registry = reparsed
            .collectors
            .iter()
            .find(|run| run.id == "windows.registry.startup")
            .expect("Registry startup Collector run must exist");
        assert!(registry.coverage.iter().any(|coverage| {
            coverage.scope_id == "current_user.shared.run"
                && matches!(
                    coverage.status,
                    systemdiff_core::CollectorStatus::Complete
                        | systemdiff_core::CollectorStatus::Partial
                        | systemdiff_core::CollectorStatus::PermissionDenied
                )
        }));
        assert!(
            registry
                .coverage
                .iter()
                .any(|coverage| coverage.scope_id == "current_user.shared.run_once")
        );
    }

    #[test]
    fn snapshot_output_never_overwrites_existing_file() {
        let path = write_temp_snapshot(b"sentinel");
        let error = create_snapshot_file(&path, b"replacement")
            .expect_err("an existing Snapshot path must be rejected");
        assert!(error.0.contains("failed to create new Snapshot"));
        assert_eq!(
            fs::read(&path).expect("sentinel must remain readable"),
            b"sentinel"
        );
        fs::remove_file(path).expect("temporary sentinel must be removed");
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("injected write failure"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn created_output_write_failure_is_reported_without_cleanup_side_effects() {
        let error = write_created_output(&mut FailingWriter, b"snapshot")
            .expect_err("injected write failure must be returned");
        assert_eq!(error.kind(), io::ErrorKind::Other);
    }
}
