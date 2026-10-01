//! Windows side of Pixl shared by the tray app and the settings window:
//! what's connected, talking to monitors, and the files both sides read.

pub mod autostart;
pub mod config;
pub mod ddc;
pub mod display;
pub mod edid;
pub mod status;
pub mod tray;
pub mod wide;
