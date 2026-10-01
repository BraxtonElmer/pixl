//! Running programs, by exe name: for "keep screens on while these apps are
//! open" and for offering apps to pick from in the settings window.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use windows_sys::Win32::Foundation::{BOOL, CloseHandle, HWND, INVALID_HANDLE_VALUE, LPARAM};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GetWindow, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible,
};

use crate::wide::from_wide;

/// Lower-case exe names of every running process.
pub fn running() -> HashSet<String> {
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

/// Exe names of programs that have a window open right now, sorted.
pub fn with_windows() -> Vec<String> {
    unsafe extern "system" fn cb(hwnd: HWND, data: LPARAM) -> BOOL {
        let pids = unsafe { &mut *(data as *mut HashSet<u32>) };
        let shown = unsafe { IsWindowVisible(hwnd) } != 0
            && unsafe { GetWindow(hwnd, GW_OWNER) }.is_null()
            && unsafe { GetWindowTextLengthW(hwnd) } > 0;
        if shown {
            let mut pid = 0;
            unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
            pids.insert(pid);
        }
        1
    }
    let mut pids: HashSet<u32> = HashSet::new();
    unsafe { EnumWindows(Some(cb), (&raw mut pids) as LPARAM) };

    let own = std::process::id();
    let names: BTreeSet<String> = pids
        .into_iter()
        .filter(|&pid| pid != own)
        .filter_map(exe_name)
        .filter(|n| {
            !matches!(n.to_lowercase().as_str(), "explorer.exe" | "applicationframehost.exe" | "textinputhost.exe")
        })
        .collect();
    names.into_iter().collect()
}

fn exe_name(pid: u32) -> Option<String> {
    let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if h.is_null() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) };
    unsafe { CloseHandle(h) };
    if ok == 0 {
        return None;
    }
    let path = from_wide(&buf[..len as usize]);
    Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned())
}
