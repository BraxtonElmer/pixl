// No console window behind the settings window.
#![windows_subsystem = "windows"]

mod commands;
mod material;
mod system;

use pixl_platform::config::Config;
use tauri::{Manager, Theme};

fn main() {
    // Only one settings window: a second launch brings the first one forward.
    if !system::claim_single_instance() {
        system::focus_existing_window();
        return;
    }
    system::ensure_tray_running();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
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
        .setup(|app| {
            let window = app.get_webview_window("main").expect("main window");
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
