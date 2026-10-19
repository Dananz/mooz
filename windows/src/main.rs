//! Mooz for Windows: trackpad pinch-to-zoom for any mouse.
//!
//! `Mooz.exe` runs the tray app (input hook + tray icon). `Mooz.exe --settings`
//! runs the Settings window as a separate process. On other platforms only the
//! Settings window builds, as a preview.

#![cfg_attr(windows, windows_subsystem = "windows")]
// The core is only partly used by the non-Windows settings preview.
#![cfg_attr(not(windows), allow(dead_code))]

mod core;
mod platform;
mod settings_ui;
mod updates;
#[cfg(windows)]
mod win;

fn main() {
    #[cfg(windows)]
    velopack::VelopackApp::build()
        .set_auto_apply_on_startup(false)
        .on_first_run(|_| win::open_settings())
        .on_after_update_fast_callback(|_| {
            if win::autostart::is_enabled() {
                let _ = win::autostart::set(true);
            }
        })
        .on_before_uninstall_fast_callback(|_| {
            let _ = win::autostart::set(false);
        })
        .run();
    let settings = std::env::args().any(|a| a == "--settings");
    #[cfg(windows)]
    if !settings {
        win::run();
        return;
    }
    let _ = settings;
    // One Settings window at a time.
    #[cfg(windows)]
    if win::focus_settings() {
        return;
    }
    if let Err(e) = settings_ui::run() {
        let message = format!(
            "The Settings window couldn't open ({e}).\n\nMooz keeps running from the tray. You can edit the settings file directly:\n{}",
            core::config::config_path().display()
        );
        #[cfg(windows)]
        win::show_error(&message);
        #[cfg(not(windows))]
        eprintln!("{message}");
    }
}
