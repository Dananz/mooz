//! The input thread: owns the WH_MOUSE_LL and WH_KEYBOARD_LL hooks, the
//! foreground-app tracker and the gesture engine, and runs their message loop.
//!
//! Windows silently removes a low-level hook whose callback is slow, so the
//! callbacks only run the (cheap) engine and queue wheel deltas; injection
//! happens on the emit thread. The keyboard hook only records physical
//! modifier state for the emitter's stuck-key check and never blocks keys.
//! A health check reinstalls the hooks if the cursor moves without the mouse
//! hook hearing about it.

use super::apps::exe_for_window;
use super::emit::{self, SIGNATURE};
use super::output::{self, Command};
use super::log::{self, log};
use super::util::now_seconds;
use crate::core::applist::should_intercept;
use crate::core::config::{config_path, ConfigWatcher};
use crate::core::gesture::{Context, Emitter, Engine, GestureSettings, Phase, Verdict};
use crate::core::modifiers::{physically_held, Modifiers};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread::JoinHandle;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU,
    VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetCursorPos, GetForegroundWindow, GetMessageW, SetCursorPos,
    KillTimer, PostThreadMessageW, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx,
    EVENT_SYSTEM_FOREGROUND, HHOOK, KBDLLHOOKSTRUCT, LLKHF_UP, MSG, MSLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WINEVENT_OUTOFCONTEXT, WM_APP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_QUIT, WM_TIMER,
};

/// Drain queued wheel deltas (posted by the hook callback).
const WM_APP_FLUSH: u32 = WM_APP + 1;
/// Re-read config.json now (posted by the tray after a toggle).
pub const WM_APP_RELOAD: u32 = WM_APP + 2;

/// Watchdog period. The engine ends a gesture after 0.2 s without input.
const WATCHDOG_MS: u32 = 50;
/// Config reload and hook health check period.
const MAINTENANCE_MS: u32 = 500;
/// Silence after which a moving cursor means the hook was dropped.
const HOOK_SILENCE_SECS: f64 = 1.0;

static THREAD_ID: AtomicU32 = AtomicU32::new(0);

pub struct HookThread {
    handle: JoinHandle<()>,
}

impl HookThread {
    pub fn stop(self) {
        post(WM_QUIT);
        let _ = self.handle.join();
    }
}

pub fn spawn() -> HookThread {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let handle = std::thread::Builder::new()
        .name("mooz-input".into())
        .spawn(move || {
            THREAD_ID.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
            let _ = ready_tx.send(());
            run_loop();
        })
        .expect("spawn input thread");
    let _ = ready_rx.recv();
    HookThread { handle }
}

/// Posts a message to the input thread (no-op before it starts).
pub fn post(message: u32) {
    let tid = THREAD_ID.load(Ordering::SeqCst);
    if tid != 0 {
        unsafe { PostThreadMessageW(tid, message, 0, 0) };
    }
}

struct State {
    engine: Engine,
    watcher: ConfigWatcher,
    settings: GestureSettings,
    foreground_exe: Option<String>,
    intercept: bool,
    mouse_hook: HHOOK,
    keyboard_hook: HHOOK,
    pending: VecDeque<(Phase, f64)>,
    stats: GestureStats,
    flush_posted: bool,
    last_hook_event: f64,
    last_hook_point: (i32, i32),
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Collects wheel deltas while the engine runs inside the hook callback.
struct Queue<'a> {
    pending: &'a mut VecDeque<(Phase, f64)>,
}

impl Emitter for Queue<'_> {
    fn emit(&mut self, magnification: f64, phase: Phase, _now: f64) {
        self.pending.push_back((phase, magnification));
    }
}

/// Per-gesture diagnostics for the debug log: is the cursor really pinned?
#[derive(Default)]
struct GestureStats {
    moves: u32,
    swallowed: u32,
    /// Times the cursor was found off the anchor and put back.
    repinned: u32,
    max_drift: i32,
}

impl State {
    fn new() -> Self {
        let watcher = ConfigWatcher::new(config_path());
        let settings = GestureSettings::from(&watcher.config);
        let mut state = Self {
            engine: Engine::new(),
            watcher,
            settings,
            foreground_exe: None,
            intercept: true,
            mouse_hook: std::ptr::null_mut(),
            keyboard_hook: std::ptr::null_mut(),
            pending: VecDeque::new(),
            stats: GestureStats::default(),
            flush_posted: false,
            last_hook_event: now_seconds(),
            last_hook_point: cursor_pos(),
        };
        state.engine.set_cursor(state.last_hook_point);
        state.foreground_exe = exe_for_window(unsafe { GetForegroundWindow() });
        state.apply_config();
        state
    }

    fn context(&self) -> Context {
        let mut held_keys = [0; 4];
        for shortcut in &self.settings.shortcuts {
            if let Some(key) = shortcut.key {
                if key_down(key) { held_keys[usize::from(key / 64)] |= 1 << (key % 64); }
            }
        }
        Context { held: held_modifiers(), held_keys, intercept: self.intercept, now: now_seconds() }
    }

    fn end_gesture(&mut self) {
        let now = now_seconds();
        let mut queue = Queue { pending: &mut self.pending };
        self.engine.end(now, &mut queue);
    }

    fn update_intercept(&mut self) {
        let c = &self.watcher.config;
        self.intercept = !self.foreground_exe.as_deref().is_some_and(|exe| exe.eq_ignore_ascii_case("Mooz.exe"))
            && should_intercept(c.list_mode, &c.apps, self.foreground_exe.as_deref());
    }

    /// Pushes config values into the engine and installs or removes the hook.
    fn apply_config(&mut self) {
        log::set_enabled(self.watcher.config.debug);
        log(&format!(
            "config: enabled={} trigger={} source={:?} sensitivity={} anchor={} method={:?} style={:?} foreground={:?}",
            self.watcher.config.enabled,
            self.watcher.config.shortcut_label(),
            self.watcher.config.input_source,
            self.watcher.config.sensitivity,
            self.watcher.config.anchor_cursor,
            self.watcher.config.zoom_method,
            self.watcher.config.zoom_style,
            self.foreground_exe,
        ));
        self.settings = GestureSettings::from(&self.watcher.config);
        output::send(Command::Configure {
            method: self.watcher.config.zoom_method,
            style: self.watcher.config.zoom_style,
        });
        self.update_intercept();
        if self.watcher.config.enabled {
            self.install_hook();
        } else {
            self.end_gesture();
            self.remove_hook();
        }
    }

    fn install_hook(&mut self) {
        if !self.mouse_hook.is_null() {
            return;
        }
        unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            self.mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0);
            if self.keyboard_hook.is_null() {
                self.keyboard_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0);
            }
        }
        log(&format!(
            "hooks installed: mouse={} keyboard={}",
            !self.mouse_hook.is_null(),
            !self.keyboard_hook.is_null()
        ));
        self.last_hook_event = now_seconds();
        self.last_hook_point = cursor_pos();
        self.engine.set_cursor(self.last_hook_point);
    }

    fn remove_hook(&mut self) {
        unsafe {
            if !self.mouse_hook.is_null() {
                UnhookWindowsHookEx(self.mouse_hook);
            }
            if !self.keyboard_hook.is_null() {
                UnhookWindowsHookEx(self.keyboard_hook);
            }
        }
        self.mouse_hook = std::ptr::null_mut();
        self.keyboard_hook = std::ptr::null_mut();
    }

    /// Safety net for anchoring: swallowing the move should already keep the
    /// cursor still, but if anything moved it (another hook, a driver), put it
    /// back. SetCursorPos isn't input, so the hook doesn't see it.
    fn keep_pinned(&mut self) {
        if !self.engine.is_active() || !self.settings.anchor_cursor {
            return;
        }
        let (x, y) = cursor_pos();
        let (ax, ay) = self.engine.anchor();
        let drift = (x - ax).abs().max((y - ay).abs());
        if drift > 0 {
            self.stats.max_drift = self.stats.max_drift.max(drift);
            self.stats.repinned += 1;
            unsafe { SetCursorPos(ax, ay) };
        }
    }

    /// Windows drops a hook that times out without telling us. If the cursor
    /// moved while the hook heard nothing for a while, reinstall it. Also
    /// retries an install that failed outright.
    fn check_hook_health(&mut self) {
        if !self.watcher.config.enabled {
            return;
        }
        if self.mouse_hook.is_null() {
            self.install_hook();
            return;
        }
        if self.engine.is_active() {
            return;
        }
        let pos = cursor_pos();
        if pos != self.last_hook_point && now_seconds() - self.last_hook_event > HOOK_SILENCE_SECS {
            log("mouse hook went silent, reinstalling");
            self.remove_hook();
            self.install_hook();
        }
    }
}

fn run_loop() {
    STATE.with(|s| *s.borrow_mut() = Some(State::new()));

    let win_event = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            std::ptr::null_mut(),
            Some(foreground_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    let watchdog = unsafe { SetTimer(std::ptr::null_mut(), 0, WATCHDOG_MS, None) };
    let maintenance = unsafe { SetTimer(std::ptr::null_mut(), 0, MAINTENANCE_MS, None) };

    let mut was_active = false;
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
        match msg.message {
            WM_TIMER if msg.wParam == watchdog => with_state(|s| {
                emit::release_stuck_keys();
                // Right after a tick the lifted keys can still read as up.
                if emit::sent_recently() {
                    return;
                }
                let ctx = s.context();
                let mut queue = Queue { pending: &mut s.pending };
                s.engine.tick(ctx, &s.settings, &mut queue);
            }),
            WM_TIMER if msg.wParam == maintenance => with_state(|s| {
                if s.watcher.refresh() {
                    s.apply_config();
                }
                s.check_hook_health();
            }),
            WM_APP_RELOAD => with_state(|s| {
                if s.watcher.refresh() {
                    s.apply_config();
                }
            }),
            WM_APP_FLUSH => with_state(|s| s.flush_posted = false),
            _ => {}
        }
        flush();
        with_state(|s| {
            s.keep_pinned();
            let active = s.engine.is_active();
            if active != was_active {
                was_active = active;
                let g = &s.stats;
                log(&format!(
                    "gesture {} held={} foreground={:?} intercept={} moves={} swallowed={} repinned={} max_drift={}px",
                    if active { "began" } else { "ended" },
                    held_modifiers().label(),
                    s.foreground_exe,
                    s.intercept,
                    g.moves,
                    g.swallowed,
                    g.repinned,
                    g.max_drift,
                ));
                if active {
                    s.stats = GestureStats::default();
                }
            }
        });
    }

    unsafe {
        KillTimer(std::ptr::null_mut(), watchdog);
        KillTimer(std::ptr::null_mut(), maintenance);
        if !win_event.is_null() {
            UnhookWinEvent(win_event);
        }
    }
    with_state(|s| {
        s.end_gesture();
        s.remove_hook();
    });
    flush();
}

fn with_state(f: impl FnOnce(&mut State)) {
    STATE.with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut() {
            if let Some(state) = guard.as_mut() {
                f(state);
            }
        }
    });
}

/// Hands queued gesture events to the output thread.
fn flush() {
    let mut pending = VecDeque::new();
    with_state(|s| std::mem::swap(&mut pending, &mut s.pending));
    for (phase, magnification) in pending {
        output::send(Command::Gesture(phase, magnification));
    }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
    }
    let info = &*(lparam as *const MSLLHOOKSTRUCT);
    let message = wparam as u32;
    if info.dwExtraInfo == SIGNATURE || (message != WM_MOUSEMOVE && message != WM_MOUSEWHEEL) {
        return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
    }

    let pt = (info.pt.x, info.pt.y);
    let mut verdict = Verdict::Pass;
    STATE.with(|cell| {
        let Ok(mut guard) = cell.try_borrow_mut() else { return };
        let Some(s) = guard.as_mut() else { return };
        s.last_hook_event = now_seconds();
        let ctx = s.context();
        let settings = &s.settings;
        let was_active = s.engine.is_active();
        let mut queue = Queue { pending: &mut s.pending };
        verdict = if message == WM_MOUSEMOVE {
            s.engine.on_move(pt, ctx, settings, &mut queue)
        } else {
            // The high word of mouseData is the signed wheel delta.
            let delta = i32::from((info.mouseData >> 16) as u16 as i16);
            s.engine.on_wheel(pt, delta, ctx, settings, &mut queue)
        };
        s.last_hook_point = if verdict == Verdict::Swallow { s.engine.anchor() } else { pt };
        if message == WM_MOUSEMOVE && (was_active || s.engine.is_active()) {
            s.stats.moves += 1;
            if verdict == Verdict::Swallow {
                s.stats.swallowed += 1;
            }
        }
        // Wake the loop for swallowed moves too, so it can check the pin.
        if (!s.pending.is_empty() || verdict == Verdict::Swallow) && !s.flush_posted {
            s.flush_posted = true;
            post(WM_APP_FLUSH);
        }
    });

    match verdict {
        // Nonzero stops the event: the cursor stays put / the wheel is eaten.
        Verdict::Swallow => 1,
        Verdict::Pass => CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam),
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam as *const KBDLLHOOKSTRUCT);
        if info.dwExtraInfo != SIGNATURE {
            emit::note_physical_key(info.vkCode, info.flags & LLKHF_UP == 0);
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

unsafe extern "system" fn foreground_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    let exe = exe_for_window(hwnd);
    with_state(|s| {
        if s.foreground_exe != exe {
            log(&format!("foreground: {exe:?}"));
        }
        s.foreground_exe = exe;
        s.update_intercept();
    });
}

fn key_down(vk: u16) -> bool {
    unsafe { GetAsyncKeyState(i32::from(vk)) as u16 & 0x8000 != 0 }
}

/// The modifiers held right now: the OS async state, overruled by the
/// keyboard hook's physical view (see `physically_held`).
fn held_modifiers() -> Modifiers {
    let held = |generic: bool, left: u16, right: u16| {
        physically_held(generic, emit::physical(left), emit::physical(right))
    };
    let mut m = Modifiers::NONE;
    if held(key_down(VK_CONTROL), VK_LCONTROL, VK_RCONTROL) {
        m = m | Modifiers::CTRL;
    }
    if held(key_down(VK_MENU), VK_LMENU, VK_RMENU) {
        m = m | Modifiers::ALT;
    }
    if held(key_down(VK_SHIFT), VK_LSHIFT, VK_RSHIFT) {
        m = m | Modifiers::SHIFT;
    }
    if held(key_down(VK_LWIN) || key_down(VK_RWIN), VK_LWIN, VK_RWIN) {
        m = m | Modifiers::WIN;
    }
    m
}

fn cursor_pos() -> (i32, i32) {
    let mut p = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut p) };
    (p.x, p.y)
}
