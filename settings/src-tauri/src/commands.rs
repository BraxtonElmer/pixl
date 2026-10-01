//! What the settings page can ask for. Settings are written to config.json
//! and the tray app is told to apply them immediately; live state comes back
//! from the tray app through status.json.

use pixl_platform::config::{self, Config};
use pixl_platform::status::{Status, unix_ms};
use pixl_platform::tray::{self, MSG_ALL, MSG_PAUSE, MSG_RELOAD, MSG_WATCH, PAUSE_UNTIL_RESTART};
use pixl_platform::{apps, autostart};
use serde::Serialize;

use crate::system;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    config: Config,
    start_with_windows: bool,
    system_accent: String,
    version: &'static str,
}

#[tauri::command]
pub fn get_state() -> State {
    State {
        config: Config::load(),
        start_with_windows: autostart::is_enabled(),
        system_accent: system::system_accent(),
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// Save every setting and have the tray app apply them.
#[tauri::command]
pub fn save_config(config: Config) -> Result<(), String> {
    config.save().map_err(|e| format!("Couldn't save your settings: {e}"))?;
    tray::post(MSG_RELOAD, 0, 0);
    Ok(())
}

/// What the tray app is doing right now. None when it isn't running (or has
/// stopped answering); the page then offers to start it.
#[tauri::command]
pub fn watch() -> Option<Status> {
    if !tray::post(MSG_WATCH, 0, 0) {
        return None;
    }
    Status::load().filter(|s| unix_ms().saturating_sub(s.written) < 5000)
}

#[tauri::command]
pub fn start_tray() -> bool {
    system::ensure_tray_running()
}

/// Pause for `minutes`; 0 resumes, a negative number pauses until Pixl restarts.
#[tauri::command]
pub fn pause(minutes: i64) -> bool {
    let w = if minutes < 0 { PAUSE_UNTIL_RESTART } else { minutes as usize };
    tray::post(MSG_PAUSE, w, 0)
}

#[tauri::command]
pub fn turn_all(off: bool) -> bool {
    tray::post(MSG_ALL, usize::from(off), 0)
}

/// Programs with a window open right now, to pick from for the keep-on list.
#[tauri::command]
pub fn open_apps() -> Vec<String> {
    apps::with_windows()
}

#[tauri::command]
pub fn set_start_with_windows(on: bool) -> Result<bool, String> {
    let exe = system::tray_exe().ok_or("Pixl.exe wasn't found next to the settings app.")?;
    if autostart::set(on, &exe) {
        Ok(autostart::is_enabled())
    } else {
        Err("Windows didn't allow changing the startup setting.".into())
    }
}

/// Frosted glass or solid, tinted for the current light/dark theme.
/// Returns the material the window actually got.
#[tauri::command]
pub fn apply_material(window: tauri::WebviewWindow, material: String, dark: bool) -> String {
    crate::material::apply(&window, &material, dark).to_string()
}

/// Only fixed destinations: the page can't make us open arbitrary things.
#[tauri::command]
pub fn open_link(which: String) {
    match which.as_str() {
        "source" => system::open("https://github.com/BraxtonElmer/pixl"),
        "issues" => system::open("https://github.com/BraxtonElmer/pixl/issues"),
        "folder" => {
            let dir = config::dir();
            let _ = std::fs::create_dir_all(&dir);
            system::open(&dir.to_string_lossy());
        }
        _ => {}
    }
}
