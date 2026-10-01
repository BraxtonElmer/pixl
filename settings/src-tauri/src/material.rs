//! The window's background material: frosted glass (Acrylic) or solid.

use tauri::WebviewWindow;
use tauri::window::{Color, Effect, EffectsBuilder};

/// Apply `wanted` and return what the window actually got ("acrylic" or
/// "solid") so the page can pick matching surface colours. Settings saved when
/// Mica was still offered get frosted glass.
pub fn apply(window: &WebviewWindow, wanted: &str, dark: bool) -> &'static str {
    let effects = match wanted {
        // A dense graphite (or pale grey) tint: frosted, not see-through, so
        // text stays readable over anything behind the window.
        "acrylic" | "mica" => Some((
            "acrylic",
            EffectsBuilder::new()
                .effect(Effect::Acrylic)
                .color(if dark { Color(20, 20, 23, 210) } else { Color(214, 214, 218, 200) })
                .build(),
        )),
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
