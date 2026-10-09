use std::time::Instant;

/// NUL-terminated UTF-16 for Win32 `W` functions.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Copies `s` into a fixed UTF-16 buffer, truncating and NUL-terminating.
pub fn copy_wide(dst: &mut [u16], s: &str) {
    let mut n = 0;
    for (slot, unit) in dst.iter_mut().zip(s.encode_utf16()) {
        *slot = unit;
        n += 1;
    }
    let end = n.min(dst.len().saturating_sub(1));
    dst[end] = 0;
}

pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Reads a NUL-terminated UTF-16 string from a raw pointer, stopping at `max`
/// units so a missing terminator can't run off the end.
///
/// # Safety
/// `ptr` must point to readable memory up to its terminator or `max` units.
pub unsafe fn wide_ptr_to_string(ptr: *const u16, max: usize) -> String {
    let mut len = 0;
    while len < max && *ptr.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

/// Monotonic seconds since process start, the clock the core expects.
pub fn now_seconds() -> f64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}
