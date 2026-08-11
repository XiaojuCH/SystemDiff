#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

pub const SNAPSHOT_DOCUMENT_TYPE: &str = "systemdiff.snapshot";
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;
pub const REGISTRY_RAW_EVIDENCE_MAX_CAPTURE_BYTES: u64 = 4_096;
pub const REGISTRY_VALUE_NAME_MAX_UTF16_UNITS: usize = 16_383;

#[derive(Debug, Deserialize)]
struct SnapshotDocumentHeader {
    document_type: String,
    schema_version: u32,
}

/// Routes a bounded JSON document to a supported Snapshot wire type.
///
/// The caller owns transport-specific resource limits. This function inspects
/// `document_type` and `schema_version` before constructing the full v1
/// [`Snapshot`].
pub fn decode_snapshot_document(input: &[u8]) -> Result<Snapshot, SnapshotDocumentError> {
    let header: SnapshotDocumentHeader =
        serde_json::from_slice(input).map_err(SnapshotDocumentError::InvalidHeader)?;

    if header.document_type != SNAPSHOT_DOCUMENT_TYPE {
        return Err(SnapshotDocumentError::UnexpectedDocumentType {
            found: header.document_type,
        });
    }

    match header.schema_version {
        SNAPSHOT_SCHEMA_VERSION => {
            serde_json::from_slice(input).map_err(SnapshotDocumentError::InvalidSnapshotV1)
        }
        found => Err(SnapshotDocumentError::UnsupportedSchemaVersion { found }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub document_type: String,
    pub schema_version: u32,
    pub systemdiff_version: String,
    pub captured_at: String,
    pub host: HostMetadata,
    pub privilege: PrivilegeState,
    pub enabled_collectors: Vec<String>,
    pub collectors: Vec<CollectorRun>,
    pub redaction: RedactionMetadata,
    pub observations: Vec<Observation>,
}

impl Snapshot {
    pub fn validate(&self) -> Result<(), SnapshotValidationError> {
        if self.document_type != SNAPSHOT_DOCUMENT_TYPE {
            return Err(SnapshotValidationError::UnexpectedDocumentType {
                found: self.document_type.clone(),
            });
        }
        if self.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(SnapshotValidationError::UnsupportedSchemaVersion {
                found: self.schema_version,
            });
        }
        if self.systemdiff_version.trim().is_empty() {
            return Err(SnapshotValidationError::EmptyField("systemdiff_version"));
        }
        if self.captured_at.trim().is_empty() {
            return Err(SnapshotValidationError::EmptyField("captured_at"));
        }
        let captured_at = OffsetDateTime::parse(&self.captured_at, &Rfc3339)
            .map_err(|_| SnapshotValidationError::InvalidCapturedAt)?;
        let has_known_utc_designator =
            self.captured_at.ends_with('Z') || self.captured_at.ends_with("+00:00");
        if captured_at.offset() != UtcOffset::UTC || !has_known_utc_designator {
            return Err(SnapshotValidationError::NonUtcCapturedAt);
        }

        let mut enabled = BTreeSet::new();
        for collector_id in &self.enabled_collectors {
            if collector_id.trim().is_empty() {
                return Err(SnapshotValidationError::EmptyField("enabled_collectors[]"));
            }
            if !enabled.insert(collector_id.as_str()) {
                return Err(SnapshotValidationError::DuplicateCollector(
                    collector_id.clone(),
                ));
            }
        }

        let mut runs = BTreeMap::new();
        for run in &self.collectors {
            if run.id.trim().is_empty() {
                return Err(SnapshotValidationError::EmptyField("collectors[].id"));
            }
            if run.version == 0 {
                return Err(SnapshotValidationError::InvalidCollectorVersion {
                    collector_id: run.id.clone(),
                });
            }
            if runs.insert(run.id.as_str(), run).is_some() {
                return Err(SnapshotValidationError::DuplicateCollector(run.id.clone()));
            }
            if !enabled.contains(run.id.as_str()) {
                return Err(SnapshotValidationError::UnexpectedCollectorRun(
                    run.id.clone(),
                ));
            }

            if run.coverage.is_empty() {
                return Err(SnapshotValidationError::EmptyCollectorCoverage(
                    run.id.clone(),
                ));
            }

            let all_scopes_complete = run
                .coverage
                .iter()
                .all(|coverage| coverage.status == CollectorStatus::Complete);
            let any_scope_complete = run
                .coverage
                .iter()
                .any(|coverage| coverage.status == CollectorStatus::Complete);
            let status_is_consistent = match run.status {
                CollectorStatus::Complete => all_scopes_complete,
                CollectorStatus::Partial => !all_scopes_complete,
                CollectorStatus::PermissionDenied
                | CollectorStatus::Unavailable
                | CollectorStatus::Unsupported
                | CollectorStatus::Failed => !any_scope_complete,
            };
            if !status_is_consistent {
                return Err(SnapshotValidationError::InconsistentCollectorStatus {
                    collector_id: run.id.clone(),
                    status: run.status,
                });
            }

            let mut scopes = BTreeSet::new();
            for coverage in &run.coverage {
                if coverage.scope_id.trim().is_empty() {
                    return Err(SnapshotValidationError::EmptyField(
                        "collectors[].coverage[].scope_id",
                    ));
                }
                if !scopes.insert(coverage.scope_id.as_str()) {
                    return Err(SnapshotValidationError::DuplicateCoverage {
                        collector_id: run.id.clone(),
                        scope_id: coverage.scope_id.clone(),
                    });
                }
            }

            for diagnostic in &run.diagnostics {
                if diagnostic.code.trim().is_empty() {
                    return Err(SnapshotValidationError::EmptyField(
                        "collectors[].diagnostics[].code",
                    ));
                }
                if let Some(scope_id) = &diagnostic.scope_id {
                    if scope_id.trim().is_empty() {
                        return Err(SnapshotValidationError::EmptyField(
                            "collectors[].diagnostics[].scope_id",
                        ));
                    }
                    if !scopes.contains(scope_id.as_str()) {
                        return Err(SnapshotValidationError::DiagnosticReferencesUnknownScope {
                            collector_id: run.id.clone(),
                            scope_id: scope_id.clone(),
                        });
                    }
                }
            }
        }

        for collector_id in &self.enabled_collectors {
            if !runs.contains_key(collector_id.as_str()) {
                return Err(SnapshotValidationError::MissingCollectorRun(
                    collector_id.clone(),
                ));
            }
        }

        let mut observation_keys = BTreeSet::new();
        for observation in &self.observations {
            let run = runs.get(observation.collector_id.as_str()).ok_or_else(|| {
                SnapshotValidationError::MissingCollectorRun(observation.collector_id.clone())
            })?;
            if run.version != observation.collector_version {
                return Err(SnapshotValidationError::CollectorVersionMismatch {
                    collector_id: observation.collector_id.clone(),
                    run_version: run.version,
                    observation_version: observation.collector_version,
                });
            }
            if !matches!(
                run.status,
                CollectorStatus::Complete | CollectorStatus::Partial
            ) {
                return Err(
                    SnapshotValidationError::ObservationFromUnavailableCollector {
                        collector_id: observation.collector_id.clone(),
                        status: run.status,
                    },
                );
            }
            let scope = run
                .coverage
                .iter()
                .find(|coverage| coverage.scope_id == observation.scope_id)
                .ok_or_else(|| SnapshotValidationError::MissingCoverage {
                    collector_id: observation.collector_id.clone(),
                    scope_id: observation.scope_id.clone(),
                })?;
            if !matches!(
                scope.status,
                CollectorStatus::Complete | CollectorStatus::Partial
            ) {
                return Err(SnapshotValidationError::ObservationFromUnavailableScope {
                    collector_id: observation.collector_id.clone(),
                    scope_id: observation.scope_id.clone(),
                    status: scope.status,
                });
            }
            if observation.canonical_id.trim().is_empty() {
                return Err(SnapshotValidationError::EmptyField(
                    "observations[].canonical_id",
                ));
            }
            if let Artifact::RegistryStartup(entry) = &observation.artifact {
                validate_registry_startup_entry(entry)?;
            }

            let key = observation.key();
            if !observation_keys.insert(key.clone()) {
                return Err(SnapshotValidationError::DuplicateObservation(key));
            }
        }

        Ok(())
    }

    pub fn scope_status(&self, key: &ArtifactKey) -> Option<CollectorStatus> {
        self.collectors
            .iter()
            .find(|run| run.id == key.collector_id)
            .and_then(|run| {
                run.coverage
                    .iter()
                    .find(|coverage| coverage.scope_id == key.scope_id)
                    .map(|coverage| match run.status {
                        CollectorStatus::Complete | CollectorStatus::Partial => coverage.status,
                        status => status,
                    })
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostMetadata {
    pub windows_version: Option<String>,
    pub windows_build: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivilegeState {
    StandardUser,
    Elevated,
    System,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionMetadata {
    pub status: RedactionStatus,
    pub policy: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionStatus {
    Unredacted,
    Redacted,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectorRun {
    pub id: String,
    pub version: u32,
    pub status: CollectorStatus,
    pub coverage: Vec<ScopeCoverage>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeCoverage {
    pub scope_id: String,
    pub status: CollectorStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectorStatus {
    Complete,
    Partial,
    PermissionDenied,
    Unavailable,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub stage: Option<String>,
    pub native_code: Option<i64>,
    #[serde(default)]
    pub scope_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub collector_id: String,
    pub collector_version: u32,
    pub scope_id: String,
    pub canonical_id: String,
    pub artifact: Artifact,
}

impl Observation {
    pub fn key(&self) -> ArtifactKey {
        ArtifactKey {
            collector_id: self.collector_id.clone(),
            scope_id: self.scope_id.clone(),
            artifact_kind: self.artifact.kind().to_owned(),
            canonical_id: self.canonical_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ArtifactKey {
    pub collector_id: String,
    pub scope_id: String,
    pub artifact_kind: String,
    pub canonical_id: String,
}

impl fmt::Display for ArtifactKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}/{}/{}/{}",
            self.collector_id, self.scope_id, self.artifact_kind, self.canonical_id
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "evidence", rename_all = "snake_case")]
pub enum Artifact {
    RegistryStartup(RegistryStartupEntry),
    WindowsService(WindowsService),
    ScheduledTask(ScheduledTask),
}

impl Artifact {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::RegistryStartup(_) => "registry_startup",
            Self::WindowsService(_) => "windows_service",
            Self::ScheduledTask(_) => "scheduled_task",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryStartupEntry {
    pub hive: RegistryHive,
    pub registry_view: RegistryView,
    pub key_path: String,
    pub value_name: RegistryValueName,
    pub startup_kind: RegistryStartupKind,
    pub run_once_prefix: Option<RunOncePrefixSemantics>,
    pub value_type: u32,
    pub content_sha256: String,
    pub decoding: RegistryValueDecoding,
    pub raw_evidence: Option<RegistryRawEvidence>,
}

/// Lossless Registry value-name evidence.
///
/// Win32 returns UTF-16 code units. Valid Unicode remains convenient JSON text;
/// malformed UTF-16 remains exact lowercase UTF-16LE hex instead of being
/// replaced with U+FFFD.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "encoding", rename_all = "snake_case")]
pub enum RegistryValueName {
    Decoded { value: String },
    InvalidUtf16 { utf16le_hex: String },
}

impl RegistryValueName {
    pub fn decoded(value: impl Into<String>) -> Self {
        Self::Decoded {
            value: value.into(),
        }
    }

    pub fn from_utf16_units(units: &[u16]) -> Self {
        match String::from_utf16(units) {
            Ok(value) => Self::Decoded { value },
            Err(_) => Self::InvalidUtf16 {
                utf16le_hex: encode_utf16le_hex(units),
            },
        }
    }

    pub fn utf16_units(&self) -> Option<Vec<u16>> {
        match self {
            Self::Decoded { value } => Some(value.encode_utf16().collect()),
            Self::InvalidUtf16 { utf16le_hex } => decode_utf16le_hex(utf16le_hex),
        }
    }

    pub fn decoded_value(&self) -> Option<&str> {
        match self {
            Self::Decoded { value } => Some(value),
            Self::InvalidUtf16 { .. } => None,
        }
    }
}

/// Identifies which documented startup key produced an observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryStartupKind {
    Run,
    RunOnce,
}

/// Structured meaning derived from the complete RunOnce Registry value name.
///
/// Microsoft documents `!` as deferring value deletion until after the
/// command runs and `*` as allowing execution in Safe Mode. Combined,
/// repeated, and marker-only prefixes remain [`Self::Undocumented`] because
/// their behavior is not documented. The original value name remains the
/// authoritative raw evidence and identity input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOncePrefixSemantics {
    NoDocumentedPrefix,
    DeferDeletionUntilAfterRun,
    RunInSafeMode,
    Undocumented,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RegistryValueDecoding {
    Decoded { value: RegistryDecodedValue },
    NotApplicable,
    InvalidData,
    UnsupportedType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RegistryDecodedValue {
    String { value: String },
    ExpandString { value: String },
    MultiString { values: Vec<String> },
    Dword { value: u32 },
    Qword { value: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryRawEvidence {
    pub content_hex: String,
    pub captured_byte_count: u64,
    pub original_byte_count: u64,
    pub truncated: bool,
}

fn validate_registry_startup_entry(
    entry: &RegistryStartupEntry,
) -> Result<(), SnapshotValidationError> {
    let value_name_units = validate_registry_value_name(&entry.value_name)?;
    let key_kind = entry.key_path.rsplit('\\').next().and_then(|key_name| {
        if key_name.eq_ignore_ascii_case("Run") {
            Some(RegistryStartupKind::Run)
        } else if key_name.eq_ignore_ascii_case("RunOnce") {
            Some(RegistryStartupKind::RunOnce)
        } else {
            None
        }
    });
    if key_kind != Some(entry.startup_kind) {
        return Err(SnapshotValidationError::InvalidRegistryEvidence {
            field: "key_path/startup_kind",
        });
    }

    match entry.startup_kind {
        RegistryStartupKind::Run => {
            if entry.run_once_prefix.is_some() {
                return Err(SnapshotValidationError::InvalidRegistryEvidence {
                    field: "startup_kind/run_once_prefix",
                });
            }
        }
        RegistryStartupKind::RunOnce => {
            let expected = classify_run_once_prefix_units(&value_name_units);
            if entry.run_once_prefix != Some(expected) {
                return Err(SnapshotValidationError::InvalidRegistryEvidence {
                    field: "value_name/run_once_prefix",
                });
            }
        }
    }

    if !is_lower_hex(&entry.content_sha256, 64) {
        return Err(SnapshotValidationError::InvalidRegistryEvidence {
            field: "content_sha256",
        });
    }

    if let RegistryValueDecoding::Decoded { value } = &entry.decoding
        && !decoded_value_matches_type(entry.value_type, value)
    {
        return Err(SnapshotValidationError::InvalidRegistryEvidence {
            field: "value_type/decoding.value.kind",
        });
    }

    if let Some(raw) = &entry.raw_evidence {
        if raw.captured_byte_count > REGISTRY_RAW_EVIDENCE_MAX_CAPTURE_BYTES {
            return Err(SnapshotValidationError::InvalidRegistryEvidence {
                field: "raw_evidence.captured_byte_count",
            });
        }
        if raw.captured_byte_count > raw.original_byte_count
            || raw.truncated != (raw.captured_byte_count < raw.original_byte_count)
        {
            return Err(SnapshotValidationError::InvalidRegistryEvidence {
                field: "raw_evidence.truncation",
            });
        }
        let expected_hex_len = raw.captured_byte_count as usize * 2;
        if !is_lower_hex(&raw.content_hex, expected_hex_len) {
            return Err(SnapshotValidationError::InvalidRegistryEvidence {
                field: "raw_evidence.content_hex",
            });
        }
    }

    Ok(())
}

pub fn classify_run_once_prefix_units(value_name: &[u16]) -> RunOncePrefixSemantics {
    if let Some(remainder) = value_name.strip_prefix(&[u16::from(b'!')]) {
        if remainder.is_empty()
            || remainder.starts_with(&[u16::from(b'!')])
            || remainder.starts_with(&[u16::from(b'*')])
        {
            RunOncePrefixSemantics::Undocumented
        } else {
            RunOncePrefixSemantics::DeferDeletionUntilAfterRun
        }
    } else if let Some(remainder) = value_name.strip_prefix(&[u16::from(b'*')]) {
        if remainder.is_empty()
            || remainder.starts_with(&[u16::from(b'!')])
            || remainder.starts_with(&[u16::from(b'*')])
        {
            RunOncePrefixSemantics::Undocumented
        } else {
            RunOncePrefixSemantics::RunInSafeMode
        }
    } else {
        RunOncePrefixSemantics::NoDocumentedPrefix
    }
}

fn validate_registry_value_name(
    value_name: &RegistryValueName,
) -> Result<Vec<u16>, SnapshotValidationError> {
    let units =
        value_name
            .utf16_units()
            .ok_or(SnapshotValidationError::InvalidRegistryEvidence {
                field: "value_name",
            })?;
    if units.len() > REGISTRY_VALUE_NAME_MAX_UTF16_UNITS || units.contains(&0) {
        return Err(SnapshotValidationError::InvalidRegistryEvidence {
            field: "value_name",
        });
    }
    match value_name {
        RegistryValueName::Decoded { .. } => {}
        RegistryValueName::InvalidUtf16 { .. } => {
            if units.is_empty() || String::from_utf16(&units).is_ok() {
                return Err(SnapshotValidationError::InvalidRegistryEvidence {
                    field: "value_name",
                });
            }
        }
    }
    Ok(units)
}

fn encode_utf16le_hex(units: &[u16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(units.len().saturating_mul(4));
    for unit in units {
        for byte in unit.to_le_bytes() {
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    encoded
}

fn decode_utf16le_hex(encoded: &str) -> Option<Vec<u16>> {
    if !encoded.len().is_multiple_of(4) || !is_lower_hex(encoded, encoded.len()) {
        return None;
    }
    encoded
        .as_bytes()
        .chunks_exact(4)
        .map(|chunk| {
            let low = (hex_nibble(chunk[0])? << 4) | hex_nibble(chunk[1])?;
            let high = (hex_nibble(chunk[2])? << 4) | hex_nibble(chunk[3])?;
            Some(u16::from_le_bytes([low, high]))
        })
        .collect()
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn decoded_value_matches_type(value_type: u32, value: &RegistryDecodedValue) -> bool {
    match value {
        RegistryDecodedValue::String { .. } => value_type == 1,
        RegistryDecodedValue::ExpandString { .. } => value_type == 2,
        RegistryDecodedValue::MultiString { .. } => value_type == 7,
        RegistryDecodedValue::Dword { .. } => matches!(value_type, 4 | 5),
        RegistryDecodedValue::Qword { .. } => value_type == 11,
    }
}

fn is_lower_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryHive {
    CurrentUser,
    LocalMachine,
}

/// SystemDiff's explicit evidence label for the Registry view used to collect a key.
///
/// A Collector must choose a variant from the target key's documented Windows
/// semantics. It must never infer a stable view from the Collector process
/// bitness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistryView {
    /// The key is documented by Microsoft as shared across WOW64 logical views
    /// and is collected once. WOW64 view selectors do not create distinct data.
    #[serde(rename = "shared")]
    Shared,

    /// The key has one system view because no WOW64 alternate logical views
    /// exist for that key on the target Windows installation.
    ///
    /// This must not be used for a redirected key by omitting a WOW64 selector;
    /// such a default would vary with Collector process bitness.
    #[serde(rename = "native")]
    Native,

    /// The 32-bit logical Registry view selected explicitly with
    /// `KEY_WOW64_32KEY` where alternate views exist.
    #[serde(rename = "registry32")]
    Registry32,

    /// The 64-bit logical Registry view selected explicitly with
    /// `KEY_WOW64_64KEY` where alternate views exist.
    #[serde(rename = "registry64")]
    Registry64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowsService {
    pub service_name: String,
    pub display_name: Option<String>,
    pub service_type: u32,
    pub start_type: u32,
    pub error_control: u32,
    pub binary_path: String,
    pub account: Option<String>,
    pub dependencies: Vec<String>,
    pub delayed_auto_start: Option<bool>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub task_path: String,
    pub enabled: bool,
    pub hidden: bool,
    pub principal: Option<TaskPrincipal>,
    pub actions: Vec<TaskAction>,
    pub raw_xml: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPrincipal {
    pub identity: Option<String>,
    pub logon_type: Option<String>,
    pub run_level: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskAction {
    Exec {
        command: String,
        arguments: Option<String>,
        working_directory: Option<String>,
    },
    Other {
        action_type: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectorDescriptor {
    pub id: String,
    pub version: u32,
    pub description: String,
    pub privilege: PrivilegeRequirement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivilegeRequirement {
    StandardUser,
    StandardUserPartial,
    Administrator,
    ObjectAclDependent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionContext {
    pub privilege: PrivilegeState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionOutcome {
    pub run: CollectorRun,
    pub observations: Vec<Observation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotMetadata {
    pub systemdiff_version: String,
    pub captured_at: String,
    pub host: HostMetadata,
    pub privilege: PrivilegeState,
    pub redaction: RedactionMetadata,
}

pub fn assemble_snapshot(
    metadata: SnapshotMetadata,
    mut outcomes: Vec<CollectionOutcome>,
) -> Result<Snapshot, SnapshotValidationError> {
    outcomes.sort_by(|left, right| left.run.id.cmp(&right.run.id));
    for outcome in &mut outcomes {
        outcome
            .run
            .coverage
            .sort_by(|left, right| left.scope_id.cmp(&right.scope_id));
        outcome.run.diagnostics.sort_by(|left, right| {
            (
                &left.scope_id,
                &left.code,
                &left.stage,
                left.native_code,
                &left.message,
            )
                .cmp(&(
                    &right.scope_id,
                    &right.code,
                    &right.stage,
                    right.native_code,
                    &right.message,
                ))
        });
        outcome
            .observations
            .sort_by_key(|observation| observation.key());
    }

    let enabled_collectors = outcomes
        .iter()
        .map(|outcome| outcome.run.id.clone())
        .collect();
    let collectors = outcomes.iter().map(|outcome| outcome.run.clone()).collect();
    let mut observations: Vec<_> = outcomes
        .into_iter()
        .flat_map(|outcome| outcome.observations)
        .collect();
    observations.sort_by_key(|observation| observation.key());

    let snapshot = Snapshot {
        document_type: SNAPSHOT_DOCUMENT_TYPE.to_owned(),
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        systemdiff_version: metadata.systemdiff_version,
        captured_at: metadata.captured_at,
        host: metadata.host,
        privilege: metadata.privilege,
        enabled_collectors,
        collectors,
        redaction: metadata.redaction,
        observations,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

pub trait Collector {
    fn descriptor(&self) -> CollectorDescriptor;
    fn collect(&self, context: &CollectionContext) -> CollectionOutcome;
}

#[derive(Debug)]
pub enum SnapshotDocumentError {
    InvalidHeader(serde_json::Error),
    UnexpectedDocumentType { found: String },
    UnsupportedSchemaVersion { found: u32 },
    InvalidSnapshotV1(serde_json::Error),
}

impl fmt::Display for SnapshotDocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader(error) => {
                write!(
                    formatter,
                    "failed to parse snapshot document header: {error}"
                )
            }
            Self::UnexpectedDocumentType { found } => {
                write!(formatter, "unexpected snapshot document type: {found}")
            }
            Self::UnsupportedSchemaVersion { found } => {
                write!(formatter, "unsupported snapshot schema version: {found}")
            }
            Self::InvalidSnapshotV1(error) => {
                write!(formatter, "failed to parse snapshot schema v1: {error}")
            }
        }
    }
}

impl Error for SnapshotDocumentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidHeader(error) | Self::InvalidSnapshotV1(error) => Some(error),
            Self::UnexpectedDocumentType { .. } | Self::UnsupportedSchemaVersion { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotValidationError {
    UnexpectedDocumentType {
        found: String,
    },
    UnsupportedSchemaVersion {
        found: u32,
    },
    EmptyField(&'static str),
    InvalidCapturedAt,
    NonUtcCapturedAt,
    InvalidCollectorVersion {
        collector_id: String,
    },
    DuplicateCollector(String),
    MissingCollectorRun(String),
    UnexpectedCollectorRun(String),
    EmptyCollectorCoverage(String),
    InconsistentCollectorStatus {
        collector_id: String,
        status: CollectorStatus,
    },
    DuplicateCoverage {
        collector_id: String,
        scope_id: String,
    },
    DiagnosticReferencesUnknownScope {
        collector_id: String,
        scope_id: String,
    },
    MissingCoverage {
        collector_id: String,
        scope_id: String,
    },
    CollectorVersionMismatch {
        collector_id: String,
        run_version: u32,
        observation_version: u32,
    },
    ObservationFromUnavailableCollector {
        collector_id: String,
        status: CollectorStatus,
    },
    ObservationFromUnavailableScope {
        collector_id: String,
        scope_id: String,
        status: CollectorStatus,
    },
    InvalidRegistryEvidence {
        field: &'static str,
    },
    DuplicateObservation(ArtifactKey),
}

impl fmt::Display for SnapshotValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedDocumentType { found } => {
                write!(formatter, "unexpected document type: {found}")
            }
            Self::UnsupportedSchemaVersion { found } => {
                write!(formatter, "unsupported snapshot schema version: {found}")
            }
            Self::EmptyField(field) => write!(formatter, "required field is empty: {field}"),
            Self::InvalidCapturedAt => {
                formatter.write_str("captured_at is not a valid RFC 3339 timestamp")
            }
            Self::NonUtcCapturedAt => {
                formatter.write_str("captured_at must use known UTC with Z or +00:00")
            }
            Self::InvalidCollectorVersion { collector_id } => {
                write!(formatter, "collector {collector_id} has version 0")
            }
            Self::DuplicateCollector(collector_id) => {
                write!(formatter, "duplicate collector: {collector_id}")
            }
            Self::MissingCollectorRun(collector_id) => {
                write!(formatter, "missing collector run: {collector_id}")
            }
            Self::UnexpectedCollectorRun(collector_id) => {
                write!(formatter, "collector run is not enabled: {collector_id}")
            }
            Self::EmptyCollectorCoverage(collector_id) => {
                write!(
                    formatter,
                    "collector has no coverage scopes: {collector_id}"
                )
            }
            Self::InconsistentCollectorStatus {
                collector_id,
                status,
            } => write!(
                formatter,
                "collector aggregate status is inconsistent with scope coverage for {collector_id}: {status:?}"
            ),
            Self::DuplicateCoverage {
                collector_id,
                scope_id,
            } => write!(
                formatter,
                "duplicate coverage scope for {collector_id}: {scope_id}"
            ),
            Self::DiagnosticReferencesUnknownScope {
                collector_id,
                scope_id,
            } => write!(
                formatter,
                "diagnostic references unknown coverage scope for {collector_id}: {scope_id}"
            ),
            Self::MissingCoverage {
                collector_id,
                scope_id,
            } => write!(
                formatter,
                "observation references missing coverage for {collector_id}: {scope_id}"
            ),
            Self::CollectorVersionMismatch {
                collector_id,
                run_version,
                observation_version,
            } => write!(
                formatter,
                "collector version mismatch for {collector_id}: run={run_version}, observation={observation_version}"
            ),
            Self::ObservationFromUnavailableCollector {
                collector_id,
                status,
            } => write!(
                formatter,
                "collector {collector_id} has observations despite aggregate status {status:?}"
            ),
            Self::ObservationFromUnavailableScope {
                collector_id,
                scope_id,
                status,
            } => write!(
                formatter,
                "collector {collector_id} has observations in unavailable scope {scope_id}: {status:?}"
            ),
            Self::InvalidRegistryEvidence { field } => {
                write!(formatter, "invalid Registry evidence: {field}")
            }
            Self::DuplicateObservation(key) => {
                write!(formatter, "duplicate observation identity: {key}")
            }
        }
    }
}

impl Error for SnapshotValidationError {}
