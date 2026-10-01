//! The tray icon, drawn in code: a screen with a single lit pixel in its
//! corner. Matches the taskbar's light/dark theme; faded while paused or off.

use std::ptr::null_mut;

use pixl_platform::wide::to_wide;
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIcon, HICON};

const SIZE: usize = 32;

pub fn tray_icon(active: bool) -> HICON {
    let (r, g, b) = if taskbar_is_light() { (0x1B, 0x1B, 0x1B) } else { (0xFF, 0xFF, 0xFF) };
    let alpha: u8 = if active { 0xFF } else { 0x70 };
    let mut px = [[0u8; 4]; SIZE * SIZE];
    let mut set = |x: usize, y: usize| px[y * SIZE + x] = [b, g, r, alpha];

    // Screen: 2 px outline with rounded-off corners.
    let (x0, y0, x1, y1) = (2, 5, 29, 23);
    for x in x0..=x1 {
        for y in y0..=y1 {
            let edge = x <= x0 + 1 || x >= x1 - 1 || y <= y0 + 1 || y >= y1 - 1;
            let corner = (x == x0 || x == x1) && (y == y0 || y == y1);
            if edge && !corner {
                set(x, y);
            }
        }
    }
    // Stand.
    for x in 14..=17 {
        for y in 24..=26 {
            set(x, y);
        }
    }
    for x in 10..=21 {
        for y in 27..=28 {
            set(x, y);
        }
    }
    // The lit pixel.
    for x in 22..=25 {
        for y in 9..=12 {
            set(x, y);
        }
    }

    let xor: Vec<u8> = px.iter().flatten().copied().collect();
    let and = [0u8; SIZE * SIZE / 8];
    unsafe { CreateIcon(null_mut(), SIZE as i32, SIZE as i32, 1, 32, and.as_ptr(), xor.as_ptr()) }
}

fn taskbar_is_light() -> bool {
    let key = to_wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let name = to_wide("SystemUsesLightTheme");
    let mut v: u32 = 0;
    let mut len = size_of::<u32>() as u32;
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            (&raw mut v).cast(),
            &mut len,
        )
    };
    rc == 0 && v != 0
}
