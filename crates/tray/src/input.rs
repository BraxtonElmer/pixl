//! When the user last used the mouse and the keyboard, without hooking either.
//!
//! Windows remembers the time of the last input of any kind, so Pixl can
//! sleep and simply ask when it wakes. Two things need input as it happens,
//! and get it as raw input sent to our hidden window, only while needed:
//!
//! - key presses, when a setting has to tell typing apart from the mouse;
//! - any input while a screen is dark, so it wakes the moment you return.

use windows_sys::Win32::Foundation::{HWND, LPARAM, POINT};
use windows_sys::Win32::System::SystemInformation::{GetTickCount, GetTickCount64};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows_sys::Win32::UI::Input::{
    GetRawInputData, HRAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RID_HEADER, RIDEV_INPUTSINK, RIDEV_REMOVE,
    RIM_TYPEKEYBOARD, RIM_TYPEMOUSE, RegisterRawInputDevices,
};
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

const USAGE_MOUSE: u16 = 0x02;
const USAGE_KEYBOARD: u16 = 0x06;

pub fn now() -> u64 {
    unsafe { GetTickCount64() }
}

pub fn cursor() -> (i32, i32) {
    let mut pt = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut pt) };
    (pt.x, pt.y)
}

/// What a WM_INPUT message was.
pub enum Raw {
    Mouse,
    Key,
    Other,
}

pub fn raw_kind(lparam: LPARAM) -> Raw {
    let mut header: RAWINPUTHEADER = unsafe { std::mem::zeroed() };
    let mut size = size_of::<RAWINPUTHEADER>() as u32;
    let got = unsafe {
        GetRawInputData(
            lparam as HRAWINPUT,
            RID_HEADER,
            (&raw mut header).cast(),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        )
    };
    match header.dwType {
        _ if got == u32::MAX => Raw::Other,
        RIM_TYPEMOUSE => Raw::Mouse,
        RIM_TYPEKEYBOARD => Raw::Key,
        _ => Raw::Other,
    }
}

#[derive(Default)]
pub struct Input {
    pub last_mouse: u64,
    pub last_key: u64,
    last_any: u64,
    last_cursor: (i32, i32),
    key_since_sample: bool,
    /// Raw input we're currently signed up for.
    keys: bool,
    mouse: bool,
}

impl Input {
    pub fn new() -> Self {
        Self { last_any: last_input(), last_cursor: cursor(), ..Self::default() }
    }

    /// Sign up for (or drop) raw key presses and mouse input.
    pub fn listen(&mut self, hwnd: HWND, keys: bool, mouse: bool) {
        if keys != self.keys {
            self.keys = keys;
            register(hwnd, USAGE_KEYBOARD, keys);
        }
        if mouse != self.mouse {
            self.mouse = mouse;
            register(hwnd, USAGE_MOUSE, mouse);
        }
    }

    pub fn key_pressed(&mut self) {
        self.last_key = now();
        self.key_since_sample = true;
    }

    pub fn mouse_moved(&mut self) {
        self.last_mouse = now();
    }

    /// Bring last_mouse up to date from Windows' last-input time.
    pub fn sample(&mut self) {
        let any = last_input();
        let pos = cursor();
        let moved = pos != self.last_cursor;
        self.last_cursor = pos;
        if any > self.last_any {
            // A click or wheel without movement also counts as the mouse,
            // unless a key press explains the new input. Without raw key
            // presses, typing counts as the mouse too, which is what the
            // settings that don't ask for them want.
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

fn register(hwnd: HWND, usage: u16, on: bool) {
    let dev = if on {
        RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: usage, dwFlags: RIDEV_INPUTSINK, hwndTarget: hwnd }
    } else {
        RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: usage, dwFlags: RIDEV_REMOVE, hwndTarget: std::ptr::null_mut() }
    };
    unsafe { RegisterRawInputDevices(&dev, 1, size_of::<RAWINPUTDEVICE>() as u32) };
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
