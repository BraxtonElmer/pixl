// No console window behind the settings window.
#![windows_subsystem = "windows"]

mod commands;
mod material;
mod system;

use pixl_platform::config::Config;
use tauri::{LogicalSize, Manager, Theme};

/// `pixl-settings --check-update`: started by the tray app about once a day.
/// Looks for a new version and only shows a small window if there is one.
const CHECK_UPDATE_ARG: &str = "--check-update";

fn main() {
    let update_check = std::env::args().any(|a| a == CHECK_UPDATE_ARG);

    // Only one settings window: a second launch brings the first one forward.
    // A background update check simply stops (the open window checks by itself).
    if !system::claim_single_instance() {
        if !update_check {
            system::focus_existing_window();
        }
        return;
    }
    system::ensure_tray_running();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(commands::LaunchMode(if update_check { "update" } else { "settings" }))
        .invoke_handler(tauri::generate_handler![
            commands::launch_mode,
            commands::get_state,
            commands::save_config,
            commands::watch,
            commands::start_tray,
            commands::pause,
            commands::turn_all,
            commands::open_apps,
            commands::set_start_with_windows,
            commands::apply_material,
            commands::open_link,
        ])
        .setup(move |app| {
            let window = app.get_webview_window("main").expect("main window");
            if update_check {
                // A compact window for the "update available" prompt.
                let _ = window.set_min_size(Some(LogicalSize::new(440.0, 320.0)));
                let _ = window.set_size(LogicalSize::new(480.0, 380.0));
                let _ = window.set_resizable(false);
                let _ = window.center();
            }
            // Set the saved material before the page shows, so it doesn't flash.
            let a = Config::load().appearance;
            let dark = match a.theme.as_str() {
                "dark" => true,
                "light" => false,
                _ => window.theme().is_ok_and(|t| t == Theme::Dark),
            };
            material::apply(&window, &a.material, dark);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start the settings window");
}
