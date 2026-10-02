//! Black windows that cover a screen: the fade, and the "black screen" way of
//! turning a screen off. On OLED, black pixels are switched off, so this
//! protects the panel almost as well as powering it down.
//!
//! The windows never take focus. While fading they let clicks through; once
//! fully black they swallow clicks so nothing hidden gets clicked by accident,
//! and hide the cursor so it doesn't sit on the panel.
//!
//! A solid cover also tells the tray app when the mouse moves over it. Pixl
//! listens for input anyway while a screen is dark, but Windows doesn't pass
//! that on while an app running as administrator is in front; the cover sees
//! the mouse regardless, so moving it always wakes the screen at once.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ptr::{null, null_mut};

use pixl_platform::display::Rect;
use pixl_platform::wide::to_wide;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{BLACK_BRUSH, GetStockObject};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWL_EXSTYLE, GetCursorPos, GetWindowLongPtrW, HWND_TOPMOST,
    LWA_ALPHA, MA_NOACTIVATE, PostMessageW, RegisterClassW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SetCursor, SetCursorPos, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow, WM_APP,
    WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_SETCURSOR,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

const CLASS: &str = "PixlBlack";

/// Posted to the tray window when the user moves or clicks the mouse over a cover.
pub const MSG_COVER_MOUSE: u32 = WM_APP + 41;

struct Cover {
    hwnd: HWND,
    rect: Rect,
    /// Fade start (tick ms) and length; None once fully black.
    fade: Option<(u64, u64)>,
}

thread_local! {
    static COVERS: RefCell<HashMap<String, Cover>> = RefCell::new(HashMap::new());
    /// The tray window, told about mouse use over a cover.
    static OWNER: Cell<usize> = const { Cell::new(0) };
    /// Where the cursor was when we last looked, so only real moves count.
    static LAST_POS: Cell<(i32, i32)> = const { Cell::new((i32::MIN, i32::MIN)) };
}

pub fn set_owner(hwnd: HWND) {
    OWNER.with(|o| o.set(hwnd as usize));
}

fn cursor_pos() -> (i32, i32) {
    let mut pt = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut pt) };
    (pt.x, pt.y)
}

/// The user used the mouse over a cover: tell the tray app. Windows also sends
/// "moved" when a window appears under a still cursor, or when we hide it;
/// those leave the cursor where it was and don't count.
fn mouse_used(moved_only: bool) {
    let pos = cursor_pos();
    if moved_only && LAST_POS.with(|p| p.replace(pos)) == pos {
        return;
    }
    LAST_POS.with(|p| p.set(pos));
    let owner = OWNER.with(Cell::get);
    if owner != 0 {
        unsafe { PostMessageW(owner as HWND, MSG_COVER_MOUSE, 0, 0) };
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_SETCURSOR => {
            unsafe { SetCursor(null_mut()) };
            1
        }
        WM_MOUSEMOVE => {
            mouse_used(true);
            0
        }
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_MOUSEWHEEL => {
            mouse_used(false);
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn register() {
    let class = to_wide(CLASS);
    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: unsafe { GetModuleHandleW(null()) },
        hIcon: null_mut(),
        hCursor: null_mut(),
        hbrBackground: unsafe { GetStockObject(BLACK_BRUSH) },
        lpszMenuName: null(),
        lpszClassName: class.as_ptr(),
    };
    unsafe { RegisterClassW(&wc) }; // fails harmlessly if already registered
}

/// Cover screen `id`: fade in over `fade_ms`, or go black at once with 0.
pub fn show(id: &str, rect: Rect, now: u64, fade_ms: u64) {
    if COVERS.with(|c| c.borrow().get(id).is_some_and(|c| c.rect == rect)) {
        if fade_ms == 0 {
            finish(id);
        }
        return;
    }
    hide(id);
    register();
    let class = to_wide(CLASS);
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            null(),
            WS_POPUP,
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            null_mut(),
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        )
    };
    if hwnd.is_null() {
        return;
    }
    unsafe {
        SetLayeredWindowAttributes(hwnd, 0, if fade_ms == 0 { 255 } else { 0 }, LWA_ALPHA);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    let fade = (fade_ms > 0).then_some((now, fade_ms));
    COVERS.with(|c| c.borrow_mut().insert(id.to_string(), Cover { hwnd, rect, fade }));
    if fade_ms == 0 {
        finish(id);
    }
}

/// Make the cover fully black and solid.
pub fn finish(id: &str) {
    COVERS.with(|c| {
        if let Some(cover) = c.borrow_mut().get_mut(id) {
            cover.fade = None;
            unsafe {
                SetLayeredWindowAttributes(cover.hwnd, 0, 255, LWA_ALPHA);
                let ex = GetWindowLongPtrW(cover.hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(cover.hwnd, GWL_EXSTYLE, ex & !(WS_EX_TRANSPARENT as isize));
            }
            hide_cursor_over(cover.rect);
            // From here on, a cursor still where it is now hasn't moved.
            LAST_POS.with(|p| p.set(cursor_pos()));
        }
    });
}

/// A cursor resting on a black screen would stay lit and burn in. Windows
/// only asks the window under the cursor which cursor to show when the mouse
/// moves, so put it back where it already is: that counts as a move for
/// Windows (but not as the user's input), and the cover answers "none".
fn hide_cursor_over(rect: Rect) {
    let mut pt = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut pt) } == 0 || !rect.contains(pt.x, pt.y) {
        return;
    }
    unsafe {
        SetCursor(null_mut());
        SetCursorPos(pt.x, pt.y);
    }
}

pub fn hide(id: &str) {
    COVERS.with(|c| {
        if let Some(cover) = c.borrow_mut().remove(id) {
            unsafe { DestroyWindow(cover.hwnd) };
        }
    });
}

pub fn hide_all() {
    COVERS.with(|c| {
        for (_, cover) in c.borrow_mut().drain() {
            unsafe { DestroyWindow(cover.hwnd) };
        }
    });
}

pub fn is_shown(id: &str) -> bool {
    COVERS.with(|c| c.borrow().contains_key(id))
}

/// Ids of the screens currently covered.
pub fn shown() -> Vec<String> {
    COVERS.with(|c| c.borrow().keys().cloned().collect())
}

/// Step every running fade. Returns true while any fade is still running.
pub fn animate(now: u64) -> bool {
    COVERS.with(|c| {
        let mut running = false;
        for cover in c.borrow().values() {
            if let Some((start, len)) = cover.fade {
                let t = (now.saturating_sub(start) as f64 / len as f64).clamp(0.0, 1.0);
                // Ease in: dim slowly at first so a glance back catches it early.
                let alpha = (t * t * 250.0) as u8;
                unsafe { SetLayeredWindowAttributes(cover.hwnd, 0, alpha, LWA_ALPHA) };
                running = true;
            }
        }
        running
    })
}

/// Keep the covers above windows that made themselves topmost later.
pub fn raise() {
    COVERS.with(|c| {
        for cover in c.borrow().values() {
            unsafe { SetWindowPos(cover.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
        }
    });
}
