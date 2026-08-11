use systemdiff_core::{HostMetadata, PrivilegeState};

#[cfg(any(windows, test))]
fn classify_privilege_state(
    is_local_system: Option<bool>,
    is_elevated: Option<bool>,
) -> PrivilegeState {
    match is_local_system {
        Some(true) => PrivilegeState::System,
        Some(false) => match is_elevated {
            Some(true) => PrivilegeState::Elevated,
            Some(false) => PrivilegeState::StandardUser,
            None => PrivilegeState::Unknown,
        },
        None => PrivilegeState::Unknown,
    }
}

#[cfg(windows)]
pub(crate) fn is_supported() -> bool {
    windows_version::OsVersion::current() >= windows_version::OsVersion::new(10, 0, 0, 16_299)
}

#[cfg(not(windows))]
pub(crate) fn is_supported() -> bool {
    false
}

#[cfg(windows)]
pub(crate) fn host_metadata() -> HostMetadata {
    use windows::Win32::System::SystemInformation::{
        IMAGE_FILE_MACHINE_AMD64, IMAGE_FILE_MACHINE_ARM64, IMAGE_FILE_MACHINE_I386,
        IMAGE_FILE_MACHINE_UNKNOWN,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, IsWow64Process2};

    let version = windows_version::OsVersion::current();
    let product = if windows_version::is_server() {
        "Windows Server"
    } else {
        "Windows"
    };
    let mut process_machine = IMAGE_FILE_MACHINE_UNKNOWN;
    let mut native_machine = IMAGE_FILE_MACHINE_UNKNOWN;
    // SAFETY: the pseudo-handle is valid for the current process and both
    // output pointers refer to initialized local storage for the call.
    let architecture = unsafe {
        IsWow64Process2(
            GetCurrentProcess(),
            &mut process_machine,
            Some(&mut native_machine),
        )
    }
    .ok()
    .and_then(|()| {
        if native_machine == IMAGE_FILE_MACHINE_AMD64 {
            Some("x86_64")
        } else if native_machine == IMAGE_FILE_MACHINE_I386 {
            Some("x86")
        } else if native_machine == IMAGE_FILE_MACHINE_ARM64 {
            Some("arm64")
        } else {
            None
        }
    })
    .map(str::to_owned);

    HostMetadata {
        windows_version: Some(format!("{product} {}.{}", version.major, version.minor)),
        windows_build: Some(version.build.to_string()),
        architecture,
    }
}

#[cfg(not(windows))]
pub(crate) fn host_metadata() -> HostMetadata {
    HostMetadata {
        windows_version: None,
        windows_build: None,
        architecture: None,
    }
}

#[cfg(windows)]
pub(crate) fn privilege_state() -> PrivilegeState {
    use std::mem::size_of;
    use windows::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, HANDLE, WIN32_ERROR};
    use windows::Win32::Security::{
        GetTokenInformation, IsValidSid, IsWellKnownSid, TOKEN_ELEVATION, TOKEN_QUERY, TOKEN_USER,
        TokenElevation, TokenUser, WinLocalSystemSid,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    struct OwnedToken(HANDLE);
    impl Drop for OwnedToken {
        fn drop(&mut self) {
            // SAFETY: the handle is owned after a successful OpenProcessToken.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    let mut token = HANDLE::default();
    // SAFETY: token is a valid out pointer and the current-process pseudo-handle
    // is valid. The requested access is query-only.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.is_err() {
        return PrivilegeState::Unknown;
    }
    if token.is_invalid() {
        return PrivilegeState::Unknown;
    }
    let token = OwnedToken(token);
    let is_local_system = token_is_local_system(token.0);
    if is_local_system != Some(false) {
        return classify_privilege_state(is_local_system, None);
    }
    return classify_privilege_state(is_local_system, token_is_elevated(token.0));

    fn token_is_local_system(token: HANDLE) -> Option<bool> {
        const MAX_TOKEN_USER_BYTES: usize = 64 * 1024;

        let mut required = 0_u32;
        // SAFETY: the token is open for TOKEN_QUERY, the null buffer is the
        // documented size probe, and required is a valid output pointer.
        let probe = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut required) };
        match probe {
            Err(error) if WIN32_ERROR::from_error(&error) == Some(ERROR_INSUFFICIENT_BUFFER) => {}
            _ => return None,
        }

        let required = usize::try_from(required).ok()?;
        if required < size_of::<TOKEN_USER>() || required > MAX_TOKEN_USER_BYTES {
            return None;
        }
        let words = required
            .checked_add(size_of::<usize>() - 1)?
            .checked_div(size_of::<usize>())?;
        let mut buffer = vec![0_usize; words];
        let required_u32 = u32::try_from(required).ok()?;
        let mut returned = 0_u32;
        // SAFETY: Vec<usize> provides alignment suitable for TOKEN_USER, its
        // initialized allocation is at least required bytes, and the token is
        // queryable. The returned length is validated before the typed read.
        if unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                Some(buffer.as_mut_ptr().cast()),
                required_u32,
                &mut returned,
            )
        }
        .is_err()
        {
            return None;
        }
        let returned = usize::try_from(returned).ok()?;
        if returned < size_of::<TOKEN_USER>() || returned > required {
            return None;
        }

        // SAFETY: the buffer has TOKEN_USER alignment and the successful API
        // call returned at least size_of::<TOKEN_USER>() initialized bytes.
        let token_user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
        let sid = token_user.User.Sid;
        if sid.is_invalid() {
            return None;
        }
        // SAFETY: SID is borrowed from the still-live TokenUser buffer returned
        // by Windows. IsValidSid checks its documented structure before the
        // well-known SID comparison.
        if !unsafe { IsValidSid(sid) }.as_bool() {
            return None;
        }
        // SAFETY: sid remains valid for the call and passed IsValidSid above.
        Some(unsafe { IsWellKnownSid(sid, WinLocalSystemSid) }.as_bool())
    }

    fn token_is_elevated(token: HANDLE) -> Option<bool> {
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0_u32;
        let length = u32::try_from(size_of::<TOKEN_ELEVATION>()).ok()?;
        // SAFETY: token is queryable, the output pointer references a correctly
        // sized TOKEN_ELEVATION, and returned receives the documented byte count.
        if unsafe {
            GetTokenInformation(
                token,
                TokenElevation,
                Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
                length,
                &mut returned,
            )
        }
        .is_err()
            || returned < length
        {
            return None;
        }
        Some(elevation.TokenIsElevated != 0)
    }
}

#[cfg(not(windows))]
pub(crate) fn privilege_state() -> PrivilegeState {
    PrivilegeState::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privilege_classification_prioritizes_local_system_and_propagates_unknowns() {
        assert_eq!(
            classify_privilege_state(Some(true), Some(false)),
            PrivilegeState::System
        );
        assert_eq!(
            classify_privilege_state(Some(false), Some(true)),
            PrivilegeState::Elevated
        );
        assert_eq!(
            classify_privilege_state(Some(false), Some(false)),
            PrivilegeState::StandardUser
        );
        assert_eq!(
            classify_privilege_state(None, Some(true)),
            PrivilegeState::Unknown
        );
        assert_eq!(
            classify_privilege_state(Some(false), None),
            PrivilegeState::Unknown
        );
    }
}
