//! "Stay on while something is playing": every couple of seconds, shrink each
//! idle screen's picture to a 64 x 36 thumbnail and compare it with the last
//! one. Video and games change most of it; a ticking clock or a blinking
//! caret changes a few pixels and is ignored.

use std::collections::HashMap;
use std::ptr::null_mut;

use pixl_platform::display::Rect;
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject,
    GetDC, HALFTONE, ReleaseDC, SRCCOPY, SelectObject, SetBrushOrgEx, SetStretchBltMode, StretchBlt,
};

const W: usize = 64;
const H: usize = 36;
/// A pixel counts as changed past this much difference (sum of R, G and B).
const PIXEL_DELTA: u32 = 36;
/// The picture counts as changed when this share of pixels did.
const CHANGED_SHARE: f64 = 0.03;

#[derive(Default)]
pub struct Sampler {
    last: HashMap<String, Vec<u8>>,
}

impl Sampler {
    /// Returns true when screen `id`'s picture changed since the last call.
    pub fn changed(&mut self, id: &str, rect: Rect) -> bool {
        let Some(now) = thumbnail(rect) else { return false };
        let changed = self.last.get(id).is_some_and(|prev| differs(prev, &now));
        self.last.insert(id.to_string(), now);
        changed
    }

    /// Forget a screen (it was covered or removed), so the next sample starts fresh.
    pub fn forget(&mut self, id: &str) {
        self.last.remove(id);
    }
}

fn differs(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return true;
    }
    let changed = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(p, q)| {
            let d: u32 = (0..3).map(|i| u32::from(p[i].abs_diff(q[i]))).sum();
            d > PIXEL_DELTA
        })
        .count();
    changed as f64 / (W * H) as f64 > CHANGED_SHARE
}

fn thumbnail(r: Rect) -> Option<Vec<u8>> {
    unsafe {
        let screen = GetDC(null_mut());
        if screen.is_null() {
            return None;
        }
        let mem = CreateCompatibleDC(screen);
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: W as i32,
            biHeight: -(H as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut bits: *mut core::ffi::c_void = null_mut();
        let bmp = CreateDIBSection(mem, &bi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        let mut out = None;
        if !bmp.is_null() && !bits.is_null() {
            let old = SelectObject(mem, bmp);
            // Halftone averages the pixels under each thumbnail pixel instead of picking one.
            SetStretchBltMode(mem, HALFTONE);
            SetBrushOrgEx(mem, 0, 0, null_mut());
            if StretchBlt(mem, 0, 0, W as i32, H as i32, screen, r.x, r.y, r.w, r.h, SRCCOPY) != 0 {
                out = Some(std::slice::from_raw_parts(bits as *const u8, W * H * 4).to_vec());
            }
            SelectObject(mem, old);
            DeleteObject(bmp);
        }
        DeleteDC(mem);
        ReleaseDC(null_mut(), screen);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_changes_are_ignored() {
        let a = vec![40u8; W * H * 4];
        let mut b = a.clone();
        // A clock: a handful of pixels change completely.
        for px in b.as_chunks_mut::<4>().0.iter_mut().take(20) {
            px[0] = 255;
            px[1] = 255;
            px[2] = 255;
        }
        assert!(!differs(&a, &b));
        // A video frame: most of the picture moves.
        for px in b.as_chunks_mut::<4>().0.iter_mut().take(W * H / 2) {
            px[1] = 200;
        }
        assert!(differs(&a, &b));
    }
}
