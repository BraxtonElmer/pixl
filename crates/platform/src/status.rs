//! `status.json`: what the tray app is doing right now (timers, which screens
//! are off), refreshed every half second while the settings window is open.
//! Only the tray app writes it.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::{dir, save_json};
use crate::display::Rect;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ScreenPhase {
    #[default]
    On,
    Fading,
    Off,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScreenStatus {
    pub id: String,
    pub name: String,
    pub number: u32,
    pub primary: bool,
    pub px: Rect,
    pub inches: Option<f64>,
    pub connection: String,
    pub phase: ScreenPhase,
    /// Seconds until it starts turning off; None while off or not managed.
    pub remaining_secs: Option<u32>,
    /// Why it's being kept on right now, if something is holding it.
    pub held_by: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Milliseconds since 1970 when this was written; old means the tray app stopped.
    pub written: u64,
    pub enabled: bool,
    /// Paused: until this time (ms since 1970), or 0 for until restart.
    pub paused_until: Option<u64>,
    /// Shortcuts another app already uses.
    pub hotkeys_taken: Vec<String>,
    pub screens: Vec<ScreenStatus>,
}

impl Status {
    fn path() -> PathBuf {
        dir().join("status.json")
    }

    pub fn load() -> Option<Self> {
        let text = std::fs::read_to_string(Self::path()).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save(&self) -> std::io::Result<()> {
        save_json(&Self::path(), self)
    }
}

pub fn unix_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}
