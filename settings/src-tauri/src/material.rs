//! The window's background material: frosted glass (Acrylic), Mica, or solid.

use tauri::WebviewWindow;
use tauri::window::{Color, Effect, EffectsBuilder};

use crate::system;

/// Apply `wanted` and return what the window actually got ("acrylic",
/// "mica" or "solid") so the page can pick matching surface colours.
pub fn apply(window: &WebviewWindow, wanted: &str, dark: bool) -> &'static str {
    let effects = match wanted {
        // A dense graphite (or pale grey) tint: frosted, not see-through, so
        // text stays readable over anything behind the window.
        "acrylic" => Some((
            "acrylic",
            EffectsBuilder::new()
                .effect(Effect::Acrylic)
                .color(if dark { Color(20, 20, 23, 210) } else { Color(214, 214, 218, 200) })
                .build(),
        )),
        "mica" if system::supports_mica() => Some(("mica", EffectsBuilder::new().effect(Effect::Mica).build())),
        _ => None,
    };
    if let Some((name, config)) = effects
        && window.set_effects(config).is_ok()
    {
        return name;
    }
    let _ = window.set_effects(None);
    "solid"
}
