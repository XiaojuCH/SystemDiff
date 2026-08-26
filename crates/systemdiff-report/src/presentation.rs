//! Locale-neutral presentation data for the desktop IPC boundary.
//!
//! Rust owns change classification and evidence-to-field mapping. The desktop
//! frontend only translates stable semantic identifiers and renders values.

use serde::Serialize;
use systemdiff_core::{
    Artifact, CollectorStatus, RegistryDecodedValue, RegistryHive, RegistryStartupEntry,
    RegistryStartupKind, RegistryValueDecoding, RegistryValueName, RegistryView, WindowsService,
};
use systemdiff_diff::{ArtifactChange, ChangeKind, DiffDocument, DiffWarning};

use super::terminal_text;

pub const DESKTOP_PRESENTATION_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DesktopPresentation {
    pub contract_version: u32,
    pub started_at_utc: String,
    pub finished_at_utc: String,
    pub summary: DesktopSummary,
    pub groups: Vec<DesktopGroup>,
    pub coverage_notices: Vec<DesktopCoverageNotice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DesktopSummary {
    pub confirmed_change_count: u64,
    pub inconclusive_change_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DesktopGroup {
    pub group_id: DesktopGroupId,
    pub heading_message_id: DesktopMessageId,
    pub empty_message_id: DesktopMessageId,
    pub items: Vec<DesktopChangeItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopGroupId {
    Startup,
    WindowsServices,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DesktopChangeItem {
    pub change: DesktopChangeKind,
    pub message_id: DesktopMessageId,
    pub headline: DesktopValue,
    pub fields: Vec<DesktopField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopChangeKind {
    Added,
    Removed,
    Modified,
    Unchanged,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DesktopField {
    pub field_id: DesktopFieldId,
    #[serde(flatten)]
    pub content: DesktopFieldContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum DesktopFieldContent {
    Current {
        value: DesktopValue,
    },
    Changed {
        before: DesktopValue,
        after: DesktopValue,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DesktopValue {
    Evidence { value: String },
    Message { message_id: DesktopMessageId },
    Number { value: u64 },
    Boolean { value: bool },
    EvidenceList { values: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DesktopFieldId {
    #[serde(rename = "registry.location")]
    RegistryLocation,
    #[serde(rename = "registry.command")]
    RegistryCommand,
    #[serde(rename = "service.name")]
    ServiceName,
    #[serde(rename = "service.display_name")]
    ServiceDisplayName,
    #[serde(rename = "service.type")]
    ServiceType,
    #[serde(rename = "service.start")]
    ServiceStart,
    #[serde(rename = "service.delayed_auto_start")]
    ServiceDelayedAutoStart,
    #[serde(rename = "service.error_control")]
    ServiceErrorControl,
    #[serde(rename = "service.binary_path")]
    ServiceBinaryPath,
    #[serde(rename = "service.account")]
    ServiceAccount,
    #[serde(rename = "service.dependencies")]
    ServiceDependencies,
    #[serde(rename = "service.load_order_group")]
    ServiceLoadOrderGroup,
    #[serde(rename = "service.tag_id")]
    ServiceTagId,
    #[serde(rename = "service.description")]
    ServiceDescription,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DesktopCoverageNotice {
    pub group_id: Option<DesktopGroupId>,
    pub message_id: DesktopMessageId,
    pub scope_message_id: DesktopMessageId,
    pub before_status: DesktopCoverageStatus,
    pub after_status: DesktopCoverageStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopCoverageStatus {
    Complete,
    Partial,
    PermissionDenied,
    Unavailable,
    Unsupported,
    Failed,
    NotPresent,
}

/// Stable localization keys exposed at the desktop IPC boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DesktopMessageId {
    #[serde(rename = "group.startup")]
    GroupStartup,
    #[serde(rename = "group.startup.empty")]
    GroupStartupEmpty,
    #[serde(rename = "group.windows_services")]
    GroupWindowsServices,
    #[serde(rename = "group.windows_services.empty")]
    GroupWindowsServicesEmpty,
    #[serde(rename = "change.registry_startup.added")]
    RegistryStartupAdded,
    #[serde(rename = "change.registry_startup.removed")]
    RegistryStartupRemoved,
    #[serde(rename = "change.registry_startup.modified")]
    RegistryStartupModified,
    #[serde(rename = "change.registry_startup.unchanged")]
    RegistryStartupUnchanged,
    #[serde(rename = "change.registry_startup.inconclusive")]
    RegistryStartupInconclusive,
    #[serde(rename = "change.windows_service.added")]
    WindowsServiceAdded,
    #[serde(rename = "change.windows_service.removed")]
    WindowsServiceRemoved,
    #[serde(rename = "change.windows_service.modified")]
    WindowsServiceModified,
    #[serde(rename = "change.windows_service.unchanged")]
    WindowsServiceUnchanged,
    #[serde(rename = "change.windows_service.inconclusive")]
    WindowsServiceInconclusive,
    #[serde(rename = "value.registry.default_name")]
    RegistryDefaultValueName,
    #[serde(rename = "value.registry.undecodable_name")]
    RegistryUndecodableValueName,
    #[serde(rename = "value.registry.unavailable")]
    RegistryValueUnavailable,
    #[serde(rename = "value.not_set")]
    ValueNotSet,
    #[serde(rename = "value.none")]
    ValueNone,
    #[serde(rename = "value.service.start.boot")]
    ServiceStartBoot,
    #[serde(rename = "value.service.start.system")]
    ServiceStartSystem,
    #[serde(rename = "value.service.start.automatic")]
    ServiceStartAutomatic,
    #[serde(rename = "value.service.start.automatic_delayed")]
    ServiceStartAutomaticDelayed,
    #[serde(rename = "value.service.start.manual")]
    ServiceStartManual,
    #[serde(rename = "value.service.start.disabled")]
    ServiceStartDisabled,
    #[serde(rename = "value.service.start.unknown")]
    ServiceStartUnknown,
    #[serde(rename = "value.service.error_control.ignore")]
    ServiceErrorControlIgnore,
    #[serde(rename = "value.service.error_control.normal")]
    ServiceErrorControlNormal,
    #[serde(rename = "value.service.error_control.severe")]
    ServiceErrorControlSevere,
    #[serde(rename = "value.service.error_control.critical")]
    ServiceErrorControlCritical,
    #[serde(rename = "value.service.error_control.unknown")]
    ServiceErrorControlUnknown,
    #[serde(rename = "coverage.incomplete.before")]
    CoverageIncompleteBefore,
    #[serde(rename = "coverage.incomplete.after")]
    CoverageIncompleteAfter,
    #[serde(rename = "coverage.incomplete.both")]
    CoverageIncompleteBoth,
    #[serde(rename = "coverage.scope.current_user_run")]
    CoverageScopeCurrentUserRun,
    #[serde(rename = "coverage.scope.current_user_run_once")]
    CoverageScopeCurrentUserRunOnce,
    #[serde(rename = "coverage.scope.local_machine_native_run")]
    CoverageScopeLocalMachineNativeRun,
    #[serde(rename = "coverage.scope.local_machine_native_run_once")]
    CoverageScopeLocalMachineNativeRunOnce,
    #[serde(rename = "coverage.scope.local_machine_registry32_run")]
    CoverageScopeLocalMachineRegistry32Run,
    #[serde(rename = "coverage.scope.local_machine_registry32_run_once")]
    CoverageScopeLocalMachineRegistry32RunOnce,
    #[serde(rename = "coverage.scope.local_machine_registry64_run")]
    CoverageScopeLocalMachineRegistry64Run,
    #[serde(rename = "coverage.scope.local_machine_registry64_run_once")]
    CoverageScopeLocalMachineRegistry64RunOnce,
    #[serde(rename = "coverage.scope.windows_services_current_token")]
    CoverageScopeWindowsServicesCurrentToken,
    #[serde(rename = "coverage.scope.unknown")]
    CoverageScopeUnknown,
}

pub fn build_desktop_presentation(diff: &DiffDocument) -> DesktopPresentation {
    let mut startup_items = Vec::new();
    let mut service_items = Vec::new();

    for change in &diff.changes {
        match change_artifact(change) {
            Some(Artifact::RegistryStartup(_)) => {
                if let Some(item) = registry_item(change) {
                    startup_items.push(item);
                }
            }
            Some(Artifact::WindowsService(_)) => {
                if let Some(item) = service_item(change) {
                    service_items.push(item);
                }
            }
            Some(Artifact::ScheduledTask(_)) | None => {}
        }
    }

    let confirmed_change_count = u64::try_from(
        startup_items
            .iter()
            .chain(&service_items)
            .filter(|item| {
                matches!(
                    item.change,
                    DesktopChangeKind::Added
                        | DesktopChangeKind::Removed
                        | DesktopChangeKind::Modified
                )
            })
            .count(),
    )
    .unwrap_or(u64::MAX);
    let inconclusive_change_count = u64::try_from(
        startup_items
            .iter()
            .chain(&service_items)
            .filter(|item| item.change == DesktopChangeKind::Inconclusive)
            .count(),
    )
    .unwrap_or(u64::MAX);

    DesktopPresentation {
        contract_version: DESKTOP_PRESENTATION_CONTRACT_VERSION,
        started_at_utc: diff.before_captured_at.clone(),
        finished_at_utc: diff.after_captured_at.clone(),
        summary: DesktopSummary {
            confirmed_change_count,
            inconclusive_change_count,
        },
        groups: vec![
            DesktopGroup {
                group_id: DesktopGroupId::Startup,
                heading_message_id: DesktopMessageId::GroupStartup,
                empty_message_id: DesktopMessageId::GroupStartupEmpty,
                items: startup_items,
            },
            DesktopGroup {
                group_id: DesktopGroupId::WindowsServices,
                heading_message_id: DesktopMessageId::GroupWindowsServices,
                empty_message_id: DesktopMessageId::GroupWindowsServicesEmpty,
                items: service_items,
            },
        ],
        coverage_notices: diff.warnings.iter().map(coverage_notice).collect(),
    }
}

fn registry_item(change: &ArtifactChange) -> Option<DesktopChangeItem> {
    let entry = registry_change_entry(change)?;
    let (kind, message_id) = change_semantics(
        &change.change,
        DesktopMessageId::RegistryStartupAdded,
        DesktopMessageId::RegistryStartupRemoved,
        DesktopMessageId::RegistryStartupModified,
        DesktopMessageId::RegistryStartupUnchanged,
        DesktopMessageId::RegistryStartupInconclusive,
    );
    let mut fields = vec![current_field(
        DesktopFieldId::RegistryLocation,
        DesktopValue::Evidence {
            value: registry_location(entry),
        },
    )];

    match &change.change {
        ChangeKind::Modified {
            before: Artifact::RegistryStartup(before),
            after: Artifact::RegistryStartup(after),
        } => {
            let before = registry_value(before);
            let after = registry_value(after);
            if before != after {
                fields.push(changed_field(
                    DesktopFieldId::RegistryCommand,
                    before,
                    after,
                ));
            } else {
                fields.push(current_field(DesktopFieldId::RegistryCommand, after));
            }
        }
        _ => fields.push(current_field(
            DesktopFieldId::RegistryCommand,
            registry_value(entry),
        )),
    }

    Some(DesktopChangeItem {
        change: kind,
        message_id,
        headline: registry_name(&entry.value_name),
        fields,
    })
}

fn service_item(change: &ArtifactChange) -> Option<DesktopChangeItem> {
    let service = service_change_entry(change)?;
    let (kind, message_id) = change_semantics(
        &change.change,
        DesktopMessageId::WindowsServiceAdded,
        DesktopMessageId::WindowsServiceRemoved,
        DesktopMessageId::WindowsServiceModified,
        DesktopMessageId::WindowsServiceUnchanged,
        DesktopMessageId::WindowsServiceInconclusive,
    );
    let fields = match &change.change {
        ChangeKind::Modified {
            before: Artifact::WindowsService(before),
            after: Artifact::WindowsService(after),
        } => changed_service_fields(before, after),
        ChangeKind::Unchanged { .. } => Vec::new(),
        _ => service_summary_fields(service),
    };

    Some(DesktopChangeItem {
        change: kind,
        message_id,
        headline: service
            .display_name
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(evidence)
            .unwrap_or_else(|| evidence(&service.service_name)),
        fields,
    })
}

fn service_summary_fields(service: &WindowsService) -> Vec<DesktopField> {
    vec![
        current_field(DesktopFieldId::ServiceName, evidence(&service.service_name)),
        current_field(
            DesktopFieldId::ServiceStart,
            service_start(service.start_type, service.delayed_auto_start),
        ),
        current_field(
            DesktopFieldId::ServiceBinaryPath,
            evidence(&service.binary_path),
        ),
        current_field(
            DesktopFieldId::ServiceAccount,
            optional_evidence(service.account.as_deref()),
        ),
    ]
}

fn changed_service_fields(before: &WindowsService, after: &WindowsService) -> Vec<DesktopField> {
    let mut fields = Vec::new();
    push_changed(
        &mut fields,
        DesktopFieldId::ServiceName,
        &before.service_name,
        &after.service_name,
        evidence(&before.service_name),
        evidence(&after.service_name),
    );
    push_changed(
        &mut fields,
        DesktopFieldId::ServiceDisplayName,
        &before.display_name,
        &after.display_name,
        optional_evidence(before.display_name.as_deref()),
        optional_evidence(after.display_name.as_deref()),
    );
    push_changed(
        &mut fields,
        DesktopFieldId::ServiceType,
        &before.service_type,
        &after.service_type,
        number(before.service_type),
        number(after.service_type),
    );
    push_changed(
        &mut fields,
        DesktopFieldId::ServiceStart,
        &(before.start_type, before.delayed_auto_start),
        &(after.start_type, after.delayed_auto_start),
        service_start(before.start_type, before.delayed_auto_start),
        service_start(after.start_type, after.delayed_auto_start),
    );
    push_changed(
        &mut fields,
        DesktopFieldId::ServiceDelayedAutoStart,
        &before.delayed_auto_start,
        &after.delayed_auto_start,
        DesktopValue::Boolean {
            value: before.delayed_auto_start,
        },
        DesktopValue::Boolean {
            value: after.delayed_auto_start,
        },
    );
    push_changed(
        &mut fields,
        DesktopFieldId::ServiceErrorControl,
        &before.error_control,
        &after.error_control,
        service_error_control(before.error_control),
        service_error_control(after.error_control),
    );
    push_changed(
        &mut fields,
        DesktopFieldId::ServiceBinaryPath,
        &before.binary_path,
        &after.binary_path,
        evidence(&before.binary_path),
        evidence(&after.binary_path),
    );
    push_changed(
        &mut fields,
        DesktopFieldId::ServiceAccount,
        &before.account,
        &after.account,
        optional_evidence(before.account.as_deref()),
        optional_evidence(after.account.as_deref()),
    );
    push_changed(
        &mut fields,
        DesktopFieldId::ServiceDependencies,
        &before.dependencies,
        &after.dependencies,
        evidence_list(&before.dependencies),
        evidence_list(&after.dependencies),
    );
    push_changed(
        &mut fields,
        DesktopFieldId::ServiceLoadOrderGroup,
        &before.load_order_group,
        &after.load_order_group,
        optional_evidence(before.load_order_group.as_deref()),
        optional_evidence(after.load_order_group.as_deref()),
    );
    push_changed(
        &mut fields,
        DesktopFieldId::ServiceTagId,
        &before.tag_id,
        &after.tag_id,
        optional_number(before.tag_id),
        optional_number(after.tag_id),
    );
    push_changed(
        &mut fields,
        DesktopFieldId::ServiceDescription,
        &before.description,
        &after.description,
        optional_evidence(before.description.as_deref()),
        optional_evidence(after.description.as_deref()),
    );
    fields
}

fn push_changed<T: PartialEq>(
    fields: &mut Vec<DesktopField>,
    field_id: DesktopFieldId,
    before_source: &T,
    after_source: &T,
    before: DesktopValue,
    after: DesktopValue,
) {
    if before_source != after_source {
        fields.push(changed_field(field_id, before, after));
    }
}

fn coverage_notice(warning: &DiffWarning) -> DesktopCoverageNotice {
    let before_status = coverage_status(warning.before_status);
    let after_status = coverage_status(warning.after_status);
    let message_id = match (
        warning.before_status == Some(CollectorStatus::Complete),
        warning.after_status == Some(CollectorStatus::Complete),
    ) {
        (false, true) => DesktopMessageId::CoverageIncompleteBefore,
        (true, false) => DesktopMessageId::CoverageIncompleteAfter,
        _ => DesktopMessageId::CoverageIncompleteBoth,
    };

    DesktopCoverageNotice {
        group_id: match warning.collector_id.as_str() {
            "windows.registry.startup" => Some(DesktopGroupId::Startup),
            "windows.services" => Some(DesktopGroupId::WindowsServices),
            _ => None,
        },
        message_id,
        scope_message_id: coverage_scope(&warning.collector_id, &warning.scope_id),
        before_status,
        after_status,
    }
}

fn coverage_scope(collector_id: &str, scope_id: &str) -> DesktopMessageId {
    match (collector_id, scope_id) {
        ("windows.registry.startup", "current_user.shared.run") => {
            DesktopMessageId::CoverageScopeCurrentUserRun
        }
        ("windows.registry.startup", "current_user.shared.run_once") => {
            DesktopMessageId::CoverageScopeCurrentUserRunOnce
        }
        ("windows.registry.startup", "local_machine.native.run") => {
            DesktopMessageId::CoverageScopeLocalMachineNativeRun
        }
        ("windows.registry.startup", "local_machine.native.run_once") => {
            DesktopMessageId::CoverageScopeLocalMachineNativeRunOnce
        }
        ("windows.registry.startup", "local_machine.registry32.run") => {
            DesktopMessageId::CoverageScopeLocalMachineRegistry32Run
        }
        ("windows.registry.startup", "local_machine.registry32.run_once") => {
            DesktopMessageId::CoverageScopeLocalMachineRegistry32RunOnce
        }
        ("windows.registry.startup", "local_machine.registry64.run") => {
            DesktopMessageId::CoverageScopeLocalMachineRegistry64Run
        }
        ("windows.registry.startup", "local_machine.registry64.run_once") => {
            DesktopMessageId::CoverageScopeLocalMachineRegistry64RunOnce
        }
        ("windows.services", "current_token.win32") => {
            DesktopMessageId::CoverageScopeWindowsServicesCurrentToken
        }
        _ => DesktopMessageId::CoverageScopeUnknown,
    }
}

fn coverage_status(status: Option<CollectorStatus>) -> DesktopCoverageStatus {
    match status {
        Some(CollectorStatus::Complete) => DesktopCoverageStatus::Complete,
        Some(CollectorStatus::Partial) => DesktopCoverageStatus::Partial,
        Some(CollectorStatus::PermissionDenied) => DesktopCoverageStatus::PermissionDenied,
        Some(CollectorStatus::Unavailable) => DesktopCoverageStatus::Unavailable,
        Some(CollectorStatus::Unsupported) => DesktopCoverageStatus::Unsupported,
        Some(CollectorStatus::Failed) => DesktopCoverageStatus::Failed,
        None => DesktopCoverageStatus::NotPresent,
    }
}

fn change_artifact(change: &ArtifactChange) -> Option<&Artifact> {
    match &change.change {
        ChangeKind::Added { after } | ChangeKind::Modified { after, .. } => Some(after),
        ChangeKind::Removed { before } => Some(before),
        ChangeKind::Unchanged { artifact } => Some(artifact),
        ChangeKind::Inconclusive { before, after, .. } => before.as_ref().or(after.as_ref()),
    }
}

fn registry_change_entry(change: &ArtifactChange) -> Option<&RegistryStartupEntry> {
    match change_artifact(change) {
        Some(Artifact::RegistryStartup(entry)) => Some(entry),
        _ => None,
    }
}

fn service_change_entry(change: &ArtifactChange) -> Option<&WindowsService> {
    match change_artifact(change) {
        Some(Artifact::WindowsService(service)) => Some(service),
        _ => None,
    }
}

fn change_semantics(
    change: &ChangeKind,
    added: DesktopMessageId,
    removed: DesktopMessageId,
    modified: DesktopMessageId,
    unchanged: DesktopMessageId,
    inconclusive: DesktopMessageId,
) -> (DesktopChangeKind, DesktopMessageId) {
    match change {
        ChangeKind::Added { .. } => (DesktopChangeKind::Added, added),
        ChangeKind::Removed { .. } => (DesktopChangeKind::Removed, removed),
        ChangeKind::Modified { .. } => (DesktopChangeKind::Modified, modified),
        ChangeKind::Unchanged { .. } => (DesktopChangeKind::Unchanged, unchanged),
        ChangeKind::Inconclusive { .. } => (DesktopChangeKind::Inconclusive, inconclusive),
    }
}

fn current_field(field_id: DesktopFieldId, value: DesktopValue) -> DesktopField {
    DesktopField {
        field_id,
        content: DesktopFieldContent::Current { value },
    }
}

fn changed_field(
    field_id: DesktopFieldId,
    before: DesktopValue,
    after: DesktopValue,
) -> DesktopField {
    DesktopField {
        field_id,
        content: DesktopFieldContent::Changed { before, after },
    }
}

fn registry_name(name: &RegistryValueName) -> DesktopValue {
    match name {
        RegistryValueName::Decoded { value } if value.is_empty() => DesktopValue::Message {
            message_id: DesktopMessageId::RegistryDefaultValueName,
        },
        RegistryValueName::Decoded { value } => evidence(value),
        RegistryValueName::InvalidUtf16 { .. } => DesktopValue::Message {
            message_id: DesktopMessageId::RegistryUndecodableValueName,
        },
    }
}

fn registry_value(entry: &RegistryStartupEntry) -> DesktopValue {
    match &entry.decoding {
        RegistryValueDecoding::Decoded { value } => match value {
            RegistryDecodedValue::String { value }
            | RegistryDecodedValue::ExpandString { value } => evidence(value),
            RegistryDecodedValue::MultiString { values } => evidence_list(values),
            RegistryDecodedValue::Dword { value } => number(*value),
            RegistryDecodedValue::Qword { value } => DesktopValue::Number { value: *value },
        },
        RegistryValueDecoding::NotApplicable
        | RegistryValueDecoding::InvalidData
        | RegistryValueDecoding::UnsupportedType => DesktopValue::Message {
            message_id: DesktopMessageId::RegistryValueUnavailable,
        },
    }
}

fn registry_location(entry: &RegistryStartupEntry) -> String {
    let hive = match entry.hive {
        RegistryHive::CurrentUser => "HKCU",
        RegistryHive::LocalMachine => "HKLM",
    };
    let view = match entry.registry_view {
        RegistryView::Shared => "shared",
        RegistryView::Native => "native",
        RegistryView::Registry32 => "registry32",
        RegistryView::Registry64 => "registry64",
    };
    let startup_kind = match entry.startup_kind {
        RegistryStartupKind::Run => "Run",
        RegistryStartupKind::RunOnce => "RunOnce",
    };
    format!(
        "{hive}\\{} ({startup_kind}, {view})",
        terminal_text(&entry.key_path)
    )
}

fn service_start(start_type: u32, delayed_auto_start: bool) -> DesktopValue {
    let message_id = match start_type {
        0 => DesktopMessageId::ServiceStartBoot,
        1 => DesktopMessageId::ServiceStartSystem,
        2 if delayed_auto_start => DesktopMessageId::ServiceStartAutomaticDelayed,
        2 => DesktopMessageId::ServiceStartAutomatic,
        3 => DesktopMessageId::ServiceStartManual,
        4 => DesktopMessageId::ServiceStartDisabled,
        _ => DesktopMessageId::ServiceStartUnknown,
    };
    DesktopValue::Message { message_id }
}

fn service_error_control(error_control: u32) -> DesktopValue {
    let message_id = match error_control {
        0 => DesktopMessageId::ServiceErrorControlIgnore,
        1 => DesktopMessageId::ServiceErrorControlNormal,
        2 => DesktopMessageId::ServiceErrorControlSevere,
        3 => DesktopMessageId::ServiceErrorControlCritical,
        _ => DesktopMessageId::ServiceErrorControlUnknown,
    };
    DesktopValue::Message { message_id }
}

fn evidence(value: &str) -> DesktopValue {
    DesktopValue::Evidence {
        value: terminal_text(value),
    }
}

fn optional_evidence(value: Option<&str>) -> DesktopValue {
    value.map(evidence).unwrap_or(DesktopValue::Message {
        message_id: DesktopMessageId::ValueNotSet,
    })
}

fn evidence_list(values: &[String]) -> DesktopValue {
    if values.is_empty() {
        DesktopValue::Message {
            message_id: DesktopMessageId::ValueNone,
        }
    } else {
        DesktopValue::EvidenceList {
            values: values.iter().map(|value| terminal_text(value)).collect(),
        }
    }
}

fn number(value: u32) -> DesktopValue {
    DesktopValue::Number {
        value: u64::from(value),
    }
}

fn optional_number(value: Option<u32>) -> DesktopValue {
    value.map(number).unwrap_or(DesktopValue::Message {
        message_id: DesktopMessageId::ValueNotSet,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use systemdiff_core::{ArtifactKey, RegistryRawEvidence, RunOncePrefixSemantics};
    use systemdiff_diff::{DiffWarningCode, InconclusiveReason};

    const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn empty_diff() -> DiffDocument {
        DiffDocument {
            document_type: "systemdiff.diff".to_owned(),
            schema_version: 1,
            before_captured_at: "2026-08-20T00:00:00Z".to_owned(),
            after_captured_at: "2026-08-20T00:05:00Z".to_owned(),
            changes: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn key(collector_id: &str, scope_id: &str, artifact_kind: &str) -> ArtifactKey {
        ArtifactKey {
            collector_id: collector_id.to_owned(),
            scope_id: scope_id.to_owned(),
            artifact_kind: artifact_kind.to_owned(),
            canonical_id: "must-not-leak".to_owned(),
        }
    }

    fn registry(name: RegistryValueName, command: &str) -> RegistryStartupEntry {
        RegistryStartupEntry {
            hive: RegistryHive::CurrentUser,
            registry_view: RegistryView::Shared,
            key_path: r"Software\Microsoft\Windows\CurrentVersion\Run".to_owned(),
            value_name: name,
            startup_kind: RegistryStartupKind::Run,
            run_once_prefix: None,
            value_type: 1,
            content_sha256: HASH.to_owned(),
            decoding: RegistryValueDecoding::Decoded {
                value: RegistryDecodedValue::String {
                    value: command.to_owned(),
                },
            },
            raw_evidence: Some(RegistryRawEvidence {
                content_hex: "00".to_owned(),
                captured_byte_count: 1,
                original_byte_count: 1,
                truncated: false,
            }),
        }
    }

    fn service(binary_path: &str, description: Option<&str>) -> WindowsService {
        WindowsService {
            service_name: "SystemDiffTest".to_owned(),
            display_name: Some("SystemDiff Test Service".to_owned()),
            service_type: 0x10,
            start_type: 3,
            error_control: 1,
            binary_path: binary_path.to_owned(),
            account: Some("LocalSystem".to_owned()),
            dependencies: Vec::new(),
            load_order_group: None,
            tag_id: None,
            delayed_auto_start: false,
            description: description.map(str::to_owned),
        }
    }

    #[test]
    fn registry_added_is_classified_and_raw_evidence_is_not_exposed() {
        let mut diff = empty_diff();
        diff.changes.push(ArtifactChange {
            change_id: "change:v1:00000000".to_owned(),
            key: key(
                "windows.registry.startup",
                "current_user.shared.run",
                "registry_startup",
            ),
            change: ChangeKind::Added {
                after: Artifact::RegistryStartup(registry(
                    RegistryValueName::decoded("SystemDiffDogfood"),
                    r"C:\Program Files\SystemDiff\dogfood.exe",
                )),
            },
        });

        let presentation = build_desktop_presentation(&diff);

        assert_eq!(presentation.contract_version, 1);
        assert_eq!(presentation.summary.confirmed_change_count, 1);
        assert_eq!(presentation.summary.inconclusive_change_count, 0);
        assert_eq!(presentation.groups.len(), 2);
        assert_eq!(
            presentation.groups[0].items[0].change,
            DesktopChangeKind::Added
        );
        assert_eq!(
            presentation.groups[0].items[0].message_id,
            DesktopMessageId::RegistryStartupAdded
        );
        let json = serde_json::to_string(&presentation).expect("presentation must serialize");
        assert!(json.contains("SystemDiffDogfood"));
        assert!(!json.contains(HASH));
        assert!(!json.contains("must-not-leak"));
        assert!(!json.contains("content_hex"));
    }

    #[test]
    fn service_modified_contains_only_changed_fields() {
        let mut diff = empty_diff();
        diff.changes.push(ArtifactChange {
            change_id: "change:v1:00000000".to_owned(),
            key: key("windows.services", "current_token.win32", "windows_service"),
            change: ChangeKind::Modified {
                before: Artifact::WindowsService(service("old.exe", Some("before"))),
                after: Artifact::WindowsService(service("new.exe", Some("after"))),
            },
        });

        let presentation = build_desktop_presentation(&diff);
        let item = &presentation.groups[1].items[0];
        let field_ids: Vec<_> = item.fields.iter().map(|field| field.field_id).collect();

        assert_eq!(item.change, DesktopChangeKind::Modified);
        assert_eq!(
            field_ids,
            [
                DesktopFieldId::ServiceBinaryPath,
                DesktopFieldId::ServiceDescription
            ]
        );
        assert!(
            item.fields
                .iter()
                .all(|field| matches!(field.content, DesktopFieldContent::Changed { .. }))
        );
    }

    #[test]
    fn one_sided_partial_service_change_is_inconclusive_with_coverage_notice() {
        let mut diff = empty_diff();
        diff.changes.push(ArtifactChange {
            change_id: "change:v1:00000000".to_owned(),
            key: key("windows.services", "current_token.win32", "windows_service"),
            change: ChangeKind::Inconclusive {
                before: Some(Artifact::WindowsService(service("service.exe", None))),
                after: None,
                reason: InconclusiveReason::CoverageIncomplete,
            },
        });
        diff.warnings.push(DiffWarning {
            code: DiffWarningCode::CoverageIncomplete,
            collector_id: "windows.services".to_owned(),
            scope_id: "current_token.win32".to_owned(),
            before_status: Some(CollectorStatus::Complete),
            after_status: Some(CollectorStatus::Partial),
        });

        let presentation = build_desktop_presentation(&diff);

        assert_eq!(presentation.summary.confirmed_change_count, 0);
        assert_eq!(presentation.summary.inconclusive_change_count, 1);
        assert_eq!(
            presentation.groups[1].items[0].change,
            DesktopChangeKind::Inconclusive
        );
        assert_eq!(
            presentation.coverage_notices,
            [DesktopCoverageNotice {
                group_id: Some(DesktopGroupId::WindowsServices),
                message_id: DesktopMessageId::CoverageIncompleteAfter,
                scope_message_id: DesktopMessageId::CoverageScopeWindowsServicesCurrentToken,
                before_status: DesktopCoverageStatus::Complete,
                after_status: DesktopCoverageStatus::Partial,
            }]
        );
    }

    #[test]
    fn empty_diff_keeps_both_stable_groups_and_raw_utc_timestamps() {
        let presentation = build_desktop_presentation(&empty_diff());

        assert_eq!(presentation.started_at_utc, "2026-08-20T00:00:00Z");
        assert_eq!(presentation.finished_at_utc, "2026-08-20T00:05:00Z");
        assert_eq!(
            presentation
                .groups
                .iter()
                .map(|group| group.group_id)
                .collect::<Vec<_>>(),
            [DesktopGroupId::Startup, DesktopGroupId::WindowsServices]
        );
        assert!(
            presentation
                .groups
                .iter()
                .all(|group| group.items.is_empty())
        );
        assert_eq!(
            serde_json::to_value(&presentation).expect("presentation must serialize"),
            serde_json::json!({
                "contract_version": 1,
                "started_at_utc": "2026-08-20T00:00:00Z",
                "finished_at_utc": "2026-08-20T00:05:00Z",
                "summary": {
                    "confirmed_change_count": 0,
                    "inconclusive_change_count": 0
                },
                "groups": [
                    {
                        "group_id": "startup",
                        "heading_message_id": "group.startup",
                        "empty_message_id": "group.startup.empty",
                        "items": []
                    },
                    {
                        "group_id": "windows_services",
                        "heading_message_id": "group.windows_services",
                        "empty_message_id": "group.windows_services.empty",
                        "items": []
                    }
                ],
                "coverage_notices": []
            })
        );
    }

    #[test]
    fn hostile_or_undecodable_registry_evidence_uses_safe_fallbacks() {
        let mut diff = empty_diff();
        let mut entry = registry(
            RegistryValueName::InvalidUtf16 {
                utf16le_hex: "00d8".to_owned(),
            },
            "safe\u{202e}evil\nnext",
        );
        entry.startup_kind = RegistryStartupKind::RunOnce;
        entry.key_path = r"Software\Microsoft\Windows\CurrentVersion\RunOnce".to_owned();
        entry.run_once_prefix = Some(RunOncePrefixSemantics::NoDocumentedPrefix);
        diff.changes.push(ArtifactChange {
            change_id: "change:v1:00000000".to_owned(),
            key: key(
                "windows.registry.startup",
                "current_user.shared.run_once",
                "registry_startup",
            ),
            change: ChangeKind::Added {
                after: Artifact::RegistryStartup(entry),
            },
        });

        let presentation = build_desktop_presentation(&diff);
        let item = &presentation.groups[0].items[0];

        assert_eq!(
            item.headline,
            DesktopValue::Message {
                message_id: DesktopMessageId::RegistryUndecodableValueName
            }
        );
        assert!(matches!(
            &item.fields[1].content,
            DesktopFieldContent::Current {
                value: DesktopValue::Evidence { value }
            } if value == r"safe\u{202e}evil\nnext"
        ));
    }

    #[test]
    fn serialization_contract_uses_stable_semantic_and_field_ids() {
        let mut diff = empty_diff();
        diff.changes.push(ArtifactChange {
            change_id: "change:v1:00000000".to_owned(),
            key: key("windows.services", "current_token.win32", "windows_service"),
            change: ChangeKind::Added {
                after: Artifact::WindowsService(service("service.exe", None)),
            },
        });

        let json = serde_json::to_value(build_desktop_presentation(&diff))
            .expect("presentation must serialize");

        assert_eq!(json["contract_version"], 1);
        assert_eq!(json["groups"][0]["group_id"], "startup");
        assert_eq!(json["groups"][1]["group_id"], "windows_services");
        assert_eq!(
            json["groups"][1]["items"][0]["message_id"],
            "change.windows_service.added"
        );
        assert_eq!(
            json["groups"][1]["items"][0]["fields"][0]["field_id"],
            "service.name"
        );
        assert_eq!(
            json["groups"][1]["items"][0]["fields"][0]["mode"],
            "current"
        );
    }
}
