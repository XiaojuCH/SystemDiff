use crate::services::{
    MAX_SERVICES_PER_SCOPE, RawServiceConfig, RawServiceName, ServiceDataSource,
    ServiceEnumeration, ServiceFailure, ServiceFailureKind,
};
use std::collections::BTreeMap;
use std::mem::{align_of, size_of};
use std::ptr;
use systemdiff_core::windows_service_identity;
use windows::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_INSUFFICIENT_BUFFER, ERROR_MORE_DATA, ERROR_SERVICE_DOES_NOT_EXIST,
    ERROR_SERVICE_MARKED_FOR_DELETE, WIN32_ERROR,
};
use windows::Win32::System::Services::{
    CloseServiceHandle, ENUM_SERVICE_STATUS_PROCESSW, EnumServicesStatusExW, OpenSCManagerW,
    OpenServiceW, QUERY_SERVICE_CONFIGW, QueryServiceConfig2W, QueryServiceConfigW,
    SC_ENUM_PROCESS_INFO, SC_HANDLE, SC_MANAGER_ENUMERATE_SERVICE,
    SERVICE_CONFIG_DELAYED_AUTO_START_INFO, SERVICE_CONFIG_DESCRIPTION,
    SERVICE_DELAYED_AUTO_START_INFO, SERVICE_DESCRIPTIONW, SERVICE_QUERY_CONFIG, SERVICE_STATE_ALL,
    SERVICE_WIN32,
};
use windows::core::{Error as WindowsError, PCWSTR, PWSTR};

const MAX_ENUMERATION_BUFFER_BYTES: usize = 256 * 1024;
const MAX_QUERY_BUFFER_BYTES: usize = 8 * 1024;
const MAX_ENUMERATION_PAGES: usize = 64;
const MAX_QUERY_ATTEMPTS: usize = 3;

pub(crate) struct Win32ServiceSource {
    manager: Option<OwnedServiceHandle>,
}

impl Win32ServiceSource {
    pub(crate) fn new() -> Self {
        Self { manager: None }
    }

    fn manager(&mut self) -> Result<SC_HANDLE, ServiceFailure> {
        if self.manager.is_none() {
            // SAFETY: both null strings select the local machine and active
            // service database. The requested right permits enumeration only.
            let handle = unsafe {
                OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ENUMERATE_SERVICE)
            }
            .map_err(|error| map_error(error, "open_scm"))?;
            self.manager = Some(OwnedServiceHandle::new(handle, "open_scm")?);
        }
        Ok(self.manager.as_ref().expect("manager initialized").raw())
    }
}

impl ServiceDataSource for Win32ServiceSource {
    fn enumerate(&mut self) -> Result<ServiceEnumeration, ServiceFailure> {
        enumerate_services(self.manager()?)
    }

    fn read_config_once(
        &mut self,
        service_name_utf16: &[u16],
    ) -> Result<RawServiceConfig, ServiceFailure> {
        let manager = self.manager()?;
        let mut terminated = Vec::with_capacity(service_name_utf16.len().saturating_add(1));
        terminated.extend_from_slice(service_name_utf16);
        terminated.push(0);
        // SAFETY: the manager handle is owned by self, the name is
        // NUL-terminated for this call, and SERVICE_QUERY_CONFIG is read-only.
        let raw =
            unsafe { OpenServiceW(manager, PCWSTR(terminated.as_ptr()), SERVICE_QUERY_CONFIG) }
                .map_err(|error| map_error(error, "open_service"))?;
        let service = OwnedServiceHandle::new(raw, "open_service")?;

        let base = query_base_config(service.raw())?;
        let config = parse_base_config(&base)?;
        let description_buffer = query_config2(
            service.raw(),
            SERVICE_CONFIG_DESCRIPTION,
            "query_description",
        )?;
        let description = parse_description(&description_buffer)?;
        let delayed_buffer = query_config2(
            service.raw(),
            SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
            "query_delayed_auto_start",
        )?;
        let delayed_auto_start = parse_delayed_auto_start(&delayed_buffer)?;

        Ok(RawServiceConfig {
            service_name_utf16: service_name_utf16.to_vec(),
            display_name_utf16: config.display_name,
            service_type: config.service_type,
            start_type: config.start_type,
            error_control: config.error_control,
            binary_path_utf16: config.binary_path,
            account_utf16: config.account,
            dependencies_utf16: config.dependencies,
            load_order_group_utf16: config.load_order_group,
            tag_id: config.tag_id,
            delayed_auto_start,
            description_utf16: description,
        })
    }
}

struct OwnedServiceHandle(SC_HANDLE);

impl OwnedServiceHandle {
    fn new(handle: SC_HANDLE, stage: &'static str) -> Result<Self, ServiceFailure> {
        if handle.is_invalid() {
            Err(failure(
                ServiceFailureKind::Other,
                "service_invalid_handle",
                "Windows returned an invalid service handle.",
                stage,
                None,
            ))
        } else {
            Ok(Self(handle))
        }
    }

    fn raw(&self) -> SC_HANDLE {
        self.0
    }
}

impl Drop for OwnedServiceHandle {
    fn drop(&mut self) {
        // SAFETY: this wrapper is constructed only from a successful SCM or
        // service open and owns exactly one non-Copy wrapper lifetime.
        let _ = unsafe { CloseServiceHandle(self.0) };
    }
}

struct AlignedBuffer {
    words: Vec<usize>,
    byte_len: usize,
}

impl AlignedBuffer {
    fn new(byte_len: usize) -> Result<Self, ServiceFailure> {
        if byte_len == 0 {
            return Err(invalid_buffer(
                "Windows requested a zero-length service buffer.",
            ));
        }
        let words = byte_len
            .checked_add(size_of::<usize>() - 1)
            .and_then(|value| value.checked_div(size_of::<usize>()))
            .ok_or_else(resource_limit)?;
        Ok(Self {
            words: vec![usize::MAX; words],
            byte_len,
        })
    }

    fn as_mut_bytes(&mut self) -> &mut [u8] {
        // SAFETY: Vec<usize> is initialized and suitably aligned. The exposed
        // byte slice is restricted to byte_len, which is no larger than the
        // allocated word storage.
        unsafe {
            std::slice::from_raw_parts_mut(self.words.as_mut_ptr().cast::<u8>(), self.byte_len)
        }
    }

    fn as_ptr(&self) -> *const u8 {
        self.words.as_ptr().cast()
    }

    fn len(&self) -> usize {
        self.byte_len
    }
}

fn enumerate_services(manager: SC_HANDLE) -> Result<ServiceEnumeration, ServiceFailure> {
    let mut required = 0_u32;
    let mut returned = 0_u32;
    let mut probe_resume = 0_u32;
    // SAFETY: manager is a live owned SCM handle; null output requests the
    // required size and all output counters are valid for the call.
    let probe = unsafe {
        EnumServicesStatusExW(
            manager,
            SC_ENUM_PROCESS_INFO,
            SERVICE_WIN32,
            SERVICE_STATE_ALL,
            None,
            &mut required,
            &mut returned,
            Some(&mut probe_resume),
            PCWSTR::null(),
        )
    };
    if let Err(error) = probe {
        if error_code(&error) != Some(ERROR_MORE_DATA) {
            return Err(map_error(error, "enumerate_probe"));
        }
    } else if returned == 0 {
        return Ok(ServiceEnumeration {
            names: Vec::new(),
            issues: Vec::new(),
        });
    }

    let mut requested = checked_buffer_size(required, size_of::<ENUM_SERVICE_STATUS_PROCESSW>())?;
    let mut resume = 0_u32;
    let mut retained: BTreeMap<String, Vec<Vec<u16>>> = BTreeMap::new();
    let mut retained_count = 0_usize;
    let mut omitted_for_limit = false;

    for _ in 0..MAX_ENUMERATION_PAGES {
        let mut buffer = AlignedBuffer::new(requested)?;
        required = 0;
        returned = 0;
        let before_resume = resume;
        // SAFETY: buffer is initialized and aligned, counters are valid, and
        // manager remains live for the call. Returned pointers are copied only
        // after range and termination checks below.
        let call = unsafe {
            EnumServicesStatusExW(
                manager,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                Some(buffer.as_mut_bytes()),
                &mut required,
                &mut returned,
                Some(&mut resume),
                PCWSTR::null(),
            )
        };
        let more_data = match call {
            Ok(()) => false,
            Err(error) if error_code(&error) == Some(ERROR_MORE_DATA) => true,
            Err(error) => return Err(map_error(error, "enumerate")),
        };

        for name in parse_enumerated_names(&buffer, returned)? {
            let identity = windows_service_identity(&name);
            retained.entry(identity).or_default().push(name);
            retained_count = retained_count.checked_add(1).ok_or_else(resource_limit)?;
            while retained_count > MAX_SERVICES_PER_SCOPE {
                let last = retained
                    .keys()
                    .next_back()
                    .cloned()
                    .ok_or_else(resource_limit)?;
                let mut remove = false;
                if let Some(group) = retained.get_mut(&last) {
                    group.pop();
                    retained_count -= 1;
                    remove = group.is_empty();
                }
                if remove {
                    retained.remove(&last);
                }
                omitted_for_limit = true;
            }
        }

        if !more_data {
            let names = retained
                .into_values()
                .flat_map(|group| group.into_iter())
                .map(|name_utf16| RawServiceName { name_utf16 })
                .collect();
            let issues = omitted_for_limit
                .then(resource_limit)
                .into_iter()
                .collect::<Vec<_>>();
            return Ok(ServiceEnumeration { names, issues });
        }
        if resume == before_resume {
            return Err(failure(
                ServiceFailureKind::Other,
                "service_enumeration_stalled",
                "Service enumeration made no progress within its bounded pagination loop.",
                "enumerate",
                None,
            ));
        }
        if required != 0 {
            requested = checked_buffer_size(required, size_of::<ENUM_SERVICE_STATUS_PROCESSW>())?;
        }
    }

    Err(failure(
        ServiceFailureKind::ResourceLimit,
        "service_resource_limit",
        "Service enumeration exceeded the SystemDiff page limit.",
        "enumerate",
        None,
    ))
}

fn checked_buffer_size(required: u32, minimum: usize) -> Result<usize, ServiceFailure> {
    let required = usize::try_from(required).map_err(|_| resource_limit())?;
    Ok(required.max(minimum).min(MAX_ENUMERATION_BUFFER_BYTES))
}

fn parse_enumerated_names(
    buffer: &AlignedBuffer,
    returned: u32,
) -> Result<Vec<Vec<u16>>, ServiceFailure> {
    let count = usize::try_from(returned).map_err(|_| invalid_buffer("Invalid service count."))?;
    let byte_count = count
        .checked_mul(size_of::<ENUM_SERVICE_STATUS_PROCESSW>())
        .ok_or_else(|| invalid_buffer("Invalid service array length."))?;
    if byte_count > buffer.len()
        || !(buffer.as_ptr() as usize).is_multiple_of(align_of::<ENUM_SERVICE_STATUS_PROCESSW>())
    {
        return Err(invalid_buffer(
            "Service enumeration returned an invalid array length.",
        ));
    }
    let mut names = Vec::with_capacity(count);
    for index in 0..count {
        // SAFETY: the checked array byte range contains this element. read
        // avoids retaining references into the native buffer.
        let record = unsafe {
            ptr::read(
                buffer
                    .as_ptr()
                    .cast::<ENUM_SERVICE_STATUS_PROCESSW>()
                    .add(index),
            )
        };
        let name = copy_wide(record.lpServiceName, buffer, false)?;
        names.push(name);
    }
    Ok(names)
}

#[derive(Debug)]
struct ParsedBaseConfig {
    service_type: u32,
    start_type: u32,
    error_control: u32,
    binary_path: Vec<u16>,
    load_order_group: Option<Vec<u16>>,
    tag_id: Option<u32>,
    dependencies: Vec<Vec<u16>>,
    account: Option<Vec<u16>>,
    display_name: Option<Vec<u16>>,
}

fn query_base_config(service: SC_HANDLE) -> Result<AlignedBuffer, ServiceFailure> {
    let mut required = 0_u32;
    // SAFETY: a null output pointer requests the documented buffer size.
    let probe = unsafe { QueryServiceConfigW(service, None, 0, &mut required) };
    if let Err(error) = probe
        && error_code(&error) != Some(ERROR_INSUFFICIENT_BUFFER)
    {
        return Err(map_error(error, "query_config"));
    }

    for _ in 0..MAX_QUERY_ATTEMPTS {
        let requested = checked_query_size(required, size_of::<QUERY_SERVICE_CONFIGW>())?;
        let mut buffer = AlignedBuffer::new(requested)?;
        required = 0;
        // SAFETY: the buffer is aligned for QUERY_SERVICE_CONFIGW, initialized,
        // and its exact size is supplied. service remains live.
        let result = unsafe {
            QueryServiceConfigW(
                service,
                Some(buffer.as_mut_bytes().as_mut_ptr().cast()),
                u32::try_from(requested).map_err(|_| resource_limit())?,
                &mut required,
            )
        };
        match result {
            Ok(()) => return Ok(buffer),
            Err(error) if error_code(&error) == Some(ERROR_INSUFFICIENT_BUFFER) => continue,
            Err(error) => return Err(map_error(error, "query_config")),
        }
    }
    Err(changed_failure("query_config"))
}

fn query_config2(
    service: SC_HANDLE,
    level: windows::Win32::System::Services::SERVICE_CONFIG,
    stage: &'static str,
) -> Result<AlignedBuffer, ServiceFailure> {
    let mut required = 0_u32;
    // SAFETY: a null output slice requests the documented buffer size.
    let probe = unsafe { QueryServiceConfig2W(service, level, None, &mut required) };
    if let Err(error) = probe
        && error_code(&error) != Some(ERROR_INSUFFICIENT_BUFFER)
    {
        return Err(map_error(error, stage));
    }
    for _ in 0..MAX_QUERY_ATTEMPTS {
        let requested = checked_query_size(required, 1)?;
        let mut buffer = AlignedBuffer::new(requested)?;
        required = 0;
        // SAFETY: the initialized buffer is live and bounded for this query.
        let result = unsafe {
            QueryServiceConfig2W(service, level, Some(buffer.as_mut_bytes()), &mut required)
        };
        match result {
            Ok(()) => return Ok(buffer),
            Err(error) if error_code(&error) == Some(ERROR_INSUFFICIENT_BUFFER) => continue,
            Err(error) => return Err(map_error(error, stage)),
        }
    }
    Err(changed_failure(stage))
}

fn checked_query_size(required: u32, minimum: usize) -> Result<usize, ServiceFailure> {
    let requested = usize::try_from(required)
        .map_err(|_| resource_limit())?
        .max(minimum);
    if requested > MAX_QUERY_BUFFER_BYTES {
        Err(resource_limit())
    } else {
        Ok(requested)
    }
}

fn parse_base_config(buffer: &AlignedBuffer) -> Result<ParsedBaseConfig, ServiceFailure> {
    if buffer.len() < size_of::<QUERY_SERVICE_CONFIGW>()
        || !(buffer.as_ptr() as usize).is_multiple_of(align_of::<QUERY_SERVICE_CONFIGW>())
    {
        return Err(invalid_buffer(
            "Service configuration returned an invalid structure.",
        ));
    }
    // SAFETY: size and alignment were checked and the structure is copied.
    let config = unsafe { ptr::read(buffer.as_ptr().cast::<QUERY_SERVICE_CONFIGW>()) };
    Ok(ParsedBaseConfig {
        service_type: config.dwServiceType.0,
        start_type: config.dwStartType.0,
        error_control: config.dwErrorControl.0,
        binary_path: copy_wide(config.lpBinaryPathName, buffer, true)?,
        load_order_group: copy_optional_wide(config.lpLoadOrderGroup, buffer)?,
        tag_id: (config.dwTagId != 0).then_some(config.dwTagId),
        dependencies: copy_multi_sz(config.lpDependencies, buffer)?,
        account: copy_optional_wide(config.lpServiceStartName, buffer)?,
        display_name: copy_optional_wide(config.lpDisplayName, buffer)?,
    })
}

fn parse_description(buffer: &AlignedBuffer) -> Result<Option<Vec<u16>>, ServiceFailure> {
    if buffer.len() < size_of::<SERVICE_DESCRIPTIONW>()
        || !(buffer.as_ptr() as usize).is_multiple_of(align_of::<SERVICE_DESCRIPTIONW>())
    {
        return Err(invalid_buffer(
            "Service description returned an invalid structure.",
        ));
    }
    // SAFETY: size and alignment were checked and the structure is copied.
    let description = unsafe { ptr::read(buffer.as_ptr().cast::<SERVICE_DESCRIPTIONW>()) };
    copy_optional_wide(description.lpDescription, buffer)
}

fn parse_delayed_auto_start(buffer: &AlignedBuffer) -> Result<bool, ServiceFailure> {
    if buffer.len() < size_of::<SERVICE_DELAYED_AUTO_START_INFO>()
        || !(buffer.as_ptr() as usize).is_multiple_of(align_of::<SERVICE_DELAYED_AUTO_START_INFO>())
    {
        return Err(invalid_buffer(
            "Delayed auto-start returned an invalid structure.",
        ));
    }
    // SAFETY: size and alignment were checked and the structure is copied.
    let delayed = unsafe { ptr::read(buffer.as_ptr().cast::<SERVICE_DELAYED_AUTO_START_INFO>()) };
    Ok(delayed.fDelayedAutostart.as_bool())
}

fn copy_optional_wide(
    pointer: PWSTR,
    buffer: &AlignedBuffer,
) -> Result<Option<Vec<u16>>, ServiceFailure> {
    if pointer.is_null() {
        return Ok(None);
    }
    let value = copy_wide(pointer, buffer, true)?;
    Ok((!value.is_empty()).then_some(value))
}

fn copy_wide(
    pointer: PWSTR,
    buffer: &AlignedBuffer,
    allow_empty: bool,
) -> Result<Vec<u16>, ServiceFailure> {
    let start = buffer.as_ptr() as usize;
    let end = start
        .checked_add(buffer.len())
        .ok_or_else(|| invalid_buffer("Service buffer address overflowed."))?;
    let address = pointer.0 as usize;
    if pointer.is_null()
        || address < start
        || address >= end
        || !(address - start).is_multiple_of(size_of::<u16>())
    {
        return Err(invalid_buffer(
            "Service data contained an out-of-range string pointer.",
        ));
    }
    let available = (end - address) / size_of::<u16>();
    let mut value = Vec::new();
    for index in 0..available {
        // SAFETY: index is within the validated buffer range and read_unaligned
        // handles any u16 alignment stricter than the byte offset check.
        let unit = unsafe { ptr::read_unaligned(pointer.0.add(index)) };
        if unit == 0 {
            if value.is_empty() && !allow_empty {
                return Err(invalid_buffer(
                    "Service data contained an empty required string.",
                ));
            }
            return Ok(value);
        }
        value.push(unit);
    }
    Err(invalid_buffer(
        "Service data contained an unterminated UTF-16 string.",
    ))
}

fn copy_multi_sz(pointer: PWSTR, buffer: &AlignedBuffer) -> Result<Vec<Vec<u16>>, ServiceFailure> {
    if pointer.is_null() {
        return Ok(Vec::new());
    }
    let start = buffer.as_ptr() as usize;
    let end = start
        .checked_add(buffer.len())
        .ok_or_else(|| invalid_buffer("Service buffer address overflowed."))?;
    let address = pointer.0 as usize;
    if address < start || address >= end || !(address - start).is_multiple_of(size_of::<u16>()) {
        return Err(invalid_buffer(
            "Service data contained an out-of-range dependency pointer.",
        ));
    }
    let available = (end - address) / size_of::<u16>();
    let mut encoded = Vec::new();
    let mut previous_zero = false;
    for index in 0..available {
        // SAFETY: index is inside the validated native output buffer.
        let unit = unsafe { ptr::read_unaligned(pointer.0.add(index)) };
        encoded.push(unit);
        if unit == 0 {
            if index == 0 || previous_zero {
                return parse_multi_sz_units(&encoded);
            }
            previous_zero = true;
        } else {
            previous_zero = false;
        }
    }
    Err(invalid_buffer(
        "Service dependencies were not double-NUL terminated.",
    ))
}

fn parse_multi_sz_units(encoded: &[u16]) -> Result<Vec<Vec<u16>>, ServiceFailure> {
    if encoded == [0] {
        return Ok(Vec::new());
    }
    if encoded.len() < 2 || !encoded.ends_with(&[0, 0]) {
        return Err(invalid_buffer(
            "Service dependencies were not double-NUL terminated.",
        ));
    }
    let mut dependencies = Vec::new();
    let mut start = 0_usize;
    for index in 0..encoded.len() - 1 {
        if encoded[index] == 0 {
            if index == start {
                return Err(invalid_buffer(
                    "Service dependencies contained an empty item.",
                ));
            }
            dependencies.push(encoded[start..index].to_vec());
            start = index + 1;
            if encoded[index + 1] == 0 {
                break;
            }
        }
    }
    Ok(dependencies)
}

fn error_code(error: &WindowsError) -> Option<WIN32_ERROR> {
    WIN32_ERROR::from_error(error)
}

fn map_error(error: WindowsError, stage: &'static str) -> ServiceFailure {
    let code = error_code(&error);
    let native_code = code.map(|value| i64::from(value.0));
    if code == Some(ERROR_ACCESS_DENIED) {
        failure(
            ServiceFailureKind::AccessDenied,
            "service_access_denied",
            "Service configuration access was denied.",
            stage,
            native_code,
        )
    } else if matches!(
        code,
        Some(ERROR_SERVICE_DOES_NOT_EXIST | ERROR_SERVICE_MARKED_FOR_DELETE)
    ) {
        failure(
            ServiceFailureKind::DoesNotExist,
            "service_vanished_during_scan",
            "A service vanished during collection.",
            stage,
            native_code,
        )
    } else {
        failure(
            ServiceFailureKind::Other,
            "service_query_failed",
            "A Windows service API query failed.",
            stage,
            native_code,
        )
    }
}

fn invalid_buffer(message: &'static str) -> ServiceFailure {
    failure(
        ServiceFailureKind::InvalidData,
        "service_invalid_data",
        message,
        "native_buffer",
        None,
    )
}

fn resource_limit() -> ServiceFailure {
    failure(
        ServiceFailureKind::ResourceLimit,
        "service_resource_limit",
        "Service data exceeded a SystemDiff capture budget.",
        "native_buffer",
        None,
    )
}

fn changed_failure(stage: &'static str) -> ServiceFailure {
    failure(
        ServiceFailureKind::Other,
        "service_changed_during_scan",
        "A service changed during bounded configuration reads.",
        stage,
        None,
    )
}

fn failure(
    kind: ServiceFailureKind,
    code: &'static str,
    message: &'static str,
    stage: &'static str,
    native_code: Option<i64>,
) -> ServiceFailure {
    ServiceFailure {
        kind,
        code,
        message,
        stage,
        native_code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_parser_preserves_order_group_prefix_and_empty_list() {
        assert_eq!(parse_multi_sz_units(&[0]).unwrap(), Vec::<Vec<u16>>::new());
        let encoded: Vec<u16> = "RpcSs\0+NetworkProvider\0\0".encode_utf16().collect();
        assert_eq!(
            parse_multi_sz_units(&encoded).unwrap(),
            vec![
                "RpcSs".encode_utf16().collect::<Vec<_>>(),
                "+NetworkProvider".encode_utf16().collect::<Vec<_>>()
            ]
        );
    }

    #[test]
    fn dependency_parser_rejects_missing_terminator() {
        assert!(parse_multi_sz_units(&[b'A' as u16, 0]).is_err());
    }

    #[test]
    fn aligned_buffer_rejects_out_of_range_string_pointer() {
        let buffer = AlignedBuffer::new(32).unwrap();
        let outside = PWSTR(buffer.as_ptr().wrapping_add(buffer.len()).cast_mut().cast());
        assert!(copy_wide(outside, &buffer, true).is_err());
    }
}
