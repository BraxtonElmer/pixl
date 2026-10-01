//! Things that keep screens on regardless of input: a fullscreen game, an app
//! asking Windows to keep the display awake, a program on the keep-on list.

use std::collections::HashSet;

use pixl_platform::wide::from_wide;
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL, MONITORINFO, MonitorFromWindow,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Power::{CallNtPowerInformation, ES_DISPLAY_REQUIRED, SystemExecutionState};
use windows_sys::Win32::UI::Shell::{
    QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowRect};

/// The monitor showing the window that has keyboard focus.
pub fn focus_monitor() -> Option<usize> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return None;
    }
    let mon: HMONITOR = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL) };
    (!mon.is_null()).then_some(mon as usize)
}

/// The monitor a fullscreen game, video or presentation is on, if one is in front.
pub fn fullscreen_monitor() -> Option<usize> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return None;
    }
    // The desktop and taskbar cover the screen too, but they aren't apps.
    let mut class = [0u16; 64];
    let n = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) };
    let class = from_wide(&class[..n.max(0) as usize]);
    if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "PixlBlack") {
        return None;
    }
    let mon = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };

    let mut state = 0;
    let shell_says = unsafe { SHQueryUserNotificationState(&mut state) } == 0
        && matches!(state, QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE);
    if shell_says {
        return Some(mon as usize);
    }

    // Borderless-windowed games aren't reported by the shell.
    let mut w: RECT = unsafe { std::mem::zeroed() };
    if unsafe { GetWindowRect(hwnd, &mut w) } == 0 {
        return None;
    }
    let mut mi: MONITORINFO = unsafe { std::mem::zeroed() };
    mi.cbSize = size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(mon, &mut mi) } == 0 {
        return None;
    }
    let m = mi.rcMonitor;
    let covers = w.left <= m.left && w.top <= m.top && w.right >= m.right && w.bottom >= m.bottom;
    covers.then_some(mon as usize)
}

/// Some app (a video player, a call, a presentation) asked Windows to keep the display on.
pub fn display_kept_awake() -> bool {
    let mut state: u32 = 0;
    let rc = unsafe {
        CallNtPowerInformation(
            SystemExecutionState,
            std::ptr::null(),
            0,
            (&raw mut state).cast(),
            size_of::<u32>() as u32,
        )
    };
    rc == 0 && state & ES_DISPLAY_REQUIRED != 0
}

/// Lower-case exe names of every running process.
pub fn running_programs() -> HashSet<String> {
    let mut names = HashSet::new();
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return names;
    }
    let mut e: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    e.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut ok = unsafe { Process32FirstW(snap, &mut e) } != 0;
    while ok {
        names.insert(from_wide(&e.szExeFile).to_lowercase());
        ok = unsafe { Process32NextW(snap, &mut e) } != 0;
    }
    unsafe { CloseHandle(snap) };
    names
}
