//! An optional log of what Pixl decides, for checking its timing on a real
//! PC: set `PIXL_LOG` to a file path before starting it. Off otherwise, and
//! capped at 4 MB so it can be left running for days.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

const MAX_BYTES: u64 = 4 * 1024 * 1024;

static FILE: Mutex<Option<File>> = Mutex::new(None);

pub fn init() {
    let Some(path) = std::env::var_os("PIXL_LOG") else { return };
    if let Ok(f) = OpenOptions::new().create(true).append(true).open(path) {
        *FILE.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
    }
}

pub fn enabled() -> bool {
    FILE.lock().map(|f| f.is_some()).unwrap_or(false)
}

/// Append one line, stamped with the tick count in seconds.
pub fn line(msg: impl AsRef<str>) {
    let mut guard = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(f) = guard.as_mut() else { return };
    if f.metadata().map_or(true, |m| m.len() > MAX_BYTES) {
        *guard = None;
        return;
    }
    let t = crate::input::now() as f64 / 1000.0;
    let _ = writeln!(f, "{t:.3} {}", msg.as_ref());
}
