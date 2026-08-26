#![forbid(unsafe_code)]

pub mod presentation;

use serde::Serialize;
use std::error::Error;
use std::fmt;
use std::fmt::Write as FmtWrite;
use std::io::{self, Write as IoWrite};
use systemdiff_core::{
    Artifact, CollectorStatus, Diagnostic, RegistryDecodedValue, RegistryHive, RegistryRawEvidence,
    RegistryStartupEntry, RegistryStartupKind, RegistryValueDecoding, RegistryValueName,
    RegistryView, RunOncePrefixSemantics, Snapshot,
};
use systemdiff_diff::{ArtifactChange, ChangeKind, DiffDocument, DiffWarning};

pub fn write_json<W: IoWrite, T: Serialize>(mut writer: W, value: &T) -> Result<(), ReportError> {
    serde_json::to_writer_pretty(&mut writer, value).map_err(ReportError::Json)?;
    writer.write_all(b"\n").map_err(ReportError::Io)
}

/// Renders the calm, evidence-backed default terminal view.
///
/// Snapshot strings are untrusted evidence. Control characters are escaped so
/// they cannot inject terminal sequences or additional report lines.
pub fn render_terminal(diff: &DiffDocument) -> String {
    let mut output = String::new();
    let confirmed = diff
        .changes
        .iter()
        .filter(|item| {
            matches!(
                item.change,
                ChangeKind::Added { .. } | ChangeKind::Removed { .. } | ChangeKind::Modified { .. }
            )
        })
        .count();
    let inconclusive = diff
        .changes
        .iter()
        .filter(|item| matches!(item.change, ChangeKind::Inconclusive { .. }))
        .count();
    let unchanged = diff
        .changes
        .iter()
        .filter(|item| matches!(item.change, ChangeKind::Unchanged { .. }))
        .count();

    if diff.changes.is_empty() && diff.warnings.is_empty() {
        let _ = writeln!(output, "No changes found");
    } else {
        if confirmed == 0 {
            let _ = writeln!(output, "No confirmed changes");
        } else {
            let _ = writeln!(
                output,
                "{} confirmed {}",
                confirmed,
                plural(confirmed, "change", "changes")
            );
        }
        if inconclusive > 0 {
            let _ = writeln!(
                output,
                "Could not confirm {} possible {}",
                inconclusive,
                plural(inconclusive, "change", "changes")
            );
        }
        if unchanged > 0 {
            let _ = writeln!(
                output,
                "{} unchanged {} shown",
                unchanged,
                plural(unchanged, "entry", "entries")
            );
        }
    }
    let _ = writeln!(
        output,
        "Compared {} -> {}",
        terminal_text(&diff.before_captured_at),
        terminal_text(&diff.after_captured_at)
    );

    let registry_changes: Vec<_> = diff
        .changes
        .iter()
        .filter(|item| change_artifact(item).is_some_and(is_registry))
        .collect();
    if !registry_changes.is_empty() {
        let _ = writeln!(output, "\nRegistry startup changes\n");
        for (index, item) in registry_changes.into_iter().enumerate() {
            if index > 0 {
                let _ = writeln!(output);
            }
            render_human_registry_change(&mut output, item);
        }
    }

    let service_changes: Vec<_> = diff
        .changes
        .iter()
        .filter(|item| change_artifact(item).is_some_and(is_windows_service))
        .collect();
    if !service_changes.is_empty() {
        let _ = writeln!(output, "\nWindows service changes\n");
        for (index, item) in service_changes.into_iter().enumerate() {
            if index > 0 {
                let _ = writeln!(output);
            }
            render_human_service_change(&mut output, item);
        }
    }

    let other_changes: Vec<_> = diff
        .changes
        .iter()
        .filter(|item| {
            !change_artifact(item)
                .is_some_and(|artifact| is_registry(artifact) || is_windows_service(artifact))
        })
        .collect();
    if !other_changes.is_empty() {
        let _ = writeln!(output, "\nOther evidence changes\n");
        for item in other_changes {
            render_human_fallback(&mut output, item);
        }
    }

    if !diff.warnings.is_empty() {
        let _ = writeln!(
            output,
            "\nCoverage notes\n\n{} {} could not be fully checked",
            diff.warnings.len(),
            plural(diff.warnings.len(), "scope", "scopes")
        );
        for warning in &diff.warnings {
            render_human_warning(&mut output, warning);
        }
    }

    output
}

pub fn write_terminal<W: IoWrite>(mut writer: W, diff: &DiffDocument) -> io::Result<()> {
    writer.write_all(render_terminal(diff).as_bytes())
}

/// Renders exact evidence for power users and debugging without changing the
/// versioned JSON Diff document.
pub fn render_technical(diff: &DiffDocument, before: &Snapshot, after: &Snapshot) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "SystemDiff technical diff");
    let _ = writeln!(
        output,
        "Before: {}\nAfter:  {}",
        terminal_text(&diff.before_captured_at),
        terminal_text(&diff.after_captured_at)
    );
    let mut counts = [0_usize; 5];
    for item in &diff.changes {
        counts[change_index(&item.change)] += 1;
    }
    let _ = writeln!(
        output,
        "Changes: added={}, modified={}, removed={}, inconclusive={}, unchanged={}",
        counts[0], counts[1], counts[2], counts[3], counts[4]
    );

    for item in &diff.changes {
        let _ = writeln!(
            output,
            "\n{} {}",
            item.change.label().to_ascii_uppercase(),
            terminal_text(&item.change_id)
        );
        let _ = writeln!(
            output,
            "  collector ID: {}",
            terminal_text(&item.key.collector_id)
        );
        render_versions(&mut output, &item.key.collector_id, before, after);
        let _ = writeln!(output, "  scope: {}", terminal_text(&item.key.scope_id));
        let _ = writeln!(
            output,
            "  artifact kind: {}",
            terminal_text(&item.key.artifact_kind)
        );
        let _ = writeln!(
            output,
            "  canonical identity: {}",
            terminal_text(&item.key.canonical_id)
        );

        match &item.change {
            ChangeKind::Added { after } => render_technical_artifact(&mut output, "After", after),
            ChangeKind::Removed { before } => {
                render_technical_artifact(&mut output, "Before", before);
            }
            ChangeKind::Modified { before, after } => {
                render_technical_artifact(&mut output, "Before", before);
                render_technical_artifact(&mut output, "After", after);
            }
            ChangeKind::Unchanged { artifact } => {
                render_technical_artifact(&mut output, "Evidence", artifact);
            }
            ChangeKind::Inconclusive {
                before,
                after,
                reason,
            } => {
                let _ = writeln!(output, "  reason: {}", inconclusive_reason(*reason));
                if let Some(before) = before {
                    render_technical_artifact(&mut output, "Before", before);
                } else {
                    let _ = writeln!(output, "  Before: no observation");
                }
                if let Some(after) = after {
                    render_technical_artifact(&mut output, "After", after);
                } else {
                    let _ = writeln!(output, "  After: no observation");
                }
            }
        }
    }

    let _ = writeln!(output, "\nCoverage");
    render_snapshot_coverage(&mut output, "Before Snapshot", before);
    render_snapshot_coverage(&mut output, "After Snapshot", after);
    let _ = writeln!(output, "Diff coverage warnings:");
    if diff.warnings.is_empty() {
        let _ = writeln!(output, "  none");
    } else {
        for warning in &diff.warnings {
            let _ = writeln!(
                output,
                "  {}/{}: before={}, after={}",
                terminal_text(&warning.collector_id),
                terminal_text(&warning.scope_id),
                optional_status(warning.before_status),
                optional_status(warning.after_status)
            );
        }
    }
    render_snapshot_diagnostics(&mut output, "Before diagnostics", before);
    render_snapshot_diagnostics(&mut output, "After diagnostics", after);

    output
}

fn render_snapshot_coverage(output: &mut String, heading: &str, snapshot: &Snapshot) {
    let mut collectors: Vec<_> = snapshot.collectors.iter().collect();
    collectors.sort_by(|left, right| left.id.cmp(&right.id));
    let _ = writeln!(output, "{heading} collector coverage:");
    if collectors.is_empty() {
        let _ = writeln!(output, "  none");
        return;
    }
    for collector in collectors {
        let _ = writeln!(
            output,
            "  collector ID: {}\n    version: {}\n    aggregate status: {}",
            terminal_text(&collector.id),
            collector.version,
            status(collector.status)
        );
        let mut scopes: Vec<_> = collector.coverage.iter().collect();
        scopes.sort_by(|left, right| left.scope_id.cmp(&right.scope_id));
        for scope in scopes {
            let _ = writeln!(
                output,
                "    scope {}: {}",
                terminal_text(&scope.scope_id),
                status(scope.status)
            );
        }
    }
}

pub fn write_technical<W: IoWrite>(
    mut writer: W,
    diff: &DiffDocument,
    before: &Snapshot,
    after: &Snapshot,
) -> io::Result<()> {
    writer.write_all(render_technical(diff, before, after).as_bytes())
}

fn render_human_registry_change(output: &mut String, item: &ArtifactChange) {
    match &item.change {
        ChangeKind::Added {
            after: Artifact::RegistryStartup(entry),
        } => {
            human_entry_heading(output, '+', "Added", entry);
            let _ = writeln!(
                output,
                "    Added to {} {}",
                human_hive(entry.hive),
                human_startup_location(entry.startup_kind)
            );
            render_human_value(output, "Value", entry);
            render_human_location(output, entry);
        }
        ChangeKind::Removed {
            before: Artifact::RegistryStartup(entry),
        } => {
            human_entry_heading(output, '-', "Removed", entry);
            let _ = writeln!(
                output,
                "    Removed from {} {}",
                human_hive(entry.hive),
                human_startup_location(entry.startup_kind)
            );
            render_human_value(output, "Previous value", entry);
            render_human_location(output, entry);
        }
        ChangeKind::Modified {
            before: Artifact::RegistryStartup(before),
            after: Artifact::RegistryStartup(after),
        } => {
            human_entry_heading(output, '~', "Modified", after);
            if matches!(
                (decoded_command(before), decoded_command(after)),
                (Some(before), Some(after)) if before != after
            ) {
                let _ = writeln!(output, "    Startup command changed");
            } else {
                let _ = writeln!(output, "    Registry startup evidence changed");
            }
            render_human_value(output, "Before", before);
            render_human_value(output, "After", after);
            render_human_location(output, after);
        }
        ChangeKind::Unchanged {
            artifact: Artifact::RegistryStartup(entry),
        } => {
            human_entry_heading(output, '=', "Unchanged", entry);
            let _ = writeln!(output, "    No change in the captured Registry evidence");
            render_human_value(output, "Value", entry);
            render_human_location(output, entry);
        }
        ChangeKind::Inconclusive { before, after, .. } => {
            let entry = before.as_ref().or(after.as_ref()).and_then(registry_entry);
            if let Some(entry) = entry {
                human_entry_heading(output, '?', "Inconclusive", entry);
                let _ = writeln!(
                    output,
                    "    The corresponding Registry scope had incomplete coverage, so this change could not be confirmed."
                );
                render_human_value(output, "Observed value", entry);
                render_human_location(output, entry);
            }
        }
        _ => render_human_fallback(output, item),
    }
}

fn render_human_service_change(output: &mut String, item: &ArtifactChange) {
    match &item.change {
        ChangeKind::Added {
            after: Artifact::WindowsService(service),
        } => {
            human_service_heading(output, '+', "Added", service);
            render_human_service_summary(output, service);
        }
        ChangeKind::Removed {
            before: Artifact::WindowsService(service),
        } => {
            human_service_heading(output, '-', "Removed", service);
            render_human_service_summary(output, service);
        }
        ChangeKind::Modified {
            before: Artifact::WindowsService(before),
            after: Artifact::WindowsService(after),
        } => {
            human_service_heading(output, '~', "Modified", after);
            render_human_service_modifications(output, before, after);
        }
        ChangeKind::Unchanged {
            artifact: Artifact::WindowsService(service),
        } => {
            human_service_heading(output, '=', "Unchanged", service);
            let _ = writeln!(
                output,
                "    No change in the captured service configuration"
            );
        }
        ChangeKind::Inconclusive { before, after, .. } => {
            let service = before.as_ref().or(after.as_ref()).and_then(service_entry);
            if let Some(service) = service {
                human_service_heading(output, '?', "Inconclusive", service);
                let explanation = match (before.is_some(), after.is_some()) {
                    (true, false) => {
                        "Current-token service coverage was incomplete in the after Snapshot, so removal could not be confirmed."
                    }
                    (false, true) => {
                        "Current-token service coverage was incomplete in the before Snapshot, so addition could not be confirmed."
                    }
                    _ => {
                        "Current-token service coverage was incomplete, so this change could not be confirmed."
                    }
                };
                let _ = writeln!(output, "    {explanation}");
                render_human_service_summary(output, service);
            }
        }
        _ => render_human_fallback(output, item),
    }
}

fn human_service_heading(
    output: &mut String,
    symbol: char,
    change: &str,
    service: &systemdiff_core::WindowsService,
) {
    let _ = writeln!(output, "  {symbol} {}", human_service_name(service));
    let _ = writeln!(output, "    {change} (Windows service)");
    let _ = writeln!(
        output,
        "    Service name: {}",
        terminal_text(&service.service_name)
    );
}

fn render_human_service_summary(output: &mut String, service: &systemdiff_core::WindowsService) {
    let _ = writeln!(
        output,
        "    Start: {}\n    Binary path: {}\n    Account: {}",
        human_service_start(service.start_type, service.delayed_auto_start),
        terminal_text(&service.binary_path),
        human_optional_text(service.account.as_deref())
    );
}

fn render_human_service_modifications(
    output: &mut String,
    before: &systemdiff_core::WindowsService,
    after: &systemdiff_core::WindowsService,
) {
    if before.service_name != after.service_name {
        render_human_changed_text(
            output,
            "Service name",
            &before.service_name,
            &after.service_name,
        );
    }
    if before.display_name != after.display_name {
        render_human_changed_optional_text(
            output,
            "Display name",
            before.display_name.as_deref(),
            after.display_name.as_deref(),
        );
    }
    if before.service_type != after.service_type {
        render_human_changed_value(
            output,
            "Service type",
            before.service_type,
            after.service_type,
        );
    }
    if before.start_type != after.start_type {
        render_human_changed_display(
            output,
            "Start",
            &human_service_start(before.start_type, before.delayed_auto_start),
            &human_service_start(after.start_type, after.delayed_auto_start),
        );
    }
    if before.delayed_auto_start != after.delayed_auto_start {
        render_human_changed_display(
            output,
            "Delayed automatic start configured",
            human_bool(before.delayed_auto_start),
            human_bool(after.delayed_auto_start),
        );
    }
    if before.error_control != after.error_control {
        render_human_changed_display(
            output,
            "Error control",
            &human_error_control(before.error_control),
            &human_error_control(after.error_control),
        );
    }
    if before.binary_path != after.binary_path {
        render_human_changed_text(
            output,
            "Binary path",
            &before.binary_path,
            &after.binary_path,
        );
    }
    if before.account != after.account {
        render_human_changed_optional_text(
            output,
            "Account",
            before.account.as_deref(),
            after.account.as_deref(),
        );
    }
    if before.dependencies != after.dependencies {
        render_human_changed_display(
            output,
            "Dependencies",
            &human_dependencies(&before.dependencies),
            &human_dependencies(&after.dependencies),
        );
    }
    if before.load_order_group != after.load_order_group {
        render_human_changed_optional_text(
            output,
            "Load-order group",
            before.load_order_group.as_deref(),
            after.load_order_group.as_deref(),
        );
    }
    if before.tag_id != after.tag_id {
        render_human_changed_display(
            output,
            "Tag ID",
            &human_optional_u32(before.tag_id),
            &human_optional_u32(after.tag_id),
        );
    }
    if before.description != after.description {
        render_human_changed_optional_text(
            output,
            "Description",
            before.description.as_deref(),
            after.description.as_deref(),
        );
    }
}

fn render_human_changed_text(output: &mut String, label: &str, before: &str, after: &str) {
    render_human_changed_display(output, label, &terminal_text(before), &terminal_text(after));
}

fn render_human_changed_optional_text(
    output: &mut String,
    label: &str,
    before: Option<&str>,
    after: Option<&str>,
) {
    render_human_changed_display(
        output,
        label,
        &human_optional_text(before),
        &human_optional_text(after),
    );
}

fn render_human_changed_value<T: fmt::Display>(
    output: &mut String,
    label: &str,
    before: T,
    after: T,
) {
    render_human_changed_display(output, label, &before.to_string(), &after.to_string());
}

fn render_human_changed_display(output: &mut String, label: &str, before: &str, after: &str) {
    let _ = writeln!(
        output,
        "    {label}:\n      Before: {before}\n      After:  {after}"
    );
}

fn human_entry_heading(
    output: &mut String,
    symbol: char,
    change: &str,
    entry: &RegistryStartupEntry,
) {
    let _ = writeln!(output, "  {symbol} {}", human_value_name(&entry.value_name));
    let _ = writeln!(
        output,
        "    {change} ({})",
        startup_kind(entry.startup_kind)
    );
}

fn render_human_value(output: &mut String, heading: &str, entry: &RegistryStartupEntry) {
    match &entry.decoding {
        RegistryValueDecoding::Decoded { value } => {
            let label = if matches!(
                value,
                RegistryDecodedValue::String { .. } | RegistryDecodedValue::ExpandString { .. }
            ) {
                if heading == "Value" {
                    "Command"
                } else {
                    heading
                }
            } else {
                heading
            };
            let _ = writeln!(output, "\n    {label}\n      {}", human_decoded(value));
        }
        RegistryValueDecoding::InvalidData => {
            let _ = writeln!(
                output,
                "\n    Registry value data was not decoded because it is invalid for its native type."
            );
        }
        RegistryValueDecoding::UnsupportedType => {
            let _ = writeln!(
                output,
                "\n    Registry value data was not decoded because its native type is not supported."
            );
        }
        RegistryValueDecoding::NotApplicable => {
            let _ = writeln!(
                output,
                "\n    Registry value data was not decoded because this native type has no typed representation."
            );
        }
    }
}

fn render_human_location(output: &mut String, entry: &RegistryStartupEntry) {
    let _ = writeln!(
        output,
        "\n    Location\n      {}\\{}{}",
        hive_abbreviation(entry.hive),
        terminal_text(&entry.key_path),
        human_view_suffix(entry.registry_view)
    );
}

fn render_human_fallback(output: &mut String, item: &ArtifactChange) {
    let artifact = change_artifact(item);
    let name = artifact.map(human_artifact_name).unwrap_or_else(|| {
        format!(
            "{} evidence",
            terminal_text(&item.key.artifact_kind).replace('_', " ")
        )
    });
    let symbol = match item.change {
        ChangeKind::Added { .. } => '+',
        ChangeKind::Removed { .. } => '-',
        ChangeKind::Modified { .. } => '~',
        ChangeKind::Unchanged { .. } => '=',
        ChangeKind::Inconclusive { .. } => '?',
    };
    let _ = writeln!(
        output,
        "  {symbol} {name}\n    {}",
        human_change_label(&item.change)
    );
}

fn render_human_warning(output: &mut String, warning: &DiffWarning) {
    let _ = writeln!(
        output,
        "  ! {}\n    Before: {}\n    After:  {}",
        human_scope_label(&warning.collector_id, &warning.scope_id),
        optional_status(warning.before_status),
        optional_status(warning.after_status)
    );
}

fn render_versions(output: &mut String, collector_id: &str, before: &Snapshot, after: &Snapshot) {
    let before_version = collector_version(before, collector_id);
    let after_version = collector_version(after, collector_id);
    if before_version == after_version {
        if let Some(version) = before_version {
            let _ = writeln!(output, "  version: {version}");
        } else {
            let _ = writeln!(output, "  version: unavailable in both Snapshots");
        }
    } else {
        let _ = writeln!(
            output,
            "  before version: {}",
            optional_version(before_version)
        );
        let _ = writeln!(
            output,
            "  after version: {}",
            optional_version(after_version)
        );
    }
}

fn render_technical_artifact(output: &mut String, heading: &str, artifact: &Artifact) {
    let _ = writeln!(output, "  {heading} evidence:");
    match artifact {
        Artifact::RegistryStartup(entry) => render_technical_registry(output, entry),
        Artifact::WindowsService(service) => {
            render_technical_service(output, service);
        }
        Artifact::ScheduledTask(task) => {
            let _ = writeln!(
                output,
                "    task path: {}\n    enabled: {}\n    hidden: {}\n    action count: {}",
                terminal_text(&task.task_path),
                task.enabled,
                task.hidden,
                task.actions.len()
            );
        }
    }
}

fn render_technical_service(output: &mut String, service: &systemdiff_core::WindowsService) {
    let _ = writeln!(
        output,
        "    service name: {}\n    display name: {}\n    service type: {}\n    start type: {} ({})\n    error control: {} ({})\n    binary path: {}\n    account: {}",
        terminal_text(&service.service_name),
        technical_optional_literal(service.display_name.as_deref()),
        service.service_type,
        service.start_type,
        technical_start_type(service.start_type),
        service.error_control,
        technical_error_control(service.error_control),
        terminal_text(&service.binary_path),
        technical_optional_literal(service.account.as_deref())
    );
    if service.dependencies.is_empty() {
        let _ = writeln!(output, "    dependencies: none");
    } else {
        let _ = writeln!(output, "    dependencies ({}):", service.dependencies.len());
        for (index, dependency) in service.dependencies.iter().enumerate() {
            let _ = writeln!(output, "      [{index}]: {}", terminal_text(dependency));
        }
    }
    let _ = writeln!(
        output,
        "    load-order group: {}\n    tag ID: {}\n    delayed auto-start: {}\n    description: {}",
        technical_optional_literal(service.load_order_group.as_deref()),
        service
            .tag_id
            .map(|tag| tag.to_string())
            .unwrap_or_else(|| "none".to_owned()),
        service.delayed_auto_start,
        technical_optional_literal(service.description.as_deref())
    );
}

fn render_technical_registry(output: &mut String, entry: &RegistryStartupEntry) {
    let _ = writeln!(
        output,
        "    Registry hive: {}\n    Registry view: {}\n    Registry path: {}\\{}\n    startup kind: {}",
        technical_hive(entry.hive),
        technical_view(entry.registry_view),
        hive_abbreviation(entry.hive),
        terminal_text(&entry.key_path),
        technical_startup_kind(entry.startup_kind)
    );
    match &entry.value_name {
        RegistryValueName::Decoded { value } => {
            let _ = writeln!(
                output,
                "    value name encoding: decoded\n    value name: {}",
                if value.is_empty() {
                    "<empty>".to_owned()
                } else {
                    terminal_text(value)
                }
            );
        }
        RegistryValueName::InvalidUtf16 { utf16le_hex } => {
            let _ = writeln!(
                output,
                "    value name encoding: invalid_utf16\n    value name UTF-16LE hex: {}",
                terminal_text(utf16le_hex)
            );
        }
    }
    let _ = writeln!(
        output,
        "    RunOnce prefix: {}\n    value type: {} ({})\n    decode status: {}",
        entry
            .run_once_prefix
            .map(technical_prefix)
            .unwrap_or("not_applicable"),
        entry.value_type,
        registry_type_name(entry.value_type),
        decode_status(&entry.decoding)
    );
    if let RegistryValueDecoding::Decoded { value } = &entry.decoding {
        let _ = writeln!(output, "    decoded value: {}", technical_decoded(value));
    }
    let _ = writeln!(output, "    SHA-256: {}", entry.content_sha256);
    match &entry.raw_evidence {
        Some(raw) => render_raw_evidence(output, raw),
        None => {
            let _ = writeln!(output, "    raw evidence: none");
        }
    }
}

fn render_raw_evidence(output: &mut String, raw: &RegistryRawEvidence) {
    let _ = writeln!(
        output,
        "    raw evidence: {}\n    captured bytes: {}\n    original bytes: {}\n    truncated: {}",
        terminal_text(&raw.content_hex),
        raw.captured_byte_count,
        raw.original_byte_count,
        raw.truncated
    );
}

fn render_snapshot_diagnostics(output: &mut String, heading: &str, snapshot: &Snapshot) {
    let mut diagnostics: Vec<(&str, &Diagnostic)> = snapshot
        .collectors
        .iter()
        .flat_map(|run| {
            run.diagnostics
                .iter()
                .map(move |diagnostic| (run.id.as_str(), diagnostic))
        })
        .collect();
    diagnostics.sort_by(|left, right| {
        (
            left.0,
            &left.1.scope_id,
            &left.1.code,
            &left.1.stage,
            left.1.native_code,
            &left.1.message,
        )
            .cmp(&(
                right.0,
                &right.1.scope_id,
                &right.1.code,
                &right.1.stage,
                right.1.native_code,
                &right.1.message,
            ))
    });
    let _ = writeln!(output, "{heading}:");
    if diagnostics.is_empty() {
        let _ = writeln!(output, "  none");
        return;
    }
    for (collector_id, diagnostic) in diagnostics {
        let _ = writeln!(
            output,
            "  collector ID: {}\n    scope: {}\n    code: {}\n    message: {}\n    stage: {}\n    native code: {}",
            terminal_text(collector_id),
            diagnostic
                .scope_id
                .as_deref()
                .map(terminal_text)
                .unwrap_or_else(|| "collector-wide".to_owned()),
            terminal_text(&diagnostic.code),
            terminal_text(&diagnostic.message),
            diagnostic
                .stage
                .as_deref()
                .map(terminal_text)
                .unwrap_or_else(|| "none".to_owned()),
            diagnostic
                .native_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "none".to_owned())
        );
    }
}

fn change_artifact(change: &ArtifactChange) -> Option<&Artifact> {
    match &change.change {
        ChangeKind::Added { after } => Some(after),
        ChangeKind::Removed { before } => Some(before),
        ChangeKind::Modified { after, .. } => Some(after),
        ChangeKind::Unchanged { artifact } => Some(artifact),
        ChangeKind::Inconclusive { before, after, .. } => before.as_ref().or(after.as_ref()),
    }
}

fn registry_entry(artifact: &Artifact) -> Option<&RegistryStartupEntry> {
    match artifact {
        Artifact::RegistryStartup(entry) => Some(entry),
        _ => None,
    }
}

fn service_entry(artifact: &Artifact) -> Option<&systemdiff_core::WindowsService> {
    match artifact {
        Artifact::WindowsService(service) => Some(service),
        _ => None,
    }
}

fn is_registry(artifact: &Artifact) -> bool {
    matches!(artifact, Artifact::RegistryStartup(_))
}

fn is_windows_service(artifact: &Artifact) -> bool {
    matches!(artifact, Artifact::WindowsService(_))
}

fn human_artifact_name(artifact: &Artifact) -> String {
    match artifact {
        Artifact::RegistryStartup(entry) => human_value_name(&entry.value_name),
        Artifact::WindowsService(service) => service
            .display_name
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(terminal_text)
            .unwrap_or_else(|| terminal_text(&service.service_name)),
        Artifact::ScheduledTask(task) => terminal_text(&task.task_path),
    }
}

fn human_service_name(service: &systemdiff_core::WindowsService) -> String {
    service
        .display_name
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(terminal_text)
        .unwrap_or_else(|| terminal_text(&service.service_name))
}

fn human_service_start(start_type: u32, delayed_auto_start: bool) -> String {
    match start_type {
        0 => "Boot start".to_owned(),
        1 => "System start".to_owned(),
        2 if delayed_auto_start => "Automatic (delayed start)".to_owned(),
        2 => "Automatic".to_owned(),
        3 => "Manual (on demand)".to_owned(),
        4 => "Disabled".to_owned(),
        value => format!("Unknown (raw start type {value})"),
    }
}

fn human_error_control(error_control: u32) -> String {
    match error_control {
        0 => "Ignore (raw value 0)".to_owned(),
        1 => "Normal (raw value 1)".to_owned(),
        2 => "Severe (raw value 2)".to_owned(),
        3 => "Critical (raw value 3)".to_owned(),
        value => format!("Unknown (raw value {value})"),
    }
}

fn human_optional_text(value: Option<&str>) -> String {
    value
        .map(terminal_text)
        .unwrap_or_else(|| "Not set".to_owned())
}

fn human_optional_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "Not set".to_owned())
}

fn human_bool(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

fn human_dependencies(dependencies: &[String]) -> String {
    if dependencies.is_empty() {
        "None".to_owned()
    } else {
        dependencies
            .iter()
            .map(|dependency| terminal_text(dependency))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn technical_optional_literal(value: Option<&str>) -> String {
    value
        .map(technical_literal)
        .unwrap_or_else(|| "none".to_owned())
}

fn technical_start_type(start_type: u32) -> &'static str {
    match start_type {
        0 => "boot",
        1 => "system",
        2 => "automatic",
        3 => "manual/on_demand",
        4 => "disabled",
        _ => "unknown native value",
    }
}

fn technical_error_control(error_control: u32) -> &'static str {
    match error_control {
        0 => "ignore",
        1 => "normal",
        2 => "severe",
        3 => "critical",
        _ => "unknown native value",
    }
}

fn human_value_name(name: &RegistryValueName) -> String {
    match name {
        RegistryValueName::Decoded { value } if value.is_empty() => {
            "Default value (unnamed)".to_owned()
        }
        RegistryValueName::Decoded { value } => terminal_text(value),
        RegistryValueName::InvalidUtf16 { .. } => "Name could not be decoded as UTF-16".to_owned(),
    }
}

fn decoded_command(entry: &RegistryStartupEntry) -> Option<&str> {
    match &entry.decoding {
        RegistryValueDecoding::Decoded {
            value:
                RegistryDecodedValue::String { value } | RegistryDecodedValue::ExpandString { value },
        } => Some(value),
        _ => None,
    }
}

fn human_decoded(value: &RegistryDecodedValue) -> String {
    match value {
        RegistryDecodedValue::String { value } | RegistryDecodedValue::ExpandString { value } => {
            terminal_text(value)
        }
        RegistryDecodedValue::MultiString { values } => values
            .iter()
            .map(|value| terminal_text(value))
            .collect::<Vec<_>>()
            .join(" | "),
        RegistryDecodedValue::Dword { value } => value.to_string(),
        RegistryDecodedValue::Qword { value } => value.to_string(),
    }
}

fn technical_decoded(value: &RegistryDecodedValue) -> String {
    match value {
        RegistryDecodedValue::String { value } => format!("string: {}", technical_literal(value)),
        RegistryDecodedValue::ExpandString { value } => {
            format!("expand_string (unexpanded): {}", technical_literal(value))
        }
        RegistryDecodedValue::MultiString { values } => format!(
            "multi_string ({} elements): {}",
            values.len(),
            values
                .iter()
                .enumerate()
                .map(|(index, value)| format!("[{index}]={}", technical_literal(value)))
                .collect::<Vec<_>>()
                .join("; ")
        ),
        RegistryDecodedValue::Dword { value } => format!("dword: {value}"),
        RegistryDecodedValue::Qword { value } => format!("qword: {value}"),
    }
}

fn technical_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len().saturating_add(2));
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() || is_unsafe_format_character(character) => {
                let _ = write!(escaped, "\\u{{{:x}}}", u32::from(character));
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

fn terminal_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() || is_unsafe_format_character(character) => {
                let _ = write!(escaped, "\\u{{{:x}}}", u32::from(character));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn is_unsafe_format_character(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{2028}'..='\u{2029}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}

fn collector_version(snapshot: &Snapshot, collector_id: &str) -> Option<u32> {
    snapshot
        .collectors
        .iter()
        .find(|run| run.id == collector_id)
        .map(|run| run.version)
}

fn optional_version(version: Option<u32>) -> String {
    version
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_owned())
}

fn human_change_label(change: &ChangeKind) -> &'static str {
    match change {
        ChangeKind::Added { .. } => "Added",
        ChangeKind::Removed { .. } => "Removed",
        ChangeKind::Modified { .. } => "Modified",
        ChangeKind::Unchanged { .. } => "Unchanged",
        ChangeKind::Inconclusive { .. } => "Inconclusive because coverage was incomplete",
    }
}

fn human_hive(hive: RegistryHive) -> &'static str {
    match hive {
        RegistryHive::CurrentUser => "current-user",
        RegistryHive::LocalMachine => "machine-wide",
    }
}

fn hive_abbreviation(hive: RegistryHive) -> &'static str {
    match hive {
        RegistryHive::CurrentUser => "HKCU",
        RegistryHive::LocalMachine => "HKLM",
    }
}

fn technical_hive(hive: RegistryHive) -> &'static str {
    match hive {
        RegistryHive::CurrentUser => "current_user",
        RegistryHive::LocalMachine => "local_machine",
    }
}

fn startup_kind(kind: RegistryStartupKind) -> &'static str {
    match kind {
        RegistryStartupKind::Run => "Run",
        RegistryStartupKind::RunOnce => "RunOnce",
    }
}

fn technical_startup_kind(kind: RegistryStartupKind) -> &'static str {
    match kind {
        RegistryStartupKind::Run => "run",
        RegistryStartupKind::RunOnce => "run_once",
    }
}

fn human_startup_location(kind: RegistryStartupKind) -> &'static str {
    match kind {
        RegistryStartupKind::Run => "startup",
        RegistryStartupKind::RunOnce => "one-time startup (RunOnce)",
    }
}

fn human_view_suffix(view: RegistryView) -> &'static str {
    match view {
        RegistryView::Shared | RegistryView::Native => "",
        RegistryView::Registry32 => " (32-bit Registry view)",
        RegistryView::Registry64 => " (64-bit Registry view)",
    }
}

fn human_scope_label(collector_id: &str, scope_id: &str) -> String {
    if collector_id == "windows.services" && scope_id == "current_token.win32" {
        return "Windows services visible to the current token".to_owned();
    }
    if collector_id != "windows.registry.startup" {
        return format!(
            "{}/{}",
            terminal_text(collector_id),
            terminal_text(scope_id)
        );
    }

    match scope_id {
        "current_user.shared.run" => "Current-user Run startup".to_owned(),
        "current_user.shared.run_once" => "Current-user RunOnce startup".to_owned(),
        "local_machine.native.run" => "Machine-wide Run startup".to_owned(),
        "local_machine.native.run_once" => "Machine-wide RunOnce startup".to_owned(),
        "local_machine.registry32.run" => {
            "Machine-wide Run startup (32-bit Registry view)".to_owned()
        }
        "local_machine.registry32.run_once" => {
            "Machine-wide RunOnce startup (32-bit Registry view)".to_owned()
        }
        "local_machine.registry64.run" => {
            "Machine-wide Run startup (64-bit Registry view)".to_owned()
        }
        "local_machine.registry64.run_once" => {
            "Machine-wide RunOnce startup (64-bit Registry view)".to_owned()
        }
        _ => format!(
            "{}/{}",
            terminal_text(collector_id),
            terminal_text(scope_id)
        ),
    }
}

fn technical_view(view: RegistryView) -> &'static str {
    match view {
        RegistryView::Shared => "shared",
        RegistryView::Native => "native",
        RegistryView::Registry32 => "registry32",
        RegistryView::Registry64 => "registry64",
    }
}

fn technical_prefix(prefix: RunOncePrefixSemantics) -> &'static str {
    match prefix {
        RunOncePrefixSemantics::NoDocumentedPrefix => "no_documented_prefix",
        RunOncePrefixSemantics::DeferDeletionUntilAfterRun => "defer_deletion_until_after_run",
        RunOncePrefixSemantics::RunInSafeMode => "run_in_safe_mode",
        RunOncePrefixSemantics::Undocumented => "undocumented",
    }
}

fn decode_status(decoding: &RegistryValueDecoding) -> &'static str {
    match decoding {
        RegistryValueDecoding::Decoded { .. } => "decoded",
        RegistryValueDecoding::NotApplicable => "not_applicable",
        RegistryValueDecoding::InvalidData => "invalid_data",
        RegistryValueDecoding::UnsupportedType => "unsupported_type",
    }
}

fn registry_type_name(value_type: u32) -> &'static str {
    match value_type {
        0 => "REG_NONE",
        1 => "REG_SZ",
        2 => "REG_EXPAND_SZ",
        3 => "REG_BINARY",
        4 => "REG_DWORD",
        5 => "REG_DWORD_BIG_ENDIAN",
        6 => "REG_LINK",
        7 => "REG_MULTI_SZ",
        8 => "REG_RESOURCE_LIST",
        9 => "REG_FULL_RESOURCE_DESCRIPTOR",
        10 => "REG_RESOURCE_REQUIREMENTS_LIST",
        11 => "REG_QWORD",
        _ => "unknown native type",
    }
}

fn status(status: CollectorStatus) -> &'static str {
    match status {
        CollectorStatus::Complete => "complete",
        CollectorStatus::Partial => "partial",
        CollectorStatus::PermissionDenied => "permission denied",
        CollectorStatus::Unavailable => "unavailable",
        CollectorStatus::Unsupported => "unsupported",
        CollectorStatus::Failed => "failed",
    }
}

fn optional_status(value: Option<CollectorStatus>) -> &'static str {
    value.map(status).unwrap_or("not present")
}

fn inconclusive_reason(reason: systemdiff_diff::InconclusiveReason) -> &'static str {
    match reason {
        systemdiff_diff::InconclusiveReason::CoverageIncomplete => "coverage_incomplete",
    }
}

fn change_index(change: &ChangeKind) -> usize {
    match change {
        ChangeKind::Added { .. } => 0,
        ChangeKind::Modified { .. } => 1,
        ChangeKind::Removed { .. } => 2,
        ChangeKind::Inconclusive { .. } => 3,
        ChangeKind::Unchanged { .. } => 4,
    }
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
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
    use systemdiff_diff::DiffDocument;

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
                "No changes found\n",
                "Compared 2026-08-11T00:00:00Z -> 2026-08-11T00:05:00Z\n"
            )
        );
        assert!(!output.contains('\u{1b}'));
    }
}
