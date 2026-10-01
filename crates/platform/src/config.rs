//! `%APPDATA%\Pixl\config.json`: everything the settings window lets you
//! change. Only the settings window writes it; the tray app reads it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pixl_core::{Rule, Trigger, Wake};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// How long a fade lasts when "Fade out first" is on.
pub const FADE_MS: u64 = 5000;
/// Music players: their sound doesn't mean someone is watching the screen.
pub const MUSIC_APPS: [&str; 10] = [
    "Spotify.exe",
    "AppleMusic.exe",
    "iTunes.exe",
    "TIDAL.exe",
    "Deezer.exe",
    "Amazon Music.exe",
    "foobar2000.exe",
    "MusicBee.exe",
    "AIMP.exe",
    "Winamp.exe",
];
/// Shortest and longest timeouts the settings window offers.
pub const MIN_TIMEOUT_SECS: u32 = 10;
pub const MAX_TIMEOUT_SECS: u32 = 24 * 60 * 60;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    pub enabled: bool,
    /// A fullscreen game or presentation keeps its screen on.
    pub pause_in_fullscreen: bool,
    /// Apps that ask Windows to keep the display on (video players, calls)
    /// keep screens using "When I stop using the PC" on.
    pub respect_keep_awake: bool,
    /// Every screen stays on while one of these programs is running (exe names).
    pub keep_on_apps: Vec<String>,
    /// Sound from known music players (`MUSIC_APPS`) doesn't count as
    /// "something is playing", so music alone doesn't keep a screen on.
    pub ignore_music_players: bool,
    /// More programs whose sound doesn't count, added by the user (exe names).
    pub ignore_sound_from: Vec<String>,
    pub hotkeys: Hotkeys,
    pub appearance: Appearance,
    /// Rules per monitor id. Monitors without an entry use the defaults.
    pub screens: BTreeMap<String, ScreenRule>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: true,
            pause_in_fullscreen: true,
            respect_keep_awake: false,
            keep_on_apps: Vec::new(),
            ignore_music_players: true,
            ignore_sound_from: Vec::new(),
            hotkeys: Hotkeys::default(),
            appearance: Appearance::default(),
            screens: BTreeMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TriggerSetting {
    /// "When I stop using the PC"
    Pc,
    /// "When I'm not using this screen"
    Away,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WakeSetting {
    Any,
    Cursor,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct ScreenRule {
    pub enabled: bool,
    pub trigger: TriggerSetting,
    pub timeout_secs: u32,
    pub wake: WakeSetting,
    pub typing_counts: bool,
    pub stay_on_while_playing: bool,
    pub fade: bool,
}

impl Default for ScreenRule {
    fn default() -> Self {
        Self {
            enabled: true,
            trigger: TriggerSetting::Pc,
            timeout_secs: 30 * 60,
            wake: WakeSetting::Any,
            typing_counts: true,
            stay_on_while_playing: true,
            fade: true,
        }
    }
}

impl ScreenRule {
    pub fn to_rule(&self) -> Rule {
        Rule {
            enabled: self.enabled,
            trigger: match self.trigger {
                TriggerSetting::Pc => Trigger::PcIdle,
                TriggerSetting::Away => Trigger::AwayFromScreen,
            },
            timeout_ms: u64::from(self.timeout_secs.clamp(MIN_TIMEOUT_SECS, MAX_TIMEOUT_SECS)) * 1000,
            wake: match self.wake {
                WakeSetting::Any => Wake::AnyInput,
                WakeSetting::Cursor => Wake::CursorEnters,
            },
            typing_counts: self.typing_counts,
            stay_on_while_playing: self.stay_on_while_playing,
            fade_ms: if self.fade { FADE_MS } else { 0 },
        }
    }
}

/// A global shortcut: Windows virtual-key code plus modifiers.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    pub key: u32,
}

impl Hotkey {
    pub const fn ctrl_alt(key: u8) -> Self {
        Self { ctrl: true, alt: true, shift: false, win: false, key: key as u32 }
    }

    /// "Ctrl+Alt+O"
    pub fn label(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for (on, name) in [(self.ctrl, "Ctrl"), (self.alt, "Alt"), (self.shift, "Shift"), (self.win, "Win")] {
            if on {
                parts.push(name.into());
            }
        }
        parts.push(key_name(self.key));
        parts.join("+")
    }
}

fn key_name(vk: u32) -> String {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(vk).map_or_else(String::new, String::from),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0x60..=0x69 => format!("Num {}", vk - 0x60),
        0x20 => "Space".into(),
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x13 => "Pause".into(),
        0x1B => "Esc".into(),
        0x21 => "Page Up".into(),
        0x22 => "Page Down".into(),
        0x23 => "End".into(),
        0x24 => "Home".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0x2D => "Insert".into(),
        0x2E => "Delete".into(),
        0xBA => ";".into(),
        0xBB => "=".into(),
        0xBC => ",".into(),
        0xBD => "-".into(),
        0xBE => ".".into(),
        0xBF => "/".into(),
        0xC0 => "`".into(),
        0xDB => "[".into(),
        0xDC => "\\".into(),
        0xDD => "]".into(),
        0xDE => "'".into(),
        _ => format!("Key {vk:#04X}"),
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Hotkeys {
    pub turn_off_all: Option<Hotkey>,
    pub wake_all: Option<Hotkey>,
    pub pause: Option<Hotkey>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            turn_off_all: Some(Hotkey::ctrl_alt(b'O')),
            wake_all: Some(Hotkey::ctrl_alt(b'W')),
            pause: Some(Hotkey::ctrl_alt(b'P')),
        }
    }
}

/// Settings window look. The tray app ignores it.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Appearance {
    /// "system", "light" or "dark".
    pub theme: String,
    /// Accent colour as `#rrggbb`; empty = the Windows accent colour.
    pub accent: String,
    /// Window background: "acrylic" (frosted glass) or "solid".
    pub material: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Self { theme: "system".into(), accent: String::new(), material: "acrylic".into() }
    }
}

pub fn dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Pixl")
}

impl Config {
    pub fn path() -> PathBuf {
        dir().join("config.json")
    }

    /// A missing file gives defaults; a broken one is kept as `config.json.bad`
    /// so hand edits are never silently lost.
    pub fn load() -> Self {
        let mut c: Self = load_json(&Self::path(), true);
        c.tidy();
        c
    }

    /// Earlier versions listed the music players in `ignore_sound_from`;
    /// they're built in now, so only apps the user added stay there.
    fn tidy(&mut self) {
        self.ignore_sound_from.retain(|a| !MUSIC_APPS.iter().any(|m| m.eq_ignore_ascii_case(a)));
    }

    /// Lower-case exe names whose sound doesn't count as playing.
    pub fn ignored_sound(&self) -> Vec<String> {
        let music = if self.ignore_music_players { &MUSIC_APPS[..] } else { &[] };
        music
            .iter()
            .map(|s| s.to_string())
            .chain(self.ignore_sound_from.iter().cloned())
            .map(|s| s.to_lowercase())
            .collect()
    }

    pub fn save(&self) -> std::io::Result<()> {
        save_json(&Self::path(), self)
    }

    pub fn rule_for(&self, id: &str) -> ScreenRule {
        self.screens.get(id).copied().unwrap_or_default()
    }
}

pub(crate) fn load_json<T: DeserializeOwned + Default>(p: &Path, keep_bad: bool) -> T {
    match std::fs::read_to_string(p) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| {
            if keep_bad {
                let _ = std::fs::copy(p, p.with_extension("json.bad"));
            }
            T::default()
        }),
        Err(_) => T::default(),
    }
}

/// Write to a temp file and rename, so a crash never leaves half a file.
pub(crate) fn save_json<T: Serialize>(p: &Path, value: &T) -> std::io::Result<()> {
    std::fs::create_dir_all(dir())?;
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(value)?)?;
    std::fs::rename(&tmp, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_or_partial_files_fill_in_defaults() {
        let c: Config = serde_json::from_str(r#"{"enabled":false,"screens":{"A":{"timeoutSecs":90}}}"#).unwrap();
        assert!(!c.enabled);
        assert!(c.pause_in_fullscreen);
        let r = c.rule_for("A");
        assert_eq!(r.timeout_secs, 90);
        assert!(r.fade);
        assert_eq!(c.rule_for("B"), ScreenRule::default());
    }

    #[test]
    fn settings_from_the_page_round_trip() {
        // What the settings page sends: camelCase, a cleared shortcut, and a
        // field left over from an older version.
        let sent = r##"{
            "version": 1, "enabled": true, "pauseInFullscreen": false, "respectKeepAwake": true,
            "keepOnApps": ["obs64.exe"],
            "hotkeys": {"turnOffAll": {"ctrl": true, "alt": true, "shift": false, "win": false, "key": 79},
                        "wakeAll": null, "pause": {"ctrl": true, "alt": false, "shift": true, "win": false, "key": 80}},
            "appearance": {"theme": "dark", "accent": "#0f7b6c", "material": "solid"},
            "screens": {"MSI4CC2-1": {"enabled": true, "trigger": "away", "timeoutSecs": 2700, "method": "power",
                        "wake": "cursor", "typingCounts": false, "stayOnWhilePlaying": false, "fade": false}}
        }"##;
        let c: Config = serde_json::from_str(sent).unwrap();
        assert!(!c.pause_in_fullscreen && c.respect_keep_awake);
        assert_eq!(c.keep_on_apps, ["obs64.exe"]);
        assert_eq!(c.hotkeys.wake_all, None);
        assert_eq!(c.hotkeys.pause.map(|h| h.label()).as_deref(), Some("Ctrl+Shift+P"));
        assert_eq!(c.appearance.material, "solid");
        let r = c.rule_for("MSI4CC2-1");
        assert_eq!((r.trigger, r.wake, r.timeout_secs), (TriggerSetting::Away, WakeSetting::Cursor, 2700));
        assert!(!r.typing_counts && !r.stay_on_while_playing && !r.fade);

        // Written back and read again: nothing changes.
        let again: Config = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(&again).unwrap(), serde_json::to_value(&c).unwrap());
    }

    #[test]
    fn music_players_are_built_in() {
        let mut c: Config =
            serde_json::from_str(r#"{"ignoreSoundFrom": ["Spotify.exe", "spotify.exe", "obs64.exe", "Winamp.exe"]}"#)
                .unwrap();
        c.tidy();
        assert_eq!(c.ignore_sound_from, ["obs64.exe"]);
        assert!(c.ignore_music_players);
        assert!(c.ignored_sound().contains(&"spotify.exe".to_string()));
        assert!(c.ignored_sound().contains(&"obs64.exe".to_string()));
        c.ignore_music_players = false;
        assert_eq!(c.ignored_sound(), ["obs64.exe"]);
    }

    #[test]
    fn rules_convert_for_the_engine() {
        let r = ScreenRule { timeout_secs: 2, fade: false, trigger: TriggerSetting::Away, ..Default::default() };
        let e = r.to_rule();
        assert_eq!(e.timeout_ms, u64::from(MIN_TIMEOUT_SECS) * 1000);
        assert_eq!(e.fade_ms, 0);
        assert_eq!(e.trigger, Trigger::AwayFromScreen);
    }
}
