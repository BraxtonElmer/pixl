/// NUL-terminated UTF-16 for Win32 `W` functions.
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// UTF-16 buffer up to its first NUL.
pub fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// Copy `s` into a fixed-size UTF-16 field, truncating and NUL-terminating.
pub fn fill_wide(dst: &mut [u16], s: &str) {
    let mut n = 0;
    let max = dst.len().saturating_sub(1);
    for (d, c) in dst.iter_mut().zip(s.encode_utf16()).take(max) {
        *d = c;
        n += 1;
    }
    dst[n] = 0;
}
