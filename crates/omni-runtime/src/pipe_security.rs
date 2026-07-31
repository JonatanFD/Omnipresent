//! An owner-only security descriptor for the daemon's Windows named pipe.
//!
//! On Unix the IPC socket is created with mode `0600`, so only the user running
//! the daemon can command it. A named pipe created with no security descriptor
//! gets the system default instead, which is more generous than that — and the
//! promise the project makes is that only the owner can drive the daemon.
//!
//! This builds the Windows equivalent of `0600`: a protected DACL (`P`, so it
//! inherits nothing) with exactly one entry, granting full access to the SID of
//! the user the daemon runs as.

use std::io;
use std::os::raw::c_void;
use windows_sys::Win32::Foundation::{HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// A security descriptor granting the current user, and nobody else, full access.
///
/// Holds the descriptor alive for as long as it is needed; the pipe only borrows
/// it while it is being created.
pub struct OwnerOnly {
    descriptor: PSECURITY_DESCRIPTOR,
    attributes: SECURITY_ATTRIBUTES,
}

impl OwnerOnly {
    /// Builds the descriptor for the user this process runs as.
    pub fn new() -> io::Result<Self> {
        let sid = current_user_sid_string()?;
        // "D:P(A;;GA;;;<sid>)" — a protected DACL (inherits nothing) with one
        // allow-all entry for this user.
        let sddl = format!("D:P(A;;GA;;;{sid})");
        let descriptor = security_descriptor_from_sddl(&sddl)?;

        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        Ok(Self {
            descriptor,
            attributes,
        })
    }

    /// The `SECURITY_ATTRIBUTES` to hand to pipe creation.
    ///
    /// Only valid while `self` is alive.
    pub fn as_ptr(&mut self) -> *mut c_void {
        &mut self.attributes as *mut SECURITY_ATTRIBUTES as *mut c_void
    }
}

// Safety: the descriptor is an ordinary heap allocation with no thread affinity,
// and `OwnerOnly` owns it exclusively — moving that ownership to another thread
// is fine. It is deliberately not `Sync`: nothing needs to share one.
unsafe impl Send for OwnerOnly {}

impl Drop for OwnerOnly {
    fn drop(&mut self) {
        if !self.descriptor.is_null() {
            // Allocated by ConvertStringSecurityDescriptorToSecurityDescriptorW.
            unsafe { LocalFree(self.descriptor as *mut c_void) };
        }
    }
}

impl std::fmt::Debug for OwnerOnly {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnerOnly").finish_non_exhaustive()
    }
}

/// The current user's SID, in the string form SDDL expects.
fn current_user_sid_string() -> io::Result<String> {
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = OwnedToken(token);

        // Ask for the size first, then the value: the SID is variable length.
        let mut needed = 0u32;
        GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut needed);
        if needed == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0u8; needed as usize];
        if GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr() as *mut c_void,
            needed,
            &mut needed,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }

        let user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut raw: *mut u16 = std::ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut raw) == 0 {
            return Err(io::Error::last_os_error());
        }
        let sid = wide_to_string(raw);
        LocalFree(raw as *mut c_void);
        Ok(sid)
    }
}

/// Turns an SDDL string into a security descriptor the OS allocated.
fn security_descriptor_from_sddl(sddl: &str) -> io::Result<PSECURITY_DESCRIPTOR> {
    let wide: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(descriptor)
}

/// Copies a NUL-terminated wide string into a `String`.
unsafe fn wide_to_string(raw: *const u16) -> String {
    unsafe {
        let mut len = 0;
        while *raw.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(raw, len))
    }
}

/// Closes a token handle on drop, so an early return cannot leak it.
struct OwnedToken(HANDLE);

impl Drop for OwnedToken {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_user_has_a_sid_we_can_name() {
        let sid = current_user_sid_string().expect("the process always has a user");
        // SDDL SIDs are "S-1-..." or a two-letter well-known alias.
        assert!(
            sid.starts_with("S-") || sid.len() == 2,
            "unexpected SID form: {sid}"
        );
    }

    #[test]
    fn an_owner_only_descriptor_is_built() {
        let mut security = OwnerOnly::new().expect("descriptor");
        assert!(!security.descriptor.is_null());
        assert!(!security.as_ptr().is_null());
    }

    #[test]
    fn a_malformed_sddl_is_an_error_not_a_panic() {
        assert!(security_descriptor_from_sddl("not a descriptor").is_err());
    }
}
