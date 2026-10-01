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
/// `Pixl.exe --power-test N [value]` turn screen N off, then try to wake it, reporting each step
fn main() {
    // Without this every coordinate Windows hands us is scaled and wrong.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list") {
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
        list();
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--power-test") {
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
        let n = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(1);
        let value = args.get(i + 2).and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());
        power_test(n, value);
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

/// Diagnose power control on screen `number`: off, then every way we have of
/// bringing it back, printing what the monitor and Windows say at each step.
fn power_test(number: u32, value: Option<u32>) {
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    let start = Instant::now();
    let log = |msg: &str| println!("[{:5.1}s] {msg}", start.elapsed().as_secs_f32());
    let find = || display::detect().into_iter().find(|d| d.number == number);
    let Some(d) = find() else {
        log(&format!("no screen {number}"));
        return;
    };
    let Some(m) = Physical::open(d.hmonitor) else {
        log("can't open the monitor over DDC/CI");
        return;
    };
    let caps = m.capabilities();
    let off = value.unwrap_or_else(|| ddc::off_value(caps.as_deref()));
    log(&format!("{} [{}], power mode now {:?}", d.name, d.id, m.get(VCP_POWER)));
    log(&format!("sending off ({off:#04x})"));
    log(&format!("  accepted: {}", m.set(VCP_POWER, off)));
    drop(m);

    for _ in 0..3 {
        sleep(Duration::from_secs(2));
        let now = find();
        let reading = now.as_ref().and_then(|d| Physical::open(d.hmonitor)).and_then(|m| m.get(VCP_POWER));
        log(&format!("  Windows sees it: {}, power mode {reading:?}", now.is_some()));
    }

    // 1. The plain DDC/CI "on" command, a few times.
    for attempt in 1..=3 {
        let Some(d) = find() else {
            log("Windows lost the screen; DDC/CI can't reach it");
            break;
        };
        let m = Physical::open(d.hmonitor);
        let ok = m.as_ref().is_some_and(|m| m.set(VCP_POWER, ddc::POWER_ON));
        log(&format!("sending on (attempt {attempt}): accepted {ok}"));
        sleep(Duration::from_secs(3));
        let reading = find().and_then(|d| Physical::open(d.hmonitor)).and_then(|m| m.get(VCP_POWER));
        log(&format!("  power mode {reading:?}"));
        if reading == Some(ddc::POWER_ON) {
            break;
        }
    }
    // 2. Re-apply the display setup: a fresh signal wakes some monitors.
    let reading = find().and_then(|d| Physical::open(d.hmonitor)).and_then(|m| m.get(VCP_POWER));
    if reading != Some(ddc::POWER_ON) {
        use windows_sys::Win32::Devices::Display::{SDC_APPLY, SDC_USE_DATABASE_CURRENT, SetDisplayConfig};
        let rc =
            unsafe { SetDisplayConfig(0, std::ptr::null(), 0, std::ptr::null(), SDC_APPLY | SDC_USE_DATABASE_CURRENT) };
        log(&format!("re-applied the display setup (result {rc})"));
        sleep(Duration::from_secs(4));
        let now = find();
        let reading = now.as_ref().and_then(|d| Physical::open(d.hmonitor)).and_then(|m| m.get(VCP_POWER));
        log(&format!("  Windows sees it: {}, power mode {reading:?}", now.is_some()));
    }
    log("done. Is the screen on? If not, press its power button.");
}
