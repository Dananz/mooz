//! The Windows shell around the core: a low-level mouse hook on its own
//! thread, Ctrl + wheel injection, and a notification-area (tray) icon.

pub mod apps;
pub mod autostart;
mod emit;
mod hook;
mod icon;
mod log;
mod output;
pub mod pinch;
pub mod shell;
mod tray;
mod util;

use crate::settings_ui::WINDOW_TITLE;
use util::wide;
use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, FindWindowW, IsIconic, MessageBoxW, SetForegroundWindow, ShowWindow,
    ASFW_ANY, MB_ICONWARNING, MB_OK, SW_RESTORE,
};

/// Single-instance guard, the counterpart of the macOS AppDelegate check.
const INSTANCE_MUTEX: &str = "Local\\com.dananz.mooz";

/// Entry point of the tray app.
pub fn run() {
    let name = wide(INSTANCE_MUTEX);
    // The handle is intentionally never closed: it lives as long as the process.
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if mutex.is_null() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        // Already running: launching again opens Settings instead.
        open_settings();
        return;
    }

    let hook_thread = hook::spawn();
    tray::run();
    hook_thread.stop();
}

/// Focuses the Settings window when one is open. Returns false when none is.
pub fn focus_settings() -> bool {
    let title = wide(WINDOW_TITLE);
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        return false;
    }
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd);
    }
    true
}

/// Opens Settings: focuses the existing window or starts `Mooz.exe --settings`.
pub fn open_settings() {
    if focus_settings() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    // Let the new process take the foreground, which this one (the target of
    // the user's click) is allowed to grant.
    unsafe { AllowSetForegroundWindow(ASFW_ANY) };
    let _ = std::process::Command::new(exe).arg("--settings").spawn();
}

pub fn show_error(text: &str) {
    let (text, title) = (wide(text), wide("Mooz"));
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONWARNING) };
}
