//! Opt-in diagnostics, off by default: set `"debug": true` in config.json and
//! Mooz appends to %TEMP%\mooz-debug.log. Never called from the hook
//! callbacks themselves, where disk I/O could get the hook removed.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static ENABLED: AtomicBool = AtomicBool::new(false);
static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

pub fn set_enabled(on: bool) {
    if ENABLED.swap(on, Ordering::SeqCst) != on && on {
        log(&format!("debug log on, Mooz {}", env!("MOOZ_VERSION")));
    }
}

pub fn path() -> std::path::PathBuf {
    std::env::temp_dir().join("mooz-debug.log")
}

pub fn log(message: &str) {
    if !ENABLED.load(Ordering::SeqCst) {
        return;
    }
    let Ok(mut guard) = FILE.lock() else { return };
    if guard.is_none() {
        *guard = std::fs::OpenOptions::new().create(true).append(true).open(path()).ok();
    }
    if let Some(file) = guard.as_mut() {
        let secs = super::util::now_seconds();
        let _ = writeln!(file, "{secs:10.3} {message}");
    }
}
