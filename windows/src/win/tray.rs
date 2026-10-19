//! The notification-area icon and its menu, the Windows counterpart of the
//! macOS menu bar extra. Runs the main thread's message loop.

use super::hook::{self, WM_APP_RELOAD};
use super::icon::tray_icon;
use super::util::{copy_wide, wide, wide_ptr_to_string};
use crate::core::config::{config_path, ConfigWatcher, InputSource};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIIF_NONE,
    NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NINF_KEY, NIN_SELECT, NOTIFYICONDATAW,
    NOTIFYICON_VERSION_4,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, GetMessageW, GetSystemMetrics, PostMessageW,
    PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow,
    SetMenuDefaultItem, SetTimer, TrackPopupMenuEx, TranslateMessage, HICON, MF_CHECKED,
    MF_GRAYED, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, MSG, SM_MENUDROPALIGNMENT,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTALIGN,
    TPM_RIGHTBUTTON, WM_APP, WM_CONTEXTMENU, WM_DESTROY, WM_NULL, WM_SETTINGCHANGE, WM_TIMER,
    WNDCLASSW, WS_OVERLAPPED,
};

const WM_APP_TRAY: u32 = WM_APP + 10;
/// Enter/Space on the focused icon (NIN_SELECT | NINF_KEY).
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;
const TRAY_ID: u32 = 1;
const CONFIG_TIMER: usize = 1;

const CMD_HEADER: usize = 1;
const CMD_ENABLED: usize = 2;
const CMD_SETTINGS: usize = 3;
const CMD_QUIT: usize = 4;

const VERSION: &str = env!("MOOZ_VERSION");

/// Explorer broadcasts this after it restarts; the icon must be re-added.
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

struct Tray {
    hwnd: HWND,
    watcher: ConfigWatcher,
    icon: HICON,
    light_taskbar: bool,
    updater: crate::updates::Updater,
    next_update_check: Instant,
    notified_version: Option<String>,
}

thread_local! {
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

fn with_tray<R>(f: impl FnOnce(&mut Tray) -> R) -> Option<R> {
    TRAY.with(|cell| cell.try_borrow_mut().ok().and_then(|mut t| t.as_mut().map(f)))
}

pub fn run() {
    let class = wide("MoozTray");
    let title = wide("Mooz");
    let hwnd = unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(window_proc);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        RegisterClassW(&wc);
        // A hidden top-level window (not message-only, which would miss the
        // TaskbarCreated broadcast).
        CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return;
    }
    let taskbar_created = wide("TaskbarCreated");
    TASKBAR_CREATED.store(unsafe { RegisterWindowMessageW(taskbar_created.as_ptr()) }, Ordering::SeqCst);

    let mut tray = Tray {
        hwnd,
        watcher: ConfigWatcher::new(config_path()),
        icon: std::ptr::null_mut(),
        light_taskbar: light_taskbar(),
        updater: crate::updates::Updater::new(),
        next_update_check: Instant::now() + Duration::from_secs(30),
        notified_version: None,
    };
    tray.reload_icon();
    tray.add();
    tray.welcome_once();
    TRAY.with(|cell| *cell.borrow_mut() = Some(tray));

    // Picks up changes made in Settings (e.g. Enable Mooz) for the icon state.
    unsafe { SetTimer(hwnd, CONFIG_TIMER, 1000, None) };

    let mut msg: MSG = unsafe { std::mem::zeroed() };
    while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    with_tray(|t| {
        t.delete();
        if !t.icon.is_null() {
            unsafe { DestroyIcon(t.icon) };
        }
    });
}

impl Tray {
    fn check_updates(&mut self) {
        self.updater.poll();
        if self.watcher.config.check_updates && Instant::now() >= self.next_update_check {
            self.next_update_check = Instant::now() + Duration::from_secs(24 * 60 * 60);
            self.updater.check();
        }
        if let crate::updates::Status::Available(version) = &self.updater.status {
            if self.notified_version.as_ref() != Some(version) && self.watcher.config.check_updates {
                let mut nid = self.data();
                nid.uFlags = NIF_INFO;
                nid.dwInfoFlags = NIIF_NONE;
                copy_wide(&mut nid.szInfoTitle, "Mooz update available");
                copy_wide(&mut nid.szInfo, &format!("Mooz {version} is available. Open Settings, then Updates to install it."));
                unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
                self.notified_version = Some(version.clone());
            }
        }
    }

    fn data(&self) -> NOTIFYICONDATAW {
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.hwnd;
        nid.uID = TRAY_ID;
        nid
    }

    fn enabled(&self) -> bool {
        self.watcher.config.enabled
    }

    fn tooltip(&self) -> String {
        if self.enabled() { "Mooz".into() } else { "Mooz (paused)".into() }
    }

    fn add(&self) {
        let mut nid = self.data();
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
        nid.uCallbackMessage = WM_APP_TRAY;
        nid.hIcon = self.icon;
        copy_wide(&mut nid.szTip, &self.tooltip());
        unsafe {
            Shell_NotifyIconW(NIM_ADD, &nid);
            nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &nid);
        }
    }

    fn update(&self) {
        let mut nid = self.data();
        nid.uFlags = NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        nid.hIcon = self.icon;
        copy_wide(&mut nid.szTip, &self.tooltip());
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
    }

    fn delete(&self) {
        let nid = self.data();
        unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
    }

    fn reload_icon(&mut self) {
        let old = self.icon;
        self.icon = tray_icon(self.light_taskbar, !self.enabled());
        if !old.is_null() {
            unsafe { DestroyIcon(old) };
        }
    }

    /// First run only: say where Mooz lives and how to use it.
    fn welcome_once(&mut self) {
        if self.watcher.config.onboarded {
            return;
        }
        let c = &self.watcher.config;
        let how = match c.input_source {
            InputSource::MouseDrag => "move the mouse up or down",
            InputSource::ScrollWheel => "scroll",
        };
        let text = format!(
            "Hold {} and {how} to zoom. Click this icon for settings.",
            c.modifiers.label()
        );
        let mut nid = self.data();
        nid.uFlags = NIF_INFO;
        nid.dwInfoFlags = NIIF_NONE;
        copy_wide(&mut nid.szInfoTitle, "Mooz is running");
        copy_wide(&mut nid.szInfo, &text);
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
        self.watcher.config.onboarded = true;
        let _ = self.watcher.save();
    }

    fn set_enabled(&mut self, on: bool) {
        self.watcher.refresh();
        self.watcher.config.enabled = on;
        let _ = self.watcher.save();
        hook::post(WM_APP_RELOAD);
        self.reload_icon();
        self.update();
    }
}

fn show_menu(hwnd: HWND, x: i32, y: i32) {
    let Some(enabled) = with_tray(|t| {
        t.watcher.refresh();
        t.enabled()
    }) else {
        return;
    };
    let header = wide(&format!("Mooz {VERSION}"));
    let enabled_label = wide("Enabled");
    let settings = wide("Settings…");
    let quit = wide("Quit Mooz");
    let command = unsafe {
        let menu = CreatePopupMenu();
        AppendMenuW(menu, MF_STRING | MF_GRAYED, CMD_HEADER, header.as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        let check = if enabled { MF_CHECKED } else { MF_UNCHECKED };
        AppendMenuW(menu, MF_STRING | check, CMD_ENABLED, enabled_label.as_ptr());
        AppendMenuW(menu, MF_STRING, CMD_SETTINGS, settings.as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, CMD_QUIT, quit.as_ptr());
        // Bold, since a left click on the icon does the same.
        SetMenuDefaultItem(menu, CMD_SETTINGS as u32, 0);

        let align = if GetSystemMetrics(SM_MENUDROPALIGNMENT) != 0 { TPM_RIGHTALIGN } else { TPM_LEFTALIGN };
        // Documented requirement for notification-icon menus: be foreground
        // first, then post a benign message afterwards.
        SetForegroundWindow(hwnd);
        let command = TrackPopupMenuEx(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | align,
            x,
            y,
            hwnd,
            std::ptr::null(),
        );
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
        command as usize
    };

    match command {
        CMD_ENABLED => {
            with_tray(|t| t.set_enabled(!enabled));
        }
        CMD_SETTINGS => super::open_settings(),
        CMD_QUIT => unsafe {
            DestroyWindow(hwnd);
        },
        _ => {}
    }
}

/// Windows' "app mode" applies to apps, "system mode" to the taskbar.
fn light_taskbar() -> bool {
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let value = wide("SystemUsesLightTheme");
    let mut data = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut size,
        )
    };
    status == 0 && data != 0
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let taskbar_created = TASKBAR_CREATED.load(Ordering::SeqCst);
    match msg {
        WM_APP_TRAY => {
            // NOTIFYICON_VERSION_4: event in LOWORD(lParam), anchor point in wParam.
            let event = (lparam as u32) & 0xFFFF;
            let x = (wparam & 0xFFFF) as u16 as i16 as i32;
            let y = ((wparam >> 16) & 0xFFFF) as u16 as i16 as i32;
            match event {
                NIN_SELECT | NIN_KEYSELECT => super::open_settings(),
                WM_CONTEXTMENU => show_menu(hwnd, x, y),
                _ => {}
            }
            0
        }
        WM_TIMER if wparam == CONFIG_TIMER => {
            with_tray(|t| {
                if t.watcher.refresh() {
                    t.reload_icon();
                    t.update();
                }
                t.check_updates();
            });
            0
        }
        WM_SETTINGCHANGE => {
            // "ImmersiveColorSet" arrives when the light/dark theme changes.
            if lparam != 0 && wide_ptr_to_string(lparam as *const u16, 64) == "ImmersiveColorSet" {
                with_tray(|t| {
                    t.light_taskbar = light_taskbar();
                    t.reload_icon();
                    t.update();
                });
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ if taskbar_created != 0 && msg == taskbar_created => {
            with_tray(|t| t.add());
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
