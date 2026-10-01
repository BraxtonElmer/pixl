//! When the user last used the mouse and the keyboard, without hooking either.
//!
//! Windows tracks the time of the last input of any kind. Key presses are
//! told apart through raw keyboard input sent to our hidden window (a few
//! messages per keystroke, nothing per mouse move); any other input that moved
//! the clock is the mouse.

use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::System::SystemInformation::{GetTickCount, GetTickCount64};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows_sys::Win32::UI::Input::{RAWINPUTDEVICE, RIDEV_INPUTSINK, RegisterRawInputDevices};
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

pub fn now() -> u64 {
    unsafe { GetTickCount64() }
}

/// Ask Windows to send key presses to `hwnd` as WM_INPUT, even in the background.
pub fn listen_for_keys(hwnd: HWND) {
    let dev = RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x06, dwFlags: RIDEV_INPUTSINK, hwndTarget: hwnd };
    unsafe { RegisterRawInputDevices(&dev, 1, size_of::<RAWINPUTDEVICE>() as u32) };
}

pub fn cursor() -> (i32, i32) {
    let mut pt = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut pt) };
    (pt.x, pt.y)
}

#[derive(Default)]
pub struct Input {
    pub last_mouse: u64,
    pub last_key: u64,
    last_any: u64,
    last_cursor: (i32, i32),
    key_since_sample: bool,
}

impl Input {
    pub fn new() -> Self {
        Self { last_any: last_input(), last_cursor: cursor(), ..Self::default() }
    }

    /// A WM_INPUT key press arrived.
    pub fn key_pressed(&mut self) {
        let t = now();
        self.last_key = t;
        self.key_since_sample = true;
    }

    /// Bring last_mouse up to date. Call every tick.
    pub fn sample(&mut self) {
        let any = last_input();
        let pos = cursor();
        let moved = pos != self.last_cursor;
        self.last_cursor = pos;
        if any > self.last_any {
            // A click or wheel without movement also counts as the mouse,
            // unless a key press explains the new input.
            if moved || !self.key_since_sample {
                self.last_mouse = self.last_mouse.max(any);
            }
            self.last_any = any;
        }
        // A cursor moved by a program rather than the user doesn't move the
        // input clock, so it isn't counted.
        self.key_since_sample = false;
    }
}

/// Time of the last input in GetTickCount64 terms (Windows reports 32 bits).
fn last_input() -> u64 {
    let mut li = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    if unsafe { GetLastInputInfo(&mut li) } == 0 {
        return 0;
    }
    let ago = unsafe { GetTickCount() }.wrapping_sub(li.dwTime);
    now().saturating_sub(u64::from(ago))
}
