//! Files the tray app writes and the settings window only reads:
//!
//! - `power.json`: what Pixl has learned about each monitor's power control
//!   (tested and works, or didn't come back). Kept across restarts.
//! - `status.json`: what's happening right now (timers, which screens are
//!   off), refreshed every half second while the settings window is open.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::{dir, load_json, save_json};
use crate::display::Rect;

/// Whether a monitor can be powered off over DDC/CI.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PowerSupport {
    /// Still asking the monitor.
    #[default]
    Checking,
    /// It doesn't answer, or doesn't list the power command.
    Unsupported,
    /// It says it can; not tried yet.
    Untested,
    /// Tried: it turned off and came back on by itself.
    Works,
    /// Tried: it didn't come back, or Windows lost it while it was off.
    Failed,
}

/// The outcome of a power-off attempt, remembered per monitor id.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PowerResult {
    pub works: bool,
    /// Plain-language reason shown in the settings window.
    pub note: String,
    /// Seconds since 1970.
    pub at: u64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct PowerStore {
    pub monitors: BTreeMap<String, PowerResult>,
}

impl PowerStore {
    fn path() -> PathBuf {
        dir().join("power.json")
    }

    pub fn load() -> Self {
        load_json(&Self::path(), false)
    }

    pub fn save(&self) -> std::io::Result<()> {
        save_json(&Self::path(), self)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ScreenPhase {
    #[default]
    On,
    Fading,
    Off,
}

/// How a screen that's off was turned off.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OffBy {
    Power,
    Black,
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
    pub internal: bool,
    pub power: PowerSupport,
    /// Why power control has the status it has, in plain words.
    pub power_note: String,
    pub phase: ScreenPhase,
    pub off_by: Option<OffBy>,
    /// Seconds until it starts turning off; None while off or not managed.
    pub remaining_secs: Option<u32>,
    /// Why it's being kept on right now, if something is holding it.
    pub held_by: Option<String>,
}

/// Steps of "Test power off", as shown in the settings window.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TestStep {
    /// The off command was sent; waiting a few seconds.
    Off,
    /// The on command was sent; waiting for the monitor to come back.
    Waking,
    /// Done on our side; asking the user whether the screen came back.
    Ask,
    /// The monitor didn't take the off command at all.
    Refused,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TestStatus {
    pub id: String,
    pub step: TestStep,
    /// Windows still saw the monitor while it was off.
    pub stayed_connected: bool,
    /// The monitor answered "on" after the wake command.
    pub reports_on: bool,
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
    pub test: Option<TestStatus>,
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
