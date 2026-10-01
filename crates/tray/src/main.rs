// Debug builds keep a console for logs; release builds are a pure tray app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod icon;
mod input;
mod overlay;
mod power;
mod sampler;
mod watch;

use std::ptr::null;

use pixl_platform::ddc::{self, Physical, VCP_POWER};
use pixl_platform::display;
use pixl_platform::tray::{self, MSG_OPEN_SETTINGS};
use pixl_platform::wide::to_wide;
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};

/// `Pixl.exe`              start in the tray and open the settings window
/// `Pixl.exe --background` start quietly (used by "Start with Windows")
/// `Pixl.exe --list`       print the screens and whether each can be powered off
fn main() {
    // Without this every coordinate Windows hands us is scaled and wrong.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list") {
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
        list();
        return;
    }
    let background = args.iter().any(|a| a == "--background");

    let name = to_wide(r"Local\Pixl.Tray");
    let _mutex = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        // Already running: opening the app again means "show me the window".
        if !background {
            tray::post(MSG_OPEN_SETTINGS, 0, 0);
        }
        return;
    }
    app::run(!background);
}

fn list() {
    for d in display::detect() {
        println!(
            "{}. {}{}  [{}]\n   {}x{} at ({}, {}){}{}",
            d.number,
            d.name,
            if d.primary { " (main)" } else { "" },
            d.id,
            d.px.w,
            d.px.h,
            d.px.x,
            d.px.y,
            d.inches.map(|i| format!(", {i:.1}\"")).unwrap_or_default(),
            if d.connection.is_empty() { String::new() } else { format!(", {}", d.connection) },
        );
        if d.internal {
            println!("   built-in screen: no DDC/CI");
            continue;
        }
        match Physical::open(d.hmonitor) {
            None => println!("   DDC/CI: can't open the monitor"),
            Some(m) => {
                let caps = m.capabilities();
                let listed = caps.as_deref().is_some_and(|c| ddc::supports(c, VCP_POWER));
                let power = m.get(VCP_POWER);
                println!(
                    "   DDC/CI: capabilities {}, power mode {} (current value {})",
                    if caps.is_some() { "read" } else { "not answered" },
                    if listed { "listed" } else { "not listed" },
                    power.map_or("none".into(), |v| format!("{v:#04x}")),
                );
                if let Some(c) = caps {
                    println!("   {c}");
                }
            }
        }
    }
}
