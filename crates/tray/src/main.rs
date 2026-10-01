// Debug builds keep a console for logs; release builds are a pure tray app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod icon;
mod input;
mod log;
mod overlay;
mod sampler;
mod watch;

use std::ptr::null;

use pixl_platform::display;
use pixl_platform::tray::{self, MSG_OPEN_SETTINGS};
use pixl_platform::wide::to_wide;
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};

/// `Pixl.exe`              start in the tray and open the settings window
/// `Pixl.exe --background` start quietly (used by "Start with Windows")
/// `Pixl.exe --list`       print the screens Pixl sees
/// `Pixl.exe --watch N`    print every second how much of screen N's picture changed
fn main() {
    // Without this every coordinate Windows hands us is scaled and wrong.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list") {
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
        list();
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--watch") {
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
        watch_picture(args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(1));
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
    }
}

/// What "Stay on while something is playing" sees on screen `number`: the
/// share of the picture that changed each second, and its brightness (a
/// picture Windows won't let us see comes back black). Runs for 30 seconds.
fn watch_picture(number: u32) {
    let Some(d) = display::detect().into_iter().find(|d| d.number == number) else {
        println!("no screen {number}");
        return;
    };
    audio::init();
    println!("{}", audio::describe());
    println!("sound anywhere: {:?}", audio::sounding());
    println!("apps on this screen: {:?}", pixl_platform::apps::on_screens().remove(&d.hmonitor).unwrap_or_default());
    println!("watching {} ({}x{}); counts as playing above 3%, or with sound here", d.name, d.px.w, d.px.h);
    let mut prev = sampler::thumbnail(d.px);
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let now = sampler::thumbnail(d.px);
        if let (Some(a), Some(b)) = (&prev, &now) {
            let share = sampler::changed_share(a, b) * 100.0;
            let sound = audio::sounding();
            let here = pixl_platform::apps::on_screens().remove(&d.hmonitor).unwrap_or_default();
            let mut playing: Vec<_> = here.intersection(&sound).cloned().collect();
            playing.sort();
            println!("changed {share:5.1}%   brightness {:5.1}   sound here: {playing:?}", sampler::brightness(b));
        } else {
            println!("couldn't capture the screen");
        }
        prev = now;
    }
}
