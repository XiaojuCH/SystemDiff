#![forbid(unsafe_code)]

use serde::Serialize;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::Write as FmtWrite;
use std::io::{self, Write as IoWrite};
use systemdiff_diff::DiffDocument;

pub fn write_json<W: IoWrite, T: Serialize>(mut writer: W, value: &T) -> Result<(), ReportError> {
    serde_json::to_writer_pretty(&mut writer, value).map_err(ReportError::Json)?;
    writer.write_all(b"\n").map_err(ReportError::Io)
}

pub fn render_terminal(diff: &DiffDocument) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "SystemDiff diff");
    let _ = writeln!(output, "Before: {}", diff.before_captured_at);
    let _ = writeln!(output, "After:  {}", diff.after_captured_at);

    let counts = diff
        .changes
        .iter()
        .fold(BTreeMap::new(), |mut counts, item| {
            *counts.entry(item.change.label()).or_insert(0_usize) += 1;
            counts
        });
    if counts.is_empty() {
        let _ = writeln!(output, "Changes: none");
    } else {
        let summary = counts
            .iter()
            .map(|(label, count)| format!("{label}={count}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(output, "Changes: {summary}");
    }

    for item in &diff.changes {
        let _ = writeln!(
            output,
            "{} {}",
            item.change.label().to_uppercase(),
            item.key
        );
    }

    if !diff.warnings.is_empty() {
        let _ = writeln!(output, "Warnings:");
        for warning in &diff.warnings {
            let _ = writeln!(
                output,
                "  coverage incomplete for {}/{} (before={:?}, after={:?})",
                warning.collector_id, warning.scope_id, warning.before_status, warning.after_status
            );
        }
    }

    output
}

pub fn write_terminal<W: IoWrite>(mut writer: W, diff: &DiffDocument) -> io::Result<()> {
    writer.write_all(render_terminal(diff).as_bytes())
}

#[derive(Debug)]
pub enum ReportError {
    Json(serde_json::Error),
    Io(io::Error),
}

impl fmt::Display for ReportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "failed to serialize JSON report: {error}"),
            Self::Io(error) => write!(formatter, "failed to write report: {error}"),
        }
    }
}

impl Error for ReportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Io(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_diff() -> DiffDocument {
        DiffDocument {
            document_type: "systemdiff.diff".to_owned(),
            schema_version: 1,
            before_captured_at: "2026-08-11T00:00:00Z".to_owned(),
            after_captured_at: "2026-08-11T00:05:00Z".to_owned(),
            changes: Vec::new(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn json_output_is_pretty_and_newline_terminated() {
        let mut output = Vec::new();
        write_json(&mut output, &empty_diff()).expect("JSON report must render");
        let output = String::from_utf8(output).expect("JSON report must be UTF-8");

        assert!(output.contains("\n  \"document_type\""));
        assert!(output.ends_with('\n'));
    }

    #[test]
    fn terminal_output_is_plain_and_stable() {
        let output = render_terminal(&empty_diff());
        assert_eq!(
            output,
            concat!(
                "SystemDiff diff\n",
                "Before: 2026-08-11T00:00:00Z\n",
                "After:  2026-08-11T00:05:00Z\n",
                "Changes: none\n"
            )
        );
    }
}
