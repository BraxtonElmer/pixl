//! Small Windows helpers for the settings window.

use std::path::PathBuf;
use std::ptr::{null, null_mut};

use pixl_platform::tray;
use pixl_platform::wide::to_wide;
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, IsIconic, SW_RESTORE, SW_SHOWNORMAL, SetForegroundWindow, ShowWindow,
};

pub const WINDOW_TITLE: &str = "Pixl";

/// True if this is the only settings window process.
pub fn claim_single_instance() -> bool {
    let name = to_wide(r"Local\Pixl.Settings");
    // Leaked on purpose: the mutex must live as long as the process.
    let _ = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
    unsafe { GetLastError() != ERROR_ALREADY_EXISTS }
}

pub fn focus_existing_window() {
    // The tray app's hidden window is titled "Pixl Tray", so this finds ours.
    let title = to_wide(WINDOW_TITLE);
    let class = to_wide("Tauri Window");
    let mut hwnd = unsafe { FindWindowW(class.as_ptr(), title.as_ptr()) };
    if hwnd.is_null() {
        hwnd = unsafe { FindWindowW(null(), title.as_ptr()) };
    }
    if !hwnd.is_null() {
        unsafe {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            SetForegroundWindow(hwnd);
        }
    }
}

/// The settings window only edits the config; the tray app does the work.
/// If someone opened the settings directly, start the tray app too.
pub fn ensure_tray_running() -> bool {
    if tray::is_running() {
        return true;
    }
    match tray_exe() {
        Some(exe) => std::process::Command::new(exe).arg("--background").spawn().is_ok(),
        None => false,
    }
}

/// `Pixl.exe` next to this executable (`pixl-tray.exe` in a dev build).
pub fn tray_exe() -> Option<PathBuf> {
    let me = std::env::current_exe().ok()?;
    ["Pixl.exe", "pixl-tray.exe"].iter().map(|n| me.with_file_name(n)).find(|p| p.exists())
}

/// The user's Windows accent colour as `#rrggbb`.
pub fn system_accent() -> String {
    let key = to_wide(r"Software\Microsoft\Windows\DWM");
    let name = to_wide("AccentColor");
    let mut v: u32 = 0;
    let mut len = size_of::<u32>() as u32;
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            (&raw mut v).cast(),
            &mut len,
        )
    };
    if rc != 0 {
        return "#0067c0".into();
    }
    // Stored as 0xAABBGGRR.
    let (r, g, b) = (v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF);
    format!("#{r:02x}{g:02x}{b:02x}")
}

pub fn open(target: &str) {
    let target = to_wide(target);
    unsafe { ShellExecuteW(null_mut(), to_wide("open").as_ptr(), target.as_ptr(), null(), null(), SW_SHOWNORMAL) };
}
