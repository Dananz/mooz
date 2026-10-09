//! The Ctrl + mouse wheel zoom method (the fallback when the virtual touchpad
//! pinch isn't available, or when chosen in Settings). Runs on the output
//! thread so the hook thread never blocks in SendInput.
//!
//! The app must see Ctrl *alone*: Firefox picks its wheel action from the
//! exact modifier set (Ctrl + Shift + wheel scrolls), and many native apps
//! test `wParam == MK_CONTROL`. So each tick briefly lifts any held Shift, Alt
//! or Win and puts them back:
//!
//!   mask tap, lift Shift/Alt/Win, [Ctrl down], wheel, [Ctrl up], re-press, mask tap
//!
//! The mask tap is an unassigned key (0xE8). Pressing it while a modifier is
//! held stops Alt from focusing the menu bar, Win from opening Start, and
//! Shift presses from counting toward the Sticky Keys shortcut. Ctrl is only
//! pressed after the others are up, so a Ctrl + Shift layout-switch hotkey
//! can't fire. The batch goes in as one SendInput call, so physical input
//! can't interleave with it.
//!
//! A key is re-pressed only if it is down when the batch is built. If the
//! user lets go in the instant before the batch lands, the re-press would
//! leave it stuck down; `release_stuck_keys` (run by the watchdog) sees the
//! physical release reported by the keyboard hook and lifts it.

use super::log::log;
use crate::core::modifiers::{side_action, KeySide, SideAction};
use super::util::now_seconds;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, VK_CONTROL,
    VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN,
};

/// Tags every event Mooz injects (`dwExtraInfo`), so the hooks can skip its
/// own input without also skipping other injected input such as remote desktop.
pub const SIGNATURE: usize = 0x4D4F_4F5A; // "MOOZ"

/// Documented as unassigned in the virtual-key table.
const MASK_KEY: u16 = 0xE8;

/// The side-specific modifier keys the low-level keyboard hook reports.
const KEYS: [u16; 8] =
    [VK_LSHIFT, VK_RSHIFT, VK_LCONTROL, VK_RCONTROL, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN];
/// The ones lifted around a tick (everything but Ctrl).
const LIFTED: [u16; 6] = [VK_LSHIFT, VK_RSHIFT, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN];

const UNKNOWN: u8 = 0;
const DOWN: u8 = 1;
const UP: u8 = 2;

/// Last state of each key from input that wasn't ours, per the keyboard hook.
static PHYSICAL: [AtomicU8; 8] = [const { AtomicU8::new(UNKNOWN) }; 8];
/// Keys our last batch pressed back down.
static REPRESSED: [AtomicBool; 8] = [const { AtomicBool::new(false) }; 8];
/// When the last batch was sent, in microseconds of `now_seconds`.
static LAST_SEND_US: AtomicU64 = AtomicU64::new(0);

fn index(vk: u16) -> Option<usize> {
    KEYS.iter().position(|k| *k == vk)
}

/// The keyboard hook's view of one side-specific key.
pub fn physical(vk: u16) -> KeySide {
    match index(vk).map(|i| PHYSICAL[i].load(Ordering::SeqCst)) {
        Some(DOWN) => KeySide::Down,
        Some(UP) => KeySide::Up,
        _ => KeySide::Unknown,
    }
}

/// Called by the keyboard hook for every key event that Mooz didn't inject.
pub fn note_physical_key(vk: u32, down: bool) {
    let Some(i) = u16::try_from(vk).ok().and_then(index) else { return };
    PHYSICAL[i].store(if down { DOWN } else { UP }, Ordering::SeqCst);
    if down {
        REPRESSED[i].store(false, Ordering::SeqCst);
    }
}

/// True for a short while after a batch, when the async key state may still
/// show a lifted key as up. The watchdog doesn't trust a "released" reading then.
pub fn sent_recently() -> bool {
    let last = LAST_SEND_US.load(Ordering::SeqCst) as f64 / 1e6;
    last > 0.0 && now_seconds() - last < 0.05
}

fn key_down(vk: u16) -> bool {
    unsafe { GetAsyncKeyState(i32::from(vk)) as u16 & 0x8000 != 0 }
}

fn is_extended(vk: u16) -> bool {
    matches!(vk, VK_RCONTROL | VK_RMENU | VK_LWIN | VK_RWIN)
}

fn key(vk: u16, up: bool) -> INPUT {
    let mut flags = if up { KEYEVENTF_KEYUP } else { 0 };
    if is_extended(vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: SIGNATURE },
        },
    }
}

fn wheel(delta: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                // A signed delta stored in the DWORD field, as the docs specify.
                mouseData: delta as _,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: SIGNATURE,
            },
        },
    }
}

fn send(inputs: &[INPUT]) -> u32 {
    unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) }
}

/// Sends one Ctrl + wheel zoom tick (called from the output thread).
pub fn send_zoom(delta: i32) {
    let ctrl_held = key_down(VK_LCONTROL) || key_down(VK_RCONTROL);
    let mut lift = Vec::new();
    let mut stuck = Vec::new();
    for vk in LIFTED {
        match side_action(key_down(vk), physical(vk)) {
            SideAction::LiftAndRestore => lift.push(vk),
            SideAction::Release => stuck.push(vk),
            SideAction::Leave => {}
        }
    }

    let mut inputs = Vec::with_capacity(4 + 2 * lift.len() + stuck.len() + 3);
    // Keys down only because of an earlier re-press go up for good.
    inputs.extend(stuck.iter().map(|vk| key(*vk, true)));
    for vk in &stuck {
        if let Some(i) = index(*vk) {
            REPRESSED[i].store(false, Ordering::SeqCst);
        }
    }
    if !lift.is_empty() {
        inputs.push(key(MASK_KEY, false));
        inputs.push(key(MASK_KEY, true));
        inputs.extend(lift.iter().map(|vk| key(*vk, true)));
    }
    if !ctrl_held {
        inputs.push(key(VK_CONTROL, false));
    }
    inputs.push(wheel(delta));
    if !ctrl_held {
        inputs.push(key(VK_CONTROL, true));
    }
    if !lift.is_empty() {
        inputs.extend(lift.iter().map(|vk| key(*vk, false)));
        inputs.push(key(MASK_KEY, false));
        inputs.push(key(MASK_KEY, true));
    }

    for vk in &lift {
        if let Some(i) = index(*vk) {
            REPRESSED[i].store(true, Ordering::SeqCst);
        }
    }
    let sent = send(&inputs);
    LAST_SEND_US.store((now_seconds() * 1e6) as u64, Ordering::SeqCst);
    let names = |keys: &[u16]| keys.iter().map(|vk| format!("{vk:#04x}")).collect::<Vec<_>>();
    log(&format!(
        "zoom delta={delta} ctrl_injected={} lifted={:?} released={:?} sent={sent}/{}",
        !ctrl_held,
        names(&lift),
        names(&stuck),
        inputs.len()
    ));
}

/// Lifts any key our re-press left down after the user physically let go.
pub fn release_stuck_keys() {
    for (i, vk) in KEYS.iter().enumerate() {
        if !REPRESSED[i].load(Ordering::SeqCst) {
            continue;
        }
        if !key_down(*vk) {
            REPRESSED[i].store(false, Ordering::SeqCst);
        } else if PHYSICAL[i].load(Ordering::SeqCst) == UP && !sent_recently() {
            REPRESSED[i].store(false, Ordering::SeqCst);
            send(&[key(*vk, true)]);
            log(&format!("released stuck key {vk:#04x}"));
        }
    }
}
