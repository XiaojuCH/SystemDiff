use crate::registry::{
    MAX_REGISTRY_VALUE_DATA_BYTES, MAX_REGISTRY_VALUES_PER_SCOPE, RawRegistryValue, SourceIssue,
    native_evidence_bytes, registry_resource_limit_issue, registry_value_identity,
};
use std::collections::BTreeMap;
use std::mem::size_of;
use systemdiff_core::REGISTRY_VALUE_NAME_MAX_UTF16_UNITS;

const MAX_BUFFER_GROWTH_ATTEMPTS: usize = 3;

#[derive(Default)]
struct BoundedRecords {
    by_identity: BTreeMap<String, Vec<RawRegistryValue>>,
    retained_evidence_bytes: usize,
    duplicate_exact_name: bool,
}

impl BoundedRecords {
    fn insert(&mut self, record: RawRegistryValue, evidence_budget: usize) -> bool {
        let Some(record_bytes) = native_evidence_bytes(&record) else {
            return false;
        };
        let Some(total) = self.retained_evidence_bytes.checked_add(record_bytes) else {
            return false;
        };
        let identity = registry_value_identity(&record.name_utf16);
        let group = self.by_identity.entry(identity).or_default();
        self.duplicate_exact_name |= group
            .iter()
            .any(|existing| existing.name_utf16 == record.name_utf16);
        group.push(record);
        self.retained_evidence_bytes = total;

        let mut omitted = false;
        while self.retained_evidence_bytes > evidence_budget {
            let Some(last_identity) = self.by_identity.keys().next_back().cloned() else {
                break;
            };
            let mut remove_group = false;
            if let Some(group) = self.by_identity.get_mut(&last_identity) {
                if let Some(removed) = group.pop() {
                    self.retained_evidence_bytes = self
                        .retained_evidence_bytes
                        .saturating_sub(native_evidence_bytes(&removed).unwrap_or(usize::MAX));
                    omitted = true;
                }
                remove_group = group.is_empty();
            }
            if remove_group {
                self.by_identity.remove(&last_identity);
            }
        }
        !omitted
    }

    fn into_records(self) -> Vec<RawRegistryValue> {
        self.by_identity
            .into_values()
            .flat_map(|records| records.into_iter())
            .collect()
    }

    fn has_duplicate_exact_name(&self) -> bool {
        self.duplicate_exact_name
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnumerationCallStatus {
    Success,
    MoreData,
    NoMoreItems,
    KeyDeleted(i64),
    Failed(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnumerationCallResult {
    status: EnumerationCallStatus,
    name_length: u32,
    value_type: u32,
    data_length: u32,
}

enum ProbeResult {
    NoMoreItems,
    Value {
        name: Vec<u16>,
        value_type: u32,
        required_data_bytes: u32,
        name_buffer: Vec<u16>,
    },
    Skipped(SourceIssue),
    Unstable(SourceIssue),
}

fn probe_value_with<F>(mut name_buffer: Vec<u16>, mut enumerate: F) -> ProbeResult
where
    F: FnMut(&mut [u16]) -> EnumerationCallResult,
{
    for _ in 0..MAX_BUFFER_GROWTH_ATTEMPTS {
        let result = enumerate(&mut name_buffer);
        match result.status {
            EnumerationCallStatus::Success => {
                let name_length = match usize::try_from(result.name_length) {
                    Ok(length) if length <= name_buffer.len() => length,
                    _ => return ProbeResult::Unstable(invalid_length_issue()),
                };
                return ProbeResult::Value {
                    name: name_buffer[..name_length].to_vec(),
                    value_type: result.value_type,
                    required_data_bytes: result.data_length,
                    name_buffer,
                };
            }
            EnumerationCallStatus::NoMoreItems => return ProbeResult::NoMoreItems,
            EnumerationCallStatus::MoreData => {
                let maximum_capacity = REGISTRY_VALUE_NAME_MAX_UTF16_UNITS + 1;
                if name_buffer.len() >= maximum_capacity {
                    return ProbeResult::Skipped(name_too_large_issue());
                }
                let grown = name_buffer
                    .len()
                    .saturating_mul(2)
                    .max(1)
                    .min(maximum_capacity);
                name_buffer.resize(grown, 0);
            }
            EnumerationCallStatus::KeyDeleted(code) => {
                return ProbeResult::Unstable(changed_issue(Some(code)));
            }
            EnumerationCallStatus::Failed(code) => {
                return ProbeResult::Unstable(enumeration_issue(code));
            }
        }
    }
    ProbeResult::Skipped(name_too_large_issue())
}

enum BufferedValueRead {
    Record(RawRegistryValue),
    Skipped(SourceIssue),
    Unstable(SourceIssue),
}

fn read_buffered_value_with<F>(
    expected_name: Vec<u16>,
    expected_type: u32,
    mut name_buffer: Vec<u16>,
    initial_data_bytes: usize,
    available_evidence_bytes: usize,
    mut enumerate: F,
) -> BufferedValueRead
where
    F: FnMut(&mut [u16], &mut [u8]) -> EnumerationCallResult,
{
    let mut data = vec![0_u8; initial_data_bytes];
    for _ in 0..MAX_BUFFER_GROWTH_ATTEMPTS {
        let result = enumerate(&mut name_buffer, &mut data);
        match result.status {
            EnumerationCallStatus::Success => {
                let name_length = match usize::try_from(result.name_length) {
                    Ok(length) if length <= name_buffer.len() => length,
                    _ => return BufferedValueRead::Unstable(invalid_length_issue()),
                };
                let data_length = match usize::try_from(result.data_length) {
                    Ok(length) if length <= data.len() => length,
                    _ => return BufferedValueRead::Unstable(invalid_length_issue()),
                };
                if name_buffer[..name_length] != expected_name || result.value_type != expected_type
                {
                    return BufferedValueRead::Unstable(changed_issue(None));
                }
                data.truncate(data_length);
                return BufferedValueRead::Record(RawRegistryValue {
                    name_utf16: expected_name,
                    value_type: expected_type,
                    data,
                });
            }
            EnumerationCallStatus::MoreData => {
                let required = match usize::try_from(result.data_length) {
                    Ok(required) => required,
                    Err(_) => return BufferedValueRead::Skipped(value_too_large_issue()),
                };
                if required > MAX_REGISTRY_VALUE_DATA_BYTES {
                    return BufferedValueRead::Skipped(value_too_large_issue());
                }
                let grown = required.max(data.len().saturating_mul(2)).max(1);
                if grown > MAX_REGISTRY_VALUE_DATA_BYTES
                    || !fits_evidence_budget(&expected_name, grown, available_evidence_bytes)
                {
                    return BufferedValueRead::Skipped(registry_resource_limit_issue());
                }
                data.resize(grown, 0);
                let maximum_name_capacity = REGISTRY_VALUE_NAME_MAX_UTF16_UNITS + 1;
                if name_buffer.len() < maximum_name_capacity {
                    let grown_name = name_buffer
                        .len()
                        .saturating_mul(2)
                        .max(1)
                        .min(maximum_name_capacity);
                    name_buffer.resize(grown_name, 0);
                }
            }
            EnumerationCallStatus::KeyDeleted(code) => {
                return BufferedValueRead::Unstable(changed_issue(Some(code)));
            }
            EnumerationCallStatus::Failed(code) => {
                return BufferedValueRead::Unstable(enumeration_issue(code));
            }
            EnumerationCallStatus::NoMoreItems => {
                return BufferedValueRead::Unstable(enumeration_issue(259));
            }
        }
    }
    BufferedValueRead::Skipped(SourceIssue {
        code: "registry_resource_limit",
        message: "A Registry value kept changing size during bounded retries.",
        stage: "enumerate",
        native_code: None,
    })
}

fn fits_evidence_budget(name: &[u16], data_bytes: usize, available_bytes: usize) -> bool {
    name.len()
        .checked_mul(size_of::<u16>())
        .and_then(|name_bytes| name_bytes.checked_add(data_bytes))
        .is_some_and(|required| required <= available_bytes)
}

fn scope_value_limit_exhausted(enumerated: u32, observed_count: u32) -> bool {
    enumerated == MAX_REGISTRY_VALUES_PER_SCOPE && observed_count > MAX_REGISTRY_VALUES_PER_SCOPE
}

fn enumeration_count_is_consistent(enumerated: u32, observed_count: u32) -> bool {
    enumerated == observed_count || scope_value_limit_exhausted(enumerated, observed_count)
}

fn value_too_large_issue() -> SourceIssue {
    SourceIssue {
        code: "registry_value_too_large",
        message: "A Registry value exceeded the SystemDiff per-value capture limit.",
        stage: "enumerate",
        native_code: None,
    }
}

fn name_too_large_issue() -> SourceIssue {
    SourceIssue {
        code: "registry_resource_limit",
        message: "A Registry value name exceeded the bounded Collector buffer.",
        stage: "enumerate",
        native_code: None,
    }
}

fn changed_issue(native_code: Option<i64>) -> SourceIssue {
    SourceIssue {
        code: "registry_changed_during_scan",
        message: "The Registry key changed during bounded collection retries.",
        stage: "enumerate",
        native_code,
    }
}

fn enumeration_issue(native_code: i64) -> SourceIssue {
    SourceIssue {
        code: "registry_enumeration_failed",
        message: "Registry value enumeration failed.",
        stage: "enumerate",
        native_code: Some(native_code),
    }
}

fn invalid_length_issue() -> SourceIssue {
    SourceIssue {
        code: "registry_enumeration_failed",
        message: "Registry enumeration returned an invalid buffer length.",
        stage: "enumerate",
        native_code: None,
    }
}

#[cfg(windows)]
mod platform {
    use super::{
        BoundedRecords, BufferedValueRead, EnumerationCallResult, EnumerationCallStatus,
        ProbeResult, changed_issue, enumeration_count_is_consistent, fits_evidence_budget,
        probe_value_with, read_buffered_value_with, scope_value_limit_exhausted,
        value_too_large_issue,
    };

    use crate::registry::{
        KeyMetadata, KeyReadAttempt, MAX_REGISTRY_VALUE_DATA_BYTES, MAX_REGISTRY_VALUES_PER_SCOPE,
        RawRegistryValue, ReadKeyResult, RegistryDataSource, RegistryLayout, RegistryRoot,
        RegistryTarget, SourceFailure, SourceFailureKind, SourceIssue, ViewSelector,
        registry_resource_limit_issue,
    };
    use systemdiff_core::REGISTRY_VALUE_NAME_MAX_UTF16_UNITS;
    use windows::Win32::Foundation::{
        ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_KEY_DELETED, ERROR_MORE_DATA,
        ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, FILETIME, GetLastError, WIN32_ERROR,
    };
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_WOW64_32KEY,
        KEY_WOW64_64KEY, REG_SAM_FLAGS, RegCloseKey, RegEnumValueW, RegOpenKeyExW,
        RegQueryInfoKeyW,
    };
    use windows::Win32::System::SystemInformation::{
        IMAGE_FILE_MACHINE_AMD64, IMAGE_FILE_MACHINE_ARM64, IMAGE_FILE_MACHINE_I386,
        IMAGE_FILE_MACHINE_UNKNOWN,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, IsWow64Process2};
    use windows::core::{PCWSTR, PWSTR};

    pub(crate) struct Win32RegistrySource;

    impl Win32RegistrySource {
        pub(crate) fn new() -> Self {
            Self
        }
    }

    impl RegistryDataSource for Win32RegistrySource {
        fn detect_layout(&mut self) -> Result<RegistryLayout, SourceFailure> {
            let mut process_machine = IMAGE_FILE_MACHINE_UNKNOWN;
            let mut native_machine = IMAGE_FILE_MACHINE_UNKNOWN;
            // SAFETY: GetCurrentProcess returns a process pseudo-handle valid for
            // this call, and both output pointers reference initialized local
            // IMAGE_FILE_MACHINE values for the duration of the call.
            if unsafe {
                IsWow64Process2(
                    GetCurrentProcess(),
                    &mut process_machine,
                    Some(&mut native_machine),
                )
            }
            .is_err()
            {
                // SAFETY: read immediately after the failed Win32 call above.
                let status = unsafe { GetLastError() };
                return Err(SourceFailure {
                    kind: SourceFailureKind::Other,
                    stage: "detect_layout",
                    native_code: i64::from(status.0),
                });
            }

            Ok(if native_machine == IMAGE_FILE_MACHINE_AMD64 {
                RegistryLayout::X64
            } else if native_machine == IMAGE_FILE_MACHINE_I386 {
                RegistryLayout::X86
            } else if native_machine == IMAGE_FILE_MACHINE_ARM64 {
                RegistryLayout::Arm64
            } else {
                RegistryLayout::Unknown(native_machine.0)
            })
        }

        fn read_key_once(
            &mut self,
            target: &RegistryTarget,
            available_evidence_bytes: usize,
        ) -> ReadKeyResult {
            read_key_once(target, available_evidence_bytes)
        }
    }

    struct OwnedRegistryKey(HKEY);

    impl OwnedRegistryKey {
        fn open(target: &RegistryTarget) -> Result<Option<Self>, SourceFailure> {
            let root = match target.root {
                RegistryRoot::CurrentUser => HKEY_CURRENT_USER,
                RegistryRoot::LocalMachine => HKEY_LOCAL_MACHINE,
            };
            let selector = match target.selector {
                ViewSelector::Shared | ViewSelector::Native => REG_SAM_FLAGS(0),
                ViewSelector::Registry32 => KEY_WOW64_32KEY,
                ViewSelector::Registry64 => KEY_WOW64_64KEY,
            };
            let path: Vec<u16> = target.key_path.encode_utf16().chain([0]).collect();
            let mut opened = HKEY::default();
            // SAFETY: root is a borrowed predefined handle, path is NUL-terminated
            // for the call, desired access is query-only, and opened is a valid
            // out pointer. Ownership is assumed only after ERROR_SUCCESS.
            let status = unsafe {
                RegOpenKeyExW(
                    root,
                    PCWSTR(path.as_ptr()),
                    None,
                    KEY_QUERY_VALUE | selector,
                    &mut opened,
                )
            };
            match status {
                ERROR_SUCCESS => Ok(Some(Self(opened))),
                ERROR_FILE_NOT_FOUND => Ok(None),
                ERROR_ACCESS_DENIED => Err(source_failure("open", status)),
                _ => Err(source_failure("open", status)),
            }
        }
    }

    impl Drop for OwnedRegistryKey {
        fn drop(&mut self) {
            // SAFETY: this wrapper owns exactly one handle returned by a
            // successful RegOpenKeyExW. Predefined root handles are never wrapped.
            let _ = unsafe { RegCloseKey(self.0) };
        }
    }

    fn read_key_once(target: &RegistryTarget, available_evidence_bytes: usize) -> ReadKeyResult {
        let key = match OwnedRegistryKey::open(target) {
            Ok(Some(key)) => key,
            Ok(None) => return ReadKeyResult::Missing,
            Err(failure) => return ReadKeyResult::Failed(failure),
        };
        let before = match query_metadata(key.0) {
            Ok(metadata) => metadata,
            Err(failure) => return ReadKeyResult::Failed(failure),
        };

        let mut records = BoundedRecords::default();
        let mut issues = Vec::new();
        let initial_name_capacity = usize::try_from(before.max_value_name_units)
            .ok()
            .and_then(|length| length.checked_add(1))
            .unwrap_or(1)
            .clamp(1, REGISTRY_VALUE_NAME_MAX_UTF16_UNITS + 1);
        let mut index = 0_u32;

        while index < MAX_REGISTRY_VALUES_PER_SCOPE {
            match read_value(
                key.0,
                index,
                initial_name_capacity,
                available_evidence_bytes,
            ) {
                ValueRead::NoMoreItems => break,
                ValueRead::Record(record) => {
                    if !records.insert(record, available_evidence_bytes) {
                        issues.push(registry_resource_limit_issue());
                    }
                    index += 1;
                }
                ValueRead::Skipped(issue) => {
                    issues.push(issue);
                    index += 1;
                }
                ValueRead::Unstable(issue) => {
                    issues.push(issue);
                    return ReadKeyResult::Present(KeyReadAttempt {
                        before: before.metadata,
                        after: before.metadata,
                        records: records.into_records(),
                        issues,
                        stable: false,
                    });
                }
            }
        }

        let after = match query_metadata(key.0) {
            Ok(metadata) => metadata,
            Err(failure) if failure.kind == SourceFailureKind::KeyDeleted => {
                issues.push(issue_from_failure(&failure));
                return ReadKeyResult::Present(KeyReadAttempt {
                    before: before.metadata,
                    after: before.metadata,
                    records: records.into_records(),
                    issues,
                    stable: false,
                });
            }
            Err(failure) => {
                issues.push(issue_from_failure(&failure));
                return ReadKeyResult::Present(KeyReadAttempt {
                    before: before.metadata,
                    after: before.metadata,
                    records: records.into_records(),
                    issues,
                    stable: false,
                });
            }
        };
        let value_limit_exhausted = scope_value_limit_exhausted(index, after.metadata.value_count);
        if value_limit_exhausted {
            issues.push(SourceIssue {
                code: "registry_resource_limit",
                message: "The Registry scope exceeded the SystemDiff value-count capture limit.",
                stage: "enumerate",
                native_code: None,
            });
        }
        let count_is_consistent =
            enumeration_count_is_consistent(index, after.metadata.value_count);
        if !count_is_consistent {
            issues.push(changed_issue(None));
        }
        if records.has_duplicate_exact_name() {
            issues.push(changed_issue(None));
        }
        let stable = before.metadata == after.metadata
            && count_is_consistent
            && !records.has_duplicate_exact_name();
        ReadKeyResult::Present(KeyReadAttempt {
            before: before.metadata,
            after: after.metadata,
            records: records.into_records(),
            issues,
            stable,
        })
    }

    #[derive(Clone, Copy)]
    struct QueryMetadata {
        metadata: KeyMetadata,
        max_value_name_units: u32,
    }

    fn query_metadata(key: HKEY) -> Result<QueryMetadata, SourceFailure> {
        let mut value_count = 0_u32;
        let mut max_value_name_units = 0_u32;
        let mut max_value_data_bytes = 0_u32;
        let mut last_write = FILETIME::default();
        // SAFETY: key is open for KEY_QUERY_VALUE. Every supplied output pointer
        // references initialized storage of the documented unit; unused outputs
        // are null through None.
        let status = unsafe {
            RegQueryInfoKeyW(
                key,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(&mut value_count),
                Some(&mut max_value_name_units),
                Some(&mut max_value_data_bytes),
                None,
                Some(&mut last_write),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(source_failure("query", status));
        }
        let _ = max_value_data_bytes;
        Ok(QueryMetadata {
            metadata: KeyMetadata {
                value_count,
                last_write: (u64::from(last_write.dwHighDateTime) << 32)
                    | u64::from(last_write.dwLowDateTime),
            },
            max_value_name_units,
        })
    }

    enum ValueRead {
        NoMoreItems,
        Record(RawRegistryValue),
        Skipped(SourceIssue),
        Unstable(SourceIssue),
    }

    fn read_value(
        key: HKEY,
        index: u32,
        initial_name_capacity: usize,
        available_evidence_bytes: usize,
    ) -> ValueRead {
        let (name, value_type, required_data_bytes, name_buffer) =
            match probe_value(key, index, vec![0_u16; initial_name_capacity]) {
                ProbeResult::NoMoreItems => return ValueRead::NoMoreItems,
                ProbeResult::Value {
                    name,
                    value_type,
                    required_data_bytes,
                    name_buffer,
                } => (name, value_type, required_data_bytes, name_buffer),
                ProbeResult::Skipped(issue) => return ValueRead::Skipped(issue),
                ProbeResult::Unstable(issue) => return ValueRead::Unstable(issue),
            };
        let required = match usize::try_from(required_data_bytes) {
            Ok(required) => required,
            Err(_) => return ValueRead::Skipped(value_too_large_issue()),
        };
        if required > MAX_REGISTRY_VALUE_DATA_BYTES {
            return ValueRead::Skipped(value_too_large_issue());
        }
        if !fits_evidence_budget(&name, required, available_evidence_bytes) {
            return ValueRead::Skipped(registry_resource_limit_issue());
        }
        if required == 0 {
            return ValueRead::Record(RawRegistryValue {
                name_utf16: name,
                value_type,
                data: Vec::new(),
            });
        }

        match read_buffered_value_with(
            name,
            value_type,
            name_buffer,
            required,
            available_evidence_bytes,
            |name_buffer, data_buffer| {
                let mut name_length = u32::try_from(name_buffer.len())
                    .expect("bounded Registry name buffer must fit u32");
                let mut data_length = u32::try_from(data_buffer.len())
                    .expect("bounded Registry data buffer must fit u32");
                let mut read_type = 0_u32;
                // SAFETY: both buffers are initialized and their capacities are
                // supplied in the units required by RegEnumValueW. The pure
                // state machine validates all returned lengths before slicing.
                let status = unsafe {
                    RegEnumValueW(
                        key,
                        index,
                        Some(PWSTR(name_buffer.as_mut_ptr())),
                        &mut name_length,
                        None,
                        Some(&mut read_type),
                        Some(data_buffer.as_mut_ptr()),
                        Some(&mut data_length),
                    )
                };
                EnumerationCallResult {
                    status: if status == ERROR_SUCCESS {
                        EnumerationCallStatus::Success
                    } else if status == ERROR_MORE_DATA {
                        EnumerationCallStatus::MoreData
                    } else if status == ERROR_KEY_DELETED {
                        EnumerationCallStatus::KeyDeleted(i64::from(status.0))
                    } else if status == ERROR_NO_MORE_ITEMS {
                        EnumerationCallStatus::NoMoreItems
                    } else {
                        EnumerationCallStatus::Failed(i64::from(status.0))
                    },
                    name_length,
                    value_type: read_type,
                    data_length,
                }
            },
        ) {
            BufferedValueRead::Record(record) => ValueRead::Record(record),
            BufferedValueRead::Skipped(issue) => ValueRead::Skipped(issue),
            BufferedValueRead::Unstable(issue) => ValueRead::Unstable(issue),
        }
    }

    fn probe_value(key: HKEY, index: u32, name_buffer: Vec<u16>) -> ProbeResult {
        probe_value_with(name_buffer, |name_buffer| {
            let mut name_length = u32::try_from(name_buffer.len())
                .expect("bounded Registry name buffer must fit u32");
            let mut read_type = 0_u32;
            let mut required_data_bytes = 0_u32;
            // SAFETY: the initialized name buffer and its UTF-16 capacity are
            // valid for the call. lpData is null intentionally; lpcbData receives
            // the required native byte count without copying value data. The
            // pure state machine validates the returned name length.
            let status = unsafe {
                RegEnumValueW(
                    key,
                    index,
                    Some(PWSTR(name_buffer.as_mut_ptr())),
                    &mut name_length,
                    None,
                    Some(&mut read_type),
                    None,
                    Some(&mut required_data_bytes),
                )
            };
            EnumerationCallResult {
                status: if status == ERROR_SUCCESS {
                    EnumerationCallStatus::Success
                } else if status == ERROR_NO_MORE_ITEMS {
                    EnumerationCallStatus::NoMoreItems
                } else if status == ERROR_MORE_DATA {
                    EnumerationCallStatus::MoreData
                } else if status == ERROR_KEY_DELETED {
                    EnumerationCallStatus::KeyDeleted(i64::from(status.0))
                } else {
                    EnumerationCallStatus::Failed(i64::from(status.0))
                },
                name_length,
                value_type: read_type,
                data_length: required_data_bytes,
            }
        })
    }

    fn source_failure(stage: &'static str, status: WIN32_ERROR) -> SourceFailure {
        SourceFailure {
            kind: if status == ERROR_ACCESS_DENIED {
                SourceFailureKind::AccessDenied
            } else if status == ERROR_KEY_DELETED {
                SourceFailureKind::KeyDeleted
            } else {
                SourceFailureKind::Other
            },
            stage,
            native_code: i64::from(status.0),
        }
    }

    fn issue_from_failure(failure: &SourceFailure) -> SourceIssue {
        if failure.kind == SourceFailureKind::KeyDeleted {
            changed_issue(Some(failure.native_code))
        } else {
            SourceIssue {
                code: "registry_query_failed",
                message: "Registry metadata could not be read after enumeration.",
                stage: failure.stage,
                native_code: Some(failure.native_code),
            }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use crate::registry::{
        ReadKeyResult, RegistryDataSource, RegistryLayout, RegistryTarget, SourceFailure,
        SourceFailureKind,
    };

    pub(crate) struct Win32RegistrySource;

    impl Win32RegistrySource {
        pub(crate) fn new() -> Self {
            Self
        }
    }

    impl RegistryDataSource for Win32RegistrySource {
        fn detect_layout(&mut self) -> Result<RegistryLayout, SourceFailure> {
            Err(SourceFailure {
                kind: SourceFailureKind::Other,
                stage: "detect_layout",
                native_code: 0,
            })
        }

        fn read_key_once(
            &mut self,
            _target: &RegistryTarget,
            _available_evidence_bytes: usize,
        ) -> ReadKeyResult {
            ReadKeyResult::Failed(SourceFailure {
                kind: SourceFailureKind::Other,
                stage: "open",
                native_code: 0,
            })
        }
    }
}

pub(crate) use platform::Win32RegistrySource;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_retries_name_more_data_and_preserves_byte_length_metadata() {
        let mut calls = 0;
        let result = probe_value_with(vec![0_u16; 1], |name_buffer| {
            calls += 1;
            if calls == 1 {
                return EnumerationCallResult {
                    status: EnumerationCallStatus::MoreData,
                    name_length: 0,
                    value_type: 0,
                    data_length: 0,
                };
            }
            name_buffer[..2].copy_from_slice(&[u16::from(b'A'), u16::from(b'B')]);
            EnumerationCallResult {
                status: EnumerationCallStatus::Success,
                name_length: 2,
                value_type: 1,
                data_length: 257,
            }
        });

        let ProbeResult::Value {
            name,
            value_type,
            required_data_bytes,
            name_buffer,
        } = result
        else {
            panic!("probe must succeed after bounded growth");
        };
        assert_eq!(calls, 2);
        assert_eq!(name, [u16::from(b'A'), u16::from(b'B')]);
        assert_eq!(value_type, 1);
        assert_eq!(required_data_bytes, 257);
        assert_eq!(name_buffer.len(), 2);
    }

    #[test]
    fn buffered_read_retries_same_value_after_data_more_data() {
        let expected_name: Vec<_> = "Name".encode_utf16().collect();
        let mut calls = 0;
        let result = read_buffered_value_with(
            expected_name.clone(),
            3,
            vec![0_u16; expected_name.len()],
            2,
            64,
            |name_buffer, data_buffer| {
                calls += 1;
                if calls == 1 {
                    return EnumerationCallResult {
                        status: EnumerationCallStatus::MoreData,
                        name_length: expected_name.len() as u32,
                        value_type: 3,
                        data_length: 6,
                    };
                }
                name_buffer[..expected_name.len()].copy_from_slice(&expected_name);
                data_buffer[..6].copy_from_slice(b"abcdef");
                EnumerationCallResult {
                    status: EnumerationCallStatus::Success,
                    name_length: expected_name.len() as u32,
                    value_type: 3,
                    data_length: 6,
                }
            },
        );

        let BufferedValueRead::Record(record) = result else {
            panic!("buffered read must succeed after growth");
        };
        assert_eq!(calls, 2);
        assert_eq!(record.name_utf16, expected_name);
        assert_eq!(record.data, b"abcdef");
    }

    #[test]
    fn buffered_read_stops_after_three_growth_attempts() {
        let mut calls = 0;
        let result = read_buffered_value_with(
            "A".encode_utf16().collect(),
            3,
            vec![0_u16; 1],
            1,
            128,
            |_name_buffer, data_buffer| {
                calls += 1;
                EnumerationCallResult {
                    status: EnumerationCallStatus::MoreData,
                    name_length: 1,
                    value_type: 3,
                    data_length: (data_buffer.len() + 1) as u32,
                }
            },
        );

        assert_eq!(calls, MAX_BUFFER_GROWTH_ATTEMPTS);
        assert!(matches!(
            result,
            BufferedValueRead::Skipped(SourceIssue {
                code: "registry_resource_limit",
                ..
            })
        ));
    }

    #[test]
    fn buffer_state_machine_rejects_invalid_returned_lengths_and_hard_caps() {
        let invalid = read_buffered_value_with(
            "A".encode_utf16().collect(),
            3,
            vec![0_u16; 1],
            1,
            usize::MAX,
            |_name_buffer, _data_buffer| EnumerationCallResult {
                status: EnumerationCallStatus::Success,
                name_length: 2,
                value_type: 3,
                data_length: 1,
            },
        );
        assert!(matches!(
            invalid,
            BufferedValueRead::Unstable(SourceIssue {
                code: "registry_enumeration_failed",
                ..
            })
        ));

        let oversized = read_buffered_value_with(
            "A".encode_utf16().collect(),
            3,
            vec![0_u16; 1],
            1,
            usize::MAX,
            |_name_buffer, _data_buffer| EnumerationCallResult {
                status: EnumerationCallStatus::MoreData,
                name_length: 1,
                value_type: 3,
                data_length: (MAX_REGISTRY_VALUE_DATA_BYTES + 1) as u32,
            },
        );
        assert!(matches!(
            oversized,
            BufferedValueRead::Skipped(SourceIssue {
                code: "registry_value_too_large",
                ..
            })
        ));
    }

    #[test]
    fn name_and_value_evidence_and_scope_count_limits_are_exact() {
        let name = [u16::from(b'A'); 4];
        assert!(fits_evidence_budget(&name, 0, 8));
        assert!(!fits_evidence_budget(&name, 0, 7));
        assert!(fits_evidence_budget(&name, 4, 12));
        assert!(!fits_evidence_budget(&name, 4, 11));

        assert!(!scope_value_limit_exhausted(
            MAX_REGISTRY_VALUES_PER_SCOPE,
            MAX_REGISTRY_VALUES_PER_SCOPE
        ));
        assert!(scope_value_limit_exhausted(
            MAX_REGISTRY_VALUES_PER_SCOPE,
            MAX_REGISTRY_VALUES_PER_SCOPE + 1
        ));

        assert!(enumeration_count_is_consistent(0, 0));
        assert!(enumeration_count_is_consistent(7, 7));
        assert!(!enumeration_count_is_consistent(6, 7));
        assert!(!enumeration_count_is_consistent(8, 7));
        assert!(enumeration_count_is_consistent(
            MAX_REGISTRY_VALUES_PER_SCOPE,
            MAX_REGISTRY_VALUES_PER_SCOPE + 1
        ));
    }

    #[test]
    fn bounded_record_selection_is_independent_of_enumeration_order() {
        fn record(name: &str) -> RawRegistryValue {
            RawRegistryValue {
                name_utf16: name.encode_utf16().collect(),
                value_type: 3,
                data: vec![0; 4],
            }
        }

        fn retained_names(input: [&str; 3]) -> Vec<Vec<u16>> {
            let mut records = BoundedRecords::default();
            for name in input {
                let _ = records.insert(record(name), 12);
            }
            records
                .into_records()
                .into_iter()
                .map(|record| record.name_utf16)
                .collect()
        }

        assert_eq!(
            retained_names(["A", "B", "C"]),
            retained_names(["C", "A", "B"])
        );
    }

    #[test]
    fn repeated_exact_name_is_a_concurrent_enumeration_anomaly() {
        let record = RawRegistryValue {
            name_utf16: "Repeated".encode_utf16().collect(),
            value_type: 3,
            data: vec![0; 4],
        };
        let mut records = BoundedRecords::default();
        assert!(records.insert(record.clone(), 1024));
        assert!(records.insert(record, 1024));
        assert!(records.has_duplicate_exact_name());
    }
}
