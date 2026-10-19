//! The few OS services the settings window needs. On Windows they call into
//! `win`; elsewhere they are stand-ins so the window can be previewed on any
//! machine (`cargo run`).

#[cfg(windows)]
pub use crate::win::apps::running_apps;
#[cfg(windows)]
pub use crate::win::autostart::{is_enabled as autostart_enabled, set as set_autostart};
#[cfg(windows)]
pub use crate::win::pinch::available as native_pinch_available;
#[cfg(windows)]
pub use crate::win::shell::{open_url, reveal};

#[cfg(not(windows))]
pub use preview::*;

#[cfg(not(windows))]
mod preview {
    use crate::core::config::AppEntry;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};

    static AUTOSTART: AtomicBool = AtomicBool::new(false);

    /// The preview pretends the virtual touchpad exists.
    pub fn native_pinch_available() -> bool {
        true
    }

    pub fn autostart_enabled() -> bool {
        AUTOSTART.load(Ordering::Relaxed)
    }

    pub fn set_autostart(on: bool) -> std::io::Result<()> {
        AUTOSTART.store(on, Ordering::Relaxed);
        Ok(())
    }

    pub fn running_apps() -> Vec<AppEntry> {
        [("chrome.exe", "Google Chrome"), ("firefox.exe", "Firefox"), ("figma.exe", "Figma")]
            .into_iter()
            .map(|(exe, name)| AppEntry { exe: exe.into(), name: name.into() })
            .collect()
    }

    pub fn open_url(url: &str) {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }

    pub fn reveal(path: &Path) {
        let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    }
}
