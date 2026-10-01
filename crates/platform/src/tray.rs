//! How the settings window (or a second launch) talks to the running tray app:
//! a window message to its hidden window. Nothing to connect, nothing to leak.

use std::ptr::null;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, RegisterWindowMessageW, WM_APP};

use crate::wide::to_wide;

pub const WINDOW_CLASS: &str = "PixlTray";

/// Re-read config.json and apply it.
pub const MSG_RELOAD: u32 = WM_APP + 2;
/// Open (or focus) the settings window.
pub const MSG_OPEN_SETTINGS: u32 = WM_APP + 3;
/// The settings window is open: keep status.json fresh for a few seconds.
pub const MSG_WATCH: u32 = WM_APP + 4;
/// Pause for wparam minutes; 0 resumes, `PAUSE_UNTIL_RESTART` pauses until Pixl restarts.
pub const MSG_PAUSE: u32 = WM_APP + 5;
/// Start "Test power off" on the screen at index wparam of status.json.
pub const MSG_TEST_START: u32 = WM_APP + 6;
/// The user's answer to the test: lparam 1 = it came back, 0 = it stayed dark.
pub const MSG_TEST_ANSWER: u32 = WM_APP + 7;
/// Stop a running test and wake the screen.
pub const MSG_TEST_CANCEL: u32 = WM_APP + 8;
/// Ask the monitors about power control again (forgets earlier test results for wparam index, or all with usize::MAX).
pub const MSG_RECHECK: u32 = WM_APP + 9;
/// Turn every screen off now (wparam 1) or wake every screen (wparam 0).
pub const MSG_ALL: u32 = WM_APP + 10;

pub const PAUSE_UNTIL_RESTART: usize = usize::MAX;

pub fn window() -> Option<HWND> {
    let class = to_wide(WINDOW_CLASS);
    let hwnd = unsafe { FindWindowW(class.as_ptr(), null()) };
    (!hwnd.is_null()).then_some(hwnd)
}

pub fn is_running() -> bool {
    window().is_some()
}

/// Returns false when the tray app isn't running.
pub fn post(msg: u32, wparam: usize, lparam: isize) -> bool {
    match window() {
        Some(hwnd) => unsafe { PostMessageW(hwnd, msg, wparam, lparam) != 0 },
        None => false,
    }
}

/// Windows' "TaskbarCreated" broadcast, sent when Explorer restarts.
pub fn taskbar_created_message() -> u32 {
    unsafe { RegisterWindowMessageW(to_wide("TaskbarCreated").as_ptr()) }
}
