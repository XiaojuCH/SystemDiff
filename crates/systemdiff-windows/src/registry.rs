use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::mem::size_of;
use systemdiff_core::{
    Artifact, CollectionContext, CollectionOutcome, Collector, CollectorDescriptor, CollectorRun,
    CollectorStatus, Diagnostic, Observation, PrivilegeRequirement, RegistryDecodedValue,
    RegistryHive, RegistryStartupEntry, RegistryStartupKind, RegistryValueDecoding,
    RegistryValueName, RegistryView, ScopeCoverage, classify_run_once_prefix_units,
};

pub const REGISTRY_STARTUP_COLLECTOR_ID: &str = "windows.registry.startup";
pub const REGISTRY_STARTUP_COLLECTOR_VERSION: u32 = 1;
pub const MAX_REGISTRY_VALUE_DATA_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_REGISTRY_COLLECTOR_EVIDENCE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_REGISTRY_VALUES_PER_SCOPE: u32 = 4_096;
pub const MAX_SCOPE_ATTEMPTS: usize = 3;

const RUN_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_ONCE_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce";
const IDENTITY_DOMAIN: &[u8] = b"systemdiff.registry-startup.identity.v1\0";

pub fn descriptor() -> CollectorDescriptor {
    CollectorDescriptor {
        id: REGISTRY_STARTUP_COLLECTOR_ID.to_owned(),
        version: REGISTRY_STARTUP_COLLECTOR_VERSION,
        description: "Documented Run and RunOnce registry startup locations.".to_owned(),
        privilege: PrivilegeRequirement::StandardUserPartial,
    }
}

pub struct RegistryStartupCollector;

impl Collector for RegistryStartupCollector {
    fn descriptor(&self) -> CollectorDescriptor {
        descriptor()
    }

    fn collect(&self, context: &CollectionContext) -> CollectionOutcome {
        let mut source = crate::win32::Win32RegistrySource::new();
        collect_with_source(&mut source, context)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) enum RegistryLayout {
    X64,
    X86,
    Arm64,
    Unknown(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegistryRoot {
    CurrentUser,
    LocalMachine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewSelector {
    Shared,
    Native,
    Registry32,
    Registry64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegistryTarget {
    pub scope_id: &'static str,
    pub root: RegistryRoot,
    pub view: RegistryView,
    pub selector: ViewSelector,
    pub key_path: &'static str,
    pub startup_kind: RegistryStartupKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeyMetadata {
    pub value_count: u32,
    pub last_write: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawRegistryValue {
    pub name_utf16: Vec<u16>,
    pub value_type: u32,
    pub data: Vec<u8>,
}

pub(crate) fn native_evidence_bytes(record: &RawRegistryValue) -> Option<usize> {
    record
        .name_utf16
        .len()
        .checked_mul(size_of::<u16>())?
        .checked_add(record.data.len())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeyReadAttempt {
    pub before: KeyMetadata,
    pub after: KeyMetadata,
    pub records: Vec<RawRegistryValue>,
    pub issues: Vec<SourceIssue>,
    pub stable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub(crate) enum ReadKeyResult {
    Missing,
    Present(KeyReadAttempt),
    Failed(SourceFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceFailureKind {
    AccessDenied,
    KeyDeleted,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceFailure {
    pub kind: SourceFailureKind,
    pub stage: &'static str,
    pub native_code: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceIssue {
    pub code: &'static str,
    pub message: &'static str,
    pub stage: &'static str,
    pub native_code: Option<i64>,
}

pub(crate) fn registry_resource_limit_issue() -> SourceIssue {
    SourceIssue {
        code: "registry_resource_limit",
        message: "Registry name and value evidence exceeded the remaining SystemDiff Collector budget.",
        stage: "enumerate",
        native_code: None,
    }
}

pub(crate) trait RegistryDataSource {
    fn detect_layout(&mut self) -> Result<RegistryLayout, SourceFailure>;

    fn read_key_once(
        &mut self,
        target: &RegistryTarget,
        available_evidence_bytes: usize,
    ) -> ReadKeyResult;
}

fn collect_with_source<S: RegistryDataSource>(
    source: &mut S,
    context: &CollectionContext,
) -> CollectionOutcome {
    collect_with_source_and_budget(source, context, MAX_REGISTRY_COLLECTOR_EVIDENCE_BYTES)
}

fn collect_with_source_and_budget<S: RegistryDataSource>(
    source: &mut S,
    _context: &CollectionContext,
    collector_evidence_budget: usize,
) -> CollectionOutcome {
    let (targets, mut precomputed_scopes) = match source.detect_layout() {
        Ok(layout) => targets_for_layout(layout),
        Err(failure) => targets_for_layout_failure(failure),
    };

    let mut remaining_evidence_bytes = collector_evidence_budget;
    let mut scopes = Vec::new();
    let mut observations = Vec::new();

    for target in targets {
        let result = collect_target(source, &target, remaining_evidence_bytes);
        remaining_evidence_bytes =
            remaining_evidence_bytes.saturating_sub(result.retained_evidence_bytes);
        scopes.push(result.scope);
        observations.extend(result.observations);
    }
    scopes.append(&mut precomputed_scopes);
    scopes.sort_by(|left, right| left.coverage.scope_id.cmp(&right.coverage.scope_id));
    observations.sort_by_key(|observation| observation.key());

    let status = aggregate_status(scopes.iter().map(|scope| scope.coverage.status));
    let coverage = scopes.iter().map(|scope| scope.coverage.clone()).collect();
    let mut diagnostics: Vec<_> = scopes
        .into_iter()
        .flat_map(|scope| scope.diagnostics)
        .collect();
    diagnostics.sort_by(|left, right| {
        (&left.scope_id, &left.code, &left.stage, left.native_code).cmp(&(
            &right.scope_id,
            &right.code,
            &right.stage,
            right.native_code,
        ))
    });

    CollectionOutcome {
        run: CollectorRun {
            id: REGISTRY_STARTUP_COLLECTOR_ID.to_owned(),
            version: REGISTRY_STARTUP_COLLECTOR_VERSION,
            status,
            coverage,
            diagnostics,
        },
        observations,
    }
}

struct TargetCollection {
    scope: ScopeResult,
    observations: Vec<Observation>,
    retained_evidence_bytes: usize,
}

fn collect_target<S: RegistryDataSource>(
    source: &mut S,
    target: &RegistryTarget,
    available_evidence_bytes: usize,
) -> TargetCollection {
    let mut last_attempt = None;
    let mut last_key_deleted = None;

    for attempt_index in 0..MAX_SCOPE_ATTEMPTS {
        match source.read_key_once(target, available_evidence_bytes) {
            ReadKeyResult::Missing => {
                return TargetCollection::empty(target.scope_id, CollectorStatus::Complete);
            }
            ReadKeyResult::Failed(failure)
                if failure.kind == SourceFailureKind::KeyDeleted
                    && attempt_index + 1 < MAX_SCOPE_ATTEMPTS =>
            {
                last_key_deleted = Some(failure);
            }
            ReadKeyResult::Failed(failure) if failure.kind == SourceFailureKind::KeyDeleted => {
                last_key_deleted = Some(failure);
                break;
            }
            ReadKeyResult::Failed(failure) => {
                let status = if failure.kind == SourceFailureKind::AccessDenied {
                    CollectorStatus::PermissionDenied
                } else {
                    CollectorStatus::Failed
                };
                return TargetCollection {
                    scope: ScopeResult {
                        coverage: ScopeCoverage {
                            scope_id: target.scope_id.to_owned(),
                            status,
                        },
                        diagnostics: vec![diagnostic_from_failure(target.scope_id, &failure)],
                    },
                    observations: Vec::new(),
                    retained_evidence_bytes: 0,
                };
            }
            ReadKeyResult::Present(attempt) if attempt.stable => {
                return build_target_collection(target, attempt, available_evidence_bytes);
            }
            ReadKeyResult::Present(attempt) => {
                last_attempt = Some(attempt);
            }
        }
    }

    if let Some(attempt) = last_attempt {
        build_target_collection(target, attempt, available_evidence_bytes)
    } else {
        let failure = last_key_deleted.unwrap_or(SourceFailure {
            kind: SourceFailureKind::KeyDeleted,
            stage: "enumerate",
            native_code: 1018,
        });
        TargetCollection {
            scope: ScopeResult {
                coverage: ScopeCoverage {
                    scope_id: target.scope_id.to_owned(),
                    status: CollectorStatus::Partial,
                },
                diagnostics: vec![Diagnostic {
                    code: "registry_changed_during_scan".to_owned(),
                    message: "The Registry key changed during bounded collection retries."
                        .to_owned(),
                    stage: Some(failure.stage.to_owned()),
                    native_code: Some(failure.native_code),
                    scope_id: Some(target.scope_id.to_owned()),
                }],
            },
            observations: Vec::new(),
            retained_evidence_bytes: 0,
        }
    }
}

fn build_target_collection(
    target: &RegistryTarget,
    attempt: KeyReadAttempt,
    available_evidence_bytes: usize,
) -> TargetCollection {
    let KeyReadAttempt {
        before,
        after,
        records,
        mut issues,
        stable,
    } = attempt;
    if !stable || before != after {
        issues.push(SourceIssue {
            code: "registry_changed_during_scan",
            message: "The Registry key changed during bounded collection retries.",
            stage: "enumerate",
            native_code: None,
        });
    }
    let mut by_identity: BTreeMap<String, Vec<RawRegistryValue>> = BTreeMap::new();
    for record in records {
        by_identity
            .entry(registry_value_identity(&record.name_utf16))
            .or_default()
            .push(record);
    }

    let mut observations = Vec::new();
    let mut retained_evidence_bytes = 0_usize;
    for (canonical_id, mut records) in by_identity {
        if records.len() != 1 {
            issues.push(SourceIssue {
                code: "registry_identity_collision",
                message: "Multiple Registry values produced one Collector identity.",
                stage: "normalize",
                native_code: None,
            });
            continue;
        }
        let record = records.remove(0);
        let Some(evidence_bytes) = native_evidence_bytes(&record) else {
            issues.push(registry_resource_limit_issue());
            continue;
        };
        let Some(next_retained) = retained_evidence_bytes.checked_add(evidence_bytes) else {
            issues.push(registry_resource_limit_issue());
            continue;
        };
        if next_retained > available_evidence_bytes {
            issues.push(registry_resource_limit_issue());
            continue;
        }
        retained_evidence_bytes = next_retained;
        observations.push(observation_from_raw(target, canonical_id, record));
    }
    observations.sort_by_key(|observation| observation.key());
    issues.sort_by(|left, right| {
        (left.code, left.stage, left.native_code, left.message).cmp(&(
            right.code,
            right.stage,
            right.native_code,
            right.message,
        ))
    });
    issues.dedup();

    let status = if issues.is_empty() {
        CollectorStatus::Complete
    } else {
        CollectorStatus::Partial
    };
    let diagnostics = issues
        .iter()
        .map(|issue| Diagnostic {
            code: issue.code.to_owned(),
            message: issue.message.to_owned(),
            stage: Some(issue.stage.to_owned()),
            native_code: issue.native_code,
            scope_id: Some(target.scope_id.to_owned()),
        })
        .collect();
    TargetCollection {
        scope: ScopeResult {
            coverage: ScopeCoverage {
                scope_id: target.scope_id.to_owned(),
                status,
            },
            diagnostics,
        },
        observations,
        retained_evidence_bytes,
    }
}

impl TargetCollection {
    fn empty(scope_id: &str, status: CollectorStatus) -> Self {
        Self {
            scope: ScopeResult {
                coverage: ScopeCoverage {
                    scope_id: scope_id.to_owned(),
                    status,
                },
                diagnostics: Vec::new(),
            },
            observations: Vec::new(),
            retained_evidence_bytes: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScopeResult {
    coverage: ScopeCoverage,
    diagnostics: Vec<Diagnostic>,
}

fn targets_for_layout(layout: RegistryLayout) -> (Vec<RegistryTarget>, Vec<ScopeResult>) {
    let mut targets = current_user_targets();
    let precomputed = match layout {
        RegistryLayout::X64 => {
            targets.extend(local_machine_alternate_targets());
            Vec::new()
        }
        RegistryLayout::X86 => {
            targets.extend(local_machine_native_targets());
            Vec::new()
        }
        RegistryLayout::Arm64 | RegistryLayout::Unknown(_) => unsupported_machine_scopes(),
    };
    (targets, precomputed)
}

fn targets_for_layout_failure(failure: SourceFailure) -> (Vec<RegistryTarget>, Vec<ScopeResult>) {
    let scopes = alternate_scope_ids()
        .into_iter()
        .map(|scope_id| ScopeResult {
            coverage: ScopeCoverage {
                scope_id: scope_id.to_owned(),
                status: CollectorStatus::Failed,
            },
            diagnostics: vec![Diagnostic {
                code: "registry_layout_failed".to_owned(),
                message: "Windows Registry view layout detection failed.".to_owned(),
                stage: Some(failure.stage.to_owned()),
                native_code: Some(failure.native_code),
                scope_id: Some(scope_id.to_owned()),
            }],
        })
        .collect();
    (current_user_targets(), scopes)
}

fn current_user_targets() -> Vec<RegistryTarget> {
    vec![
        target(
            "current_user.shared.run",
            RegistryRoot::CurrentUser,
            RegistryView::Shared,
            ViewSelector::Shared,
            RUN_KEY_PATH,
            RegistryStartupKind::Run,
        ),
        target(
            "current_user.shared.run_once",
            RegistryRoot::CurrentUser,
            RegistryView::Shared,
            ViewSelector::Shared,
            RUN_ONCE_KEY_PATH,
            RegistryStartupKind::RunOnce,
        ),
    ]
}

fn local_machine_native_targets() -> Vec<RegistryTarget> {
    vec![
        target(
            "local_machine.native.run",
            RegistryRoot::LocalMachine,
            RegistryView::Native,
            ViewSelector::Native,
            RUN_KEY_PATH,
            RegistryStartupKind::Run,
        ),
        target(
            "local_machine.native.run_once",
            RegistryRoot::LocalMachine,
            RegistryView::Native,
            ViewSelector::Native,
            RUN_ONCE_KEY_PATH,
            RegistryStartupKind::RunOnce,
        ),
    ]
}

fn local_machine_alternate_targets() -> Vec<RegistryTarget> {
    vec![
        target(
            "local_machine.registry32.run",
            RegistryRoot::LocalMachine,
            RegistryView::Registry32,
            ViewSelector::Registry32,
            RUN_KEY_PATH,
            RegistryStartupKind::Run,
        ),
        target(
            "local_machine.registry32.run_once",
            RegistryRoot::LocalMachine,
            RegistryView::Registry32,
            ViewSelector::Registry32,
            RUN_ONCE_KEY_PATH,
            RegistryStartupKind::RunOnce,
        ),
        target(
            "local_machine.registry64.run",
            RegistryRoot::LocalMachine,
            RegistryView::Registry64,
            ViewSelector::Registry64,
            RUN_KEY_PATH,
            RegistryStartupKind::Run,
        ),
        target(
            "local_machine.registry64.run_once",
            RegistryRoot::LocalMachine,
            RegistryView::Registry64,
            ViewSelector::Registry64,
            RUN_ONCE_KEY_PATH,
            RegistryStartupKind::RunOnce,
        ),
    ]
}

fn alternate_scope_ids() -> [&'static str; 4] {
    [
        "local_machine.registry32.run",
        "local_machine.registry32.run_once",
        "local_machine.registry64.run",
        "local_machine.registry64.run_once",
    ]
}

fn unsupported_machine_scopes() -> Vec<ScopeResult> {
    alternate_scope_ids()
        .into_iter()
        .map(|scope_id| ScopeResult {
            coverage: ScopeCoverage {
                scope_id: scope_id.to_owned(),
                status: CollectorStatus::Unsupported,
            },
            diagnostics: vec![Diagnostic {
                code: "registry_layout_unsupported".to_owned(),
                message: "This v1 Collector does not define HKLM alternate Registry views for the detected architecture.".to_owned(),
                stage: Some("detect_layout".to_owned()),
                native_code: None,
                scope_id: Some(scope_id.to_owned()),
            }],
        })
        .collect()
}

fn target(
    scope_id: &'static str,
    root: RegistryRoot,
    view: RegistryView,
    selector: ViewSelector,
    key_path: &'static str,
    startup_kind: RegistryStartupKind,
) -> RegistryTarget {
    RegistryTarget {
        scope_id,
        root,
        view,
        selector,
        key_path,
        startup_kind,
    }
}

fn observation_from_raw(
    target: &RegistryTarget,
    canonical_id: String,
    raw: RawRegistryValue,
) -> Observation {
    let run_once_prefix = match target.startup_kind {
        RegistryStartupKind::Run => None,
        RegistryStartupKind::RunOnce => Some(classify_run_once_prefix_units(&raw.name_utf16)),
    };
    Observation {
        collector_id: REGISTRY_STARTUP_COLLECTOR_ID.to_owned(),
        collector_version: REGISTRY_STARTUP_COLLECTOR_VERSION,
        scope_id: target.scope_id.to_owned(),
        canonical_id,
        artifact: Artifact::RegistryStartup(RegistryStartupEntry {
            hive: match target.root {
                RegistryRoot::CurrentUser => RegistryHive::CurrentUser,
                RegistryRoot::LocalMachine => RegistryHive::LocalMachine,
            },
            registry_view: target.view,
            key_path: target.key_path.to_owned(),
            value_name: RegistryValueName::from_utf16_units(&raw.name_utf16),
            startup_kind: target.startup_kind,
            run_once_prefix,
            value_type: raw.value_type,
            content_sha256: sha256_hex(&raw.data),
            decoding: decode_registry_value(raw.value_type, &raw.data),
            raw_evidence: None,
        }),
    }
}

pub(crate) fn registry_value_identity(name_utf16: &[u16]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_DOMAIN);
    let length = u32::try_from(name_utf16.len()).unwrap_or(u32::MAX);
    hasher.update(length.to_le_bytes());
    for unit in name_utf16 {
        hasher.update(unit.to_le_bytes());
    }
    lower_hex(&hasher.finalize())
}

fn sha256_hex(data: &[u8]) -> String {
    lower_hex(&Sha256::digest(data))
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

pub(crate) fn decode_registry_value(value_type: u32, data: &[u8]) -> RegistryValueDecoding {
    match value_type {
        1 => decode_single_string(data, false),
        2 => decode_single_string(data, true),
        4 if data.len() == 4 => RegistryValueDecoding::Decoded {
            value: RegistryDecodedValue::Dword {
                value: u32::from_le_bytes(data.try_into().expect("length was checked")),
            },
        },
        5 if data.len() == 4 => RegistryValueDecoding::Decoded {
            value: RegistryDecodedValue::Dword {
                value: u32::from_be_bytes(data.try_into().expect("length was checked")),
            },
        },
        7 => decode_multi_string(data),
        11 if data.len() == 8 => RegistryValueDecoding::Decoded {
            value: RegistryDecodedValue::Qword {
                value: u64::from_le_bytes(data.try_into().expect("length was checked")),
            },
        },
        4 | 5 | 11 => RegistryValueDecoding::InvalidData,
        0 | 3 | 6 | 8 | 9 | 10 => RegistryValueDecoding::NotApplicable,
        _ => RegistryValueDecoding::UnsupportedType,
    }
}

fn decode_single_string(data: &[u8], expandable: bool) -> RegistryValueDecoding {
    let Some(units) = utf16le_units(data) else {
        return RegistryValueDecoding::InvalidData;
    };
    let Some((&0, body)) = units.split_last() else {
        return RegistryValueDecoding::InvalidData;
    };
    if body.contains(&0) {
        return RegistryValueDecoding::InvalidData;
    }
    let Ok(value) = String::from_utf16(body) else {
        return RegistryValueDecoding::InvalidData;
    };
    RegistryValueDecoding::Decoded {
        value: if expandable {
            RegistryDecodedValue::ExpandString { value }
        } else {
            RegistryDecodedValue::String { value }
        },
    }
}

fn decode_multi_string(data: &[u8]) -> RegistryValueDecoding {
    let Some(units) = utf16le_units(data) else {
        return RegistryValueDecoding::InvalidData;
    };
    if units == [0, 0] {
        return RegistryValueDecoding::Decoded {
            value: RegistryDecodedValue::MultiString { values: Vec::new() },
        };
    }
    if units.len() < 2 || !units.ends_with(&[0, 0]) {
        return RegistryValueDecoding::InvalidData;
    }

    let mut values = Vec::new();
    let mut start = 0;
    let list_end = units.len() - 1;
    while start < list_end {
        let Some(relative_end) = units[start..list_end].iter().position(|unit| *unit == 0) else {
            return RegistryValueDecoding::InvalidData;
        };
        let end = start + relative_end;
        if end == start {
            return RegistryValueDecoding::InvalidData;
        }
        let Ok(value) = String::from_utf16(&units[start..end]) else {
            return RegistryValueDecoding::InvalidData;
        };
        values.push(value);
        start = end + 1;
    }
    RegistryValueDecoding::Decoded {
        value: RegistryDecodedValue::MultiString { values },
    }
}

fn utf16le_units(data: &[u8]) -> Option<Vec<u16>> {
    if !data.len().is_multiple_of(2) {
        return None;
    }
    Some(
        data.as_chunks::<2>()
            .0
            .iter()
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .collect(),
    )
}

fn diagnostic_from_failure(scope_id: &str, failure: &SourceFailure) -> Diagnostic {
    let (code, message) = match failure.kind {
        SourceFailureKind::AccessDenied => (
            "registry_access_denied",
            "The current token cannot read this Registry scope.",
        ),
        SourceFailureKind::KeyDeleted => (
            "registry_changed_during_scan",
            "The Registry key changed during bounded collection retries.",
        ),
        SourceFailureKind::Other => (
            match failure.stage {
                "open" => "registry_open_failed",
                "query" => "registry_query_failed",
                _ => "registry_enumeration_failed",
            },
            "The Registry scope could not be collected.",
        ),
    };
    Diagnostic {
        code: code.to_owned(),
        message: message.to_owned(),
        stage: Some(failure.stage.to_owned()),
        native_code: Some(failure.native_code),
        scope_id: Some(scope_id.to_owned()),
    }
}

fn aggregate_status(statuses: impl Iterator<Item = CollectorStatus>) -> CollectorStatus {
    let statuses: Vec<_> = statuses.collect();
    if statuses
        .iter()
        .all(|status| *status == CollectorStatus::Complete)
    {
        return CollectorStatus::Complete;
    }
    for terminal in [
        CollectorStatus::PermissionDenied,
        CollectorStatus::Unavailable,
        CollectorStatus::Unsupported,
        CollectorStatus::Failed,
    ] {
        if !statuses.is_empty() && statuses.iter().all(|status| *status == terminal) {
            return terminal;
        }
    }
    CollectorStatus::Partial
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use systemdiff_core::{PrivilegeState, RunOncePrefixSemantics};

    fn context() -> CollectionContext {
        CollectionContext {
            privilege: PrivilegeState::StandardUser,
        }
    }

    fn raw(name: &[u16], value_type: u32, data: &[u8]) -> RawRegistryValue {
        RawRegistryValue {
            name_utf16: name.to_vec(),
            value_type,
            data: data.to_vec(),
        }
    }

    fn stable(records: Vec<RawRegistryValue>) -> ReadKeyResult {
        let metadata = KeyMetadata {
            value_count: records.len() as u32,
            last_write: 1,
        };
        ReadKeyResult::Present(KeyReadAttempt {
            before: metadata,
            after: metadata,
            records,
            issues: Vec::new(),
            stable: true,
        })
    }

    struct FakeSource {
        layout: Result<RegistryLayout, SourceFailure>,
        reads: BTreeMap<&'static str, VecDeque<ReadKeyResult>>,
        requested_budgets: Vec<(&'static str, usize)>,
    }

    impl RegistryDataSource for FakeSource {
        fn detect_layout(&mut self) -> Result<RegistryLayout, SourceFailure> {
            self.layout.clone()
        }

        fn read_key_once(
            &mut self,
            target: &RegistryTarget,
            available_evidence_bytes: usize,
        ) -> ReadKeyResult {
            self.requested_budgets
                .push((target.scope_id, available_evidence_bytes));
            self.reads
                .get_mut(target.scope_id)
                .and_then(VecDeque::pop_front)
                .unwrap_or(ReadKeyResult::Missing)
        }
    }

    fn fake(layout: RegistryLayout) -> FakeSource {
        FakeSource {
            layout: Ok(layout),
            reads: BTreeMap::new(),
            requested_budgets: Vec::new(),
        }
    }

    #[test]
    fn target_plans_have_explicit_views() {
        let (x64, unsupported) = targets_for_layout(RegistryLayout::X64);
        assert!(unsupported.is_empty());
        assert_eq!(x64.len(), 6);
        assert_eq!(x64[0].view, RegistryView::Shared);
        assert_eq!(x64[2].view, RegistryView::Registry32);
        assert_eq!(x64[4].view, RegistryView::Registry64);

        let (x86, unsupported) = targets_for_layout(RegistryLayout::X86);
        assert!(unsupported.is_empty());
        assert_eq!(x86.len(), 4);
        assert_eq!(x86[2].view, RegistryView::Native);

        let (arm, unsupported) = targets_for_layout(RegistryLayout::Arm64);
        assert_eq!(arm.len(), 2);
        assert_eq!(unsupported.len(), 4);
        assert!(
            unsupported
                .iter()
                .all(|scope| scope.coverage.status == CollectorStatus::Unsupported)
        );
    }

    #[test]
    fn exact_identity_is_stable_and_preserves_prefixes_and_empty_name() {
        let foo: Vec<_> = "Foo".encode_utf16().collect();
        let alternate_case: Vec<_> = "foo".encode_utf16().collect();
        let bang: Vec<_> = "!Foo".encode_utf16().collect();
        let star: Vec<_> = "*Foo".encode_utf16().collect();

        assert_eq!(registry_value_identity(&foo), registry_value_identity(&foo));
        assert_ne!(
            registry_value_identity(&foo),
            registry_value_identity(&alternate_case),
            "Collector v1 deliberately keeps exact casing as a documented limitation"
        );
        assert_ne!(
            registry_value_identity(&foo),
            registry_value_identity(&bang)
        );
        assert_ne!(
            registry_value_identity(&foo),
            registry_value_identity(&star)
        );
        assert_eq!(
            registry_value_identity(&[]),
            "599fd94bf0b72f0bf876ae79e94171025d075b9d199c668ebafc647050701c4b"
        );
        assert_eq!(
            registry_value_identity(&foo),
            "e300b1f49c3d61d973561e229a5b174ff27312a0df7c72801f6db2e8bd256a9e"
        );
        for (units, expected) in [
            (
                vec![0x66, 0x6f, 0x6f],
                "08462b31c01a23a6feb5049cb0880de67353b551fdb7d0d62324dded602d5e8c",
            ),
            (
                vec![0x21, 0x46, 0x6f, 0x6f],
                "246bb80ad302e1d428b58825421b6ec88d372e0e7d68dcf60185332d7607d833",
            ),
            (
                vec![0x2a, 0x46, 0x6f, 0x6f],
                "23c891afd1729eb817401b675d713aea7b9acfa5e9be7103a3c3e522c7edec94",
            ),
            (
                vec![0x61, 0x7c, 0x62],
                "118571aa221e07649dbb40210959903b5de36297f1ed88d1335651fb005314ff",
            ),
            (
                vec![0x00c5],
                "f02e721daa1064b51f301672cfa9395c953e0a3d9b94b12fddb8c9c79aaf4ae0",
            ),
            (
                vec![0xd800],
                "5ee36fbdb7c75bf2b14efdc3f6ef779b4bcc89dd0ee04d4eda22067e411ed5de",
            ),
        ] {
            assert_eq!(registry_value_identity(&units), expected);
        }
    }

    #[test]
    fn strict_decoding_covers_supported_and_malformed_values() {
        let utf16 = |value: &str| {
            value
                .encode_utf16()
                .chain([0])
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>()
        };
        assert!(matches!(
            decode_registry_value(1, &utf16("value")),
            RegistryValueDecoding::Decoded {
                value: RegistryDecodedValue::String { ref value }
            } if value == "value"
        ));
        assert!(matches!(
            decode_registry_value(2, &utf16("%PATH%")),
            RegistryValueDecoding::Decoded {
                value: RegistryDecodedValue::ExpandString { ref value }
            } if value == "%PATH%"
        ));
        assert!(matches!(
            decode_registry_value(4, &42_u32.to_le_bytes()),
            RegistryValueDecoding::Decoded {
                value: RegistryDecodedValue::Dword { value: 42 }
            }
        ));
        assert!(matches!(
            decode_registry_value(5, &42_u32.to_be_bytes()),
            RegistryValueDecoding::Decoded {
                value: RegistryDecodedValue::Dword { value: 42 }
            }
        ));
        assert!(matches!(
            decode_registry_value(11, &42_u64.to_le_bytes()),
            RegistryValueDecoding::Decoded {
                value: RegistryDecodedValue::Qword { value: 42 }
            }
        ));
        assert_eq!(
            decode_registry_value(3, &[]),
            RegistryValueDecoding::NotApplicable
        );
        assert_eq!(
            decode_registry_value(99, &[]),
            RegistryValueDecoding::UnsupportedType
        );
        assert_eq!(
            decode_registry_value(1, &[]),
            RegistryValueDecoding::InvalidData
        );
        assert_eq!(
            decode_registry_value(1, &[0]),
            RegistryValueDecoding::InvalidData
        );
        assert_eq!(
            decode_registry_value(1, &[0x00, 0xd8, 0, 0]),
            RegistryValueDecoding::InvalidData
        );
    }

    #[test]
    fn multi_string_requires_documented_termination_and_preserves_order() {
        let units = [
            b'o' as u16,
            b'n' as u16,
            b'e' as u16,
            0,
            b't' as u16,
            b'w' as u16,
            b'o' as u16,
            0,
            0,
        ];
        let bytes: Vec<_> = units.into_iter().flat_map(u16::to_le_bytes).collect();
        assert!(matches!(
            decode_registry_value(7, &bytes),
            RegistryValueDecoding::Decoded {
                value: RegistryDecodedValue::MultiString { ref values }
            } if values == &["one", "two"]
        ));
        assert!(matches!(
            decode_registry_value(7, &[0, 0, 0, 0]),
            RegistryValueDecoding::Decoded {
                value: RegistryDecodedValue::MultiString { ref values }
            } if values.is_empty()
        ));
        assert_eq!(
            decode_registry_value(7, &[0, 0]),
            RegistryValueDecoding::InvalidData
        );
        assert_eq!(
            decode_registry_value(7, &[b'o', 0, 0, 0]),
            RegistryValueDecoding::InvalidData
        );
    }

    #[test]
    fn missing_denied_and_mixed_scopes_degrade_independently() {
        let mut source = fake(RegistryLayout::X86);
        source.reads.insert(
            "current_user.shared.run",
            VecDeque::from([ReadKeyResult::Failed(SourceFailure {
                kind: SourceFailureKind::AccessDenied,
                stage: "open",
                native_code: 5,
            })]),
        );
        let outcome = collect_with_source(&mut source, &context());
        assert_eq!(outcome.run.status, CollectorStatus::Partial);
        assert_eq!(
            outcome.run.coverage[0].status,
            CollectorStatus::PermissionDenied
        );
        assert!(
            outcome
                .run
                .coverage
                .iter()
                .skip(1)
                .all(|coverage| coverage.status == CollectorStatus::Complete)
        );
    }

    #[test]
    fn bounded_mutation_retry_uses_only_stable_attempt() {
        let mut source = fake(RegistryLayout::X86);
        let name: Vec<_> = "Stable".encode_utf16().collect();
        let unstable = ReadKeyResult::Present(KeyReadAttempt {
            before: KeyMetadata {
                value_count: 0,
                last_write: 1,
            },
            after: KeyMetadata {
                value_count: 1,
                last_write: 2,
            },
            records: vec![raw(&name, 3, b"old")],
            issues: Vec::new(),
            stable: false,
        });
        source.reads.insert(
            "current_user.shared.run",
            VecDeque::from([unstable, stable(vec![raw(&name, 3, b"new")])]),
        );
        let outcome = collect_with_source(&mut source, &context());
        assert_eq!(outcome.run.status, CollectorStatus::Complete);
        assert_eq!(outcome.observations.len(), 1);
        let Artifact::RegistryStartup(entry) = &outcome.observations[0].artifact else {
            panic!("Registry evidence expected");
        };
        assert_eq!(entry.content_sha256, sha256_hex(b"new"));
    }

    #[test]
    fn exhausted_mutation_retry_retains_only_final_complete_records_as_partial() {
        let mut source = fake(RegistryLayout::X86);
        let attempts = [
            b"first".as_slice(),
            b"second".as_slice(),
            b"final".as_slice(),
        ]
        .into_iter()
        .map(|data| {
            ReadKeyResult::Present(KeyReadAttempt {
                before: KeyMetadata {
                    value_count: 0,
                    last_write: 1,
                },
                after: KeyMetadata {
                    value_count: 1,
                    last_write: 2,
                },
                records: vec![raw(&"Changing".encode_utf16().collect::<Vec<_>>(), 3, data)],
                issues: Vec::new(),
                stable: false,
            })
        })
        .collect::<VecDeque<_>>();
        source.reads.insert("current_user.shared.run", attempts);

        let outcome = collect_with_source(&mut source, &context());
        assert_eq!(outcome.run.status, CollectorStatus::Partial);
        assert_eq!(outcome.observations.len(), 1);
        let Artifact::RegistryStartup(entry) = &outcome.observations[0].artifact else {
            panic!("Registry evidence expected");
        };
        assert_eq!(entry.content_sha256, sha256_hex(b"final"));
        assert_eq!(
            outcome
                .run
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "registry_changed_during_scan")
                .count(),
            1
        );
    }

    #[test]
    fn normalization_is_independent_of_enumeration_order() {
        let alpha = raw(&"Alpha".encode_utf16().collect::<Vec<_>>(), 3, b"a");
        let beta = raw(&"Beta".encode_utf16().collect::<Vec<_>>(), 3, b"b");
        let mut forward = fake(RegistryLayout::X86);
        forward.reads.insert(
            "current_user.shared.run",
            VecDeque::from([stable(vec![alpha.clone(), beta.clone()])]),
        );
        let mut reverse = fake(RegistryLayout::X86);
        reverse.reads.insert(
            "current_user.shared.run",
            VecDeque::from([stable(vec![beta, alpha])]),
        );

        assert_eq!(
            collect_with_source(&mut forward, &context()),
            collect_with_source(&mut reverse, &context())
        );
    }

    #[test]
    fn duplicate_exact_names_are_omitted_as_an_identity_collision() {
        let name: Vec<_> = "Duplicate".encode_utf16().collect();
        let mut source = fake(RegistryLayout::X86);
        source.reads.insert(
            "current_user.shared.run",
            VecDeque::from([stable(vec![
                raw(&name, 3, b"first"),
                raw(&name, 3, b"second"),
            ])]),
        );

        let outcome = collect_with_source(&mut source, &context());
        assert_eq!(outcome.run.status, CollectorStatus::Partial);
        assert!(outcome.observations.is_empty());
        assert!(outcome.run.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "registry_identity_collision"
                && diagnostic.scope_id.as_deref() == Some("current_user.shared.run")
        }));
    }

    #[test]
    fn partial_scope_retains_complete_sibling_records() {
        let mut source = fake(RegistryLayout::X86);
        let normal: Vec<_> = "Normal".encode_utf16().collect();
        let metadata = KeyMetadata {
            value_count: 2,
            last_write: 1,
        };
        source.reads.insert(
            "current_user.shared.run",
            VecDeque::from([ReadKeyResult::Present(KeyReadAttempt {
                before: metadata,
                after: metadata,
                records: vec![raw(&normal, 3, b"complete")],
                issues: vec![SourceIssue {
                    code: "registry_value_too_large",
                    message: "A Registry value exceeded the SystemDiff per-value capture limit.",
                    stage: "enumerate",
                    native_code: None,
                }],
                stable: true,
            })]),
        );
        let outcome = collect_with_source(&mut source, &context());
        assert_eq!(outcome.run.status, CollectorStatus::Partial);
        assert_eq!(outcome.observations.len(), 1);
        assert!(outcome.run.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "registry_value_too_large"
                && diagnostic.scope_id.as_deref() == Some("current_user.shared.run")
        }));
    }

    #[test]
    fn evidence_budget_counts_long_names_and_retains_a_fitting_sibling() {
        let target = current_user_targets().remove(0);
        let records = vec![
            raw(&"LongNameOne".encode_utf16().collect::<Vec<_>>(), 3, &[]),
            raw(&"LongNameTwo".encode_utf16().collect::<Vec<_>>(), 3, &[]),
            raw(&"A".encode_utf16().collect::<Vec<_>>(), 3, &[]),
        ];
        let metadata = KeyMetadata {
            value_count: records.len() as u32,
            last_write: 1,
        };
        let result = build_target_collection(
            &target,
            KeyReadAttempt {
                before: metadata,
                after: metadata,
                records,
                issues: Vec::new(),
                stable: true,
            },
            size_of::<u16>(),
        );

        assert_eq!(result.scope.coverage.status, CollectorStatus::Partial);
        assert_eq!(result.retained_evidence_bytes, size_of::<u16>());
        assert_eq!(result.observations.len(), 1);
        let Artifact::RegistryStartup(entry) = &result.observations[0].artifact else {
            panic!("Registry evidence expected");
        };
        assert_eq!(entry.value_name, RegistryValueName::decoded("A"));
        assert!(
            result
                .scope
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "registry_resource_limit")
        );
    }

    #[test]
    fn evidence_budget_is_decremented_across_registry_scopes() {
        let mut source = fake(RegistryLayout::X86);
        source.reads.insert(
            "current_user.shared.run",
            VecDeque::from([stable(vec![raw(
                &"A".encode_utf16().collect::<Vec<_>>(),
                3,
                &[0; 6],
            )])]),
        );
        source.reads.insert(
            "current_user.shared.run_once",
            VecDeque::from([stable(vec![raw(
                &"B".encode_utf16().collect::<Vec<_>>(),
                3,
                &[0; 4],
            )])]),
        );

        let outcome = collect_with_source_and_budget(&mut source, &context(), 12);
        assert_eq!(source.requested_budgets[0].1, 12);
        assert_eq!(source.requested_budgets[1].1, 4);
        assert_eq!(outcome.observations.len(), 1);
        assert_eq!(outcome.run.status, CollectorStatus::Partial);
        assert!(outcome.run.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "registry_resource_limit"
                && diagnostic.scope_id.as_deref() == Some("current_user.shared.run_once")
        }));
    }

    #[test]
    fn unnamed_run_once_value_is_observed_without_marker_corruption() {
        let target = current_user_targets().remove(1);
        let observation =
            observation_from_raw(&target, registry_value_identity(&[]), raw(&[], 3, &[]));
        let Artifact::RegistryStartup(entry) = observation.artifact else {
            panic!("Registry evidence expected");
        };
        assert_eq!(entry.value_name, RegistryValueName::decoded(""));
        assert_eq!(
            entry.run_once_prefix,
            Some(RunOncePrefixSemantics::NoDocumentedPrefix)
        );
    }
}
