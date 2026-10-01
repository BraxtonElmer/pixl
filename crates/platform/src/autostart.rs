//! "Start with Windows": a value under the current user's Run key. No admin
//! rights, no scheduled task, and Task Manager's Startup tab can turn it off.

use std::path::Path;
use std::ptr::null_mut;

use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ, RegCloseKey, RegDeleteValueW,
    RegGetValueW, RegOpenKeyExW, RegSetValueExW,
};

use crate::wide::{from_wide, to_wide};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "Pixl";

/// True when Windows will start Pixl at sign-in.
pub fn is_enabled() -> bool {
    current().is_some()
}

fn current() -> Option<String> {
    let key = to_wide(RUN_KEY);
    let name = to_wide(VALUE);
    let mut buf = [0u16; 1024];
    let mut len = size_of_val(&buf) as u32;
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buf.as_mut_ptr().cast(),
            &mut len,
        )
    };
    (rc == 0).then(|| from_wide(&buf))
}

/// Point the Run entry at `tray_exe` (quiet start) or remove it.
pub fn set(enabled: bool, tray_exe: &Path) -> bool {
    let key = to_wide(RUN_KEY);
    let mut hkey: HKEY = null_mut();
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, KEY_SET_VALUE | KEY_READ, &mut hkey) } != 0 {
        return false;
    }
    let name = to_wide(VALUE);
    let rc = if enabled {
        let cmd = to_wide(&format!("\"{}\" --background", tray_exe.display()));
        unsafe { RegSetValueExW(hkey, name.as_ptr(), 0, REG_SZ, cmd.as_ptr().cast(), (cmd.len() * 2) as u32) }
    } else {
        match unsafe { RegDeleteValueW(hkey, name.as_ptr()) } {
            2 => 0, // ERROR_FILE_NOT_FOUND: already off
            rc => rc,
        }
    };
    unsafe { RegCloseKey(hkey) };
    rc == 0
}
