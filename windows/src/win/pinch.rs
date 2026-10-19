//! The virtual pinch. Preferred: a gesture-only synthetic precision touchpad
//! (Windows 11). Two fingers injected into it are recognized by Windows as a
//! touchpad pinch at the mouse location: Direct Manipulation apps (Chrome,
//! Edge, Firefox, ...) get a native smooth zoom, and other apps get the same
//! fallback a real touchpad produces. Gesture-only means the input can never
//! turn into cursor motion or clicks.
//!
//! Where that's missing (Windows 10, older Windows 11), the two fingers go on
//! the touchscreen instead (`InjectTouchInput`, Windows 8+), placed on screen
//! around the cursor. Browsers treat it as a touch pinch: the same smooth zoom
//! at the cursor. Apps without touch support get Windows' legacy translation
//! of the zoom gesture (Ctrl + wheel). The fingers carry no visual feedback,
//! and the cursor is put back where it was when they lift. (A touchscreen from
//! `CreateSyntheticPointerDevice` would be the obvious choice, but with
//! several monitors its contacts land far from the requested pixels; touch
//! injection maps them exactly on every monitor.)
//!
//! `CreateSyntheticPointerDevice2` is newer than the import libraries, so it
//! is looked up at runtime; when neither device can be made Mooz falls back to
//! Ctrl + wheel. Parameters and frame layout follow Microsoft's Precision
//! Touchpad Programming Guide injection sample.

use super::log::log;
use super::util::wide;
use crate::core::pinch::{Frame, Range, PAD_HEIGHT, PAD_WIDTH};
use std::time::Instant;
use windows_sys::Win32::Foundation::{GetLastError, POINT};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::Controls::{
    DestroySyntheticPointerDevice, HSYNTHETICPOINTERDEVICE, POINTER_FEEDBACK_NONE, POINTER_TYPE_INFO,
    POINTER_TYPE_INFO_0,
};
use windows_sys::Win32::UI::Input::Pointer::{
    InitializeTouchInjection, InjectSyntheticPointerInput, InjectTouchInput, POINTER_FLAGS,
    POINTER_FLAG_CONFIDENCE, POINTER_FLAG_DOWN, POINTER_FLAG_INCONTACT, POINTER_FLAG_INRANGE,
    POINTER_FLAG_UP, POINTER_FLAG_UPDATE, POINTER_TOUCH_INFO, TOUCH_FEEDBACK_NONE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, SetCursorPos, WindowFromPoint, PT_TOUCH, PT_TOUCHPAD,
};

const SDCO_PHYSICAL_SIZE: u32 = 0x1;
const SDCO_TOUCHPAD_GESTURE_ONLY: u32 = 0x2;

/// Touchscreen pinch span, in pixels at 100% scaling. Chromium doesn't scale
/// a touch pinch whose fingers are closer than 125 DIPs
/// (`min_scaling_span_in_pixels`), so they never get that close.
const TOUCH_MIN_SPAN: f64 = 200.0;
const TOUCH_MAX_SPAN: f64 = 1_000.0;
/// Gap kept between a finger and the monitor edge.
const EDGE_GAP: i32 = 4;

/// SYNTHETIC_DEVICE_CREATION_PARAMS, fields in documented order.
#[repr(C)]
struct CreationParams {
    pointer_type: i32,
    max_count: u32,
    feedback_mode: i32,
    monitor: HMONITOR,
    device_width: u32,
    device_height: u32,
    options: u32,
}

const _: () = assert!(std::mem::size_of::<CreationParams>() == 40);
const _: () = assert!(std::mem::size_of::<POINTER_TYPE_INFO>() == 152);

type CreateFn = unsafe extern "system" fn(*const CreationParams) -> HSYNTHETICPOINTERDEVICE;

fn create_fn() -> Option<CreateFn> {
    let user32 = wide("user32.dll");
    unsafe {
        let module = GetModuleHandleW(user32.as_ptr());
        if module.is_null() {
            return None;
        }
        let proc = GetProcAddress(module, c"CreateSyntheticPointerDevice2".as_ptr().cast())?;
        Some(std::mem::transmute::<unsafe extern "system" fn() -> isize, CreateFn>(proc))
    }
}

/// Whether this Windows build has touchpad injection.
pub fn touchpad_available() -> bool {
    create_fn().is_some()
}

/// Sets up touch injection for this process (two contacts, no visual feedback).
fn init_touch_injection() -> bool {
    unsafe { InitializeTouchInjection(2, TOUCH_FEEDBACK_NONE) != 0 }
}

/// Whether some virtual pinch (touchpad, or touchscreen as the fallback) works.
pub fn available() -> bool {
    touchpad_available() || init_touch_injection()
}

enum Kind {
    Touchpad,
    Touchscreen {
        /// Where the cursor was when the gesture began; restored on lift.
        cursor: POINT,
        /// The fingers' centroid, kept far enough from the monitor edges.
        center: POINT,
        /// Finger distances, in pixels, that fit around `center`.
        range: Range,
        down: bool,
    },
}

pub struct PinchDevice {
    /// The synthetic touchpad; null for the touchscreen, which has no device.
    handle: HSYNTHETICPOINTERDEVICE,
    kind: Kind,
    clock: Instant,
    last_time: u32,
}

// The handle is created, used and destroyed on the emit thread only.
impl PinchDevice {
    pub fn create() -> Option<Self> {
        match create_fn() {
            Some(create) => {
                let params = CreationParams {
                    pointer_type: PT_TOUCHPAD,
                    max_count: 2,
                    feedback_mode: POINTER_FEEDBACK_NONE,
                    monitor: std::ptr::null_mut(),
                    device_width: PAD_WIDTH,
                    device_height: PAD_HEIGHT,
                    options: SDCO_PHYSICAL_SIZE | SDCO_TOUCHPAD_GESTURE_ONLY,
                };
                let handle = unsafe { create(&params) };
                if !handle.is_null() {
                    log("pinch: virtual touchpad ready");
                    return Some(Self::new(handle, Kind::Touchpad));
                }
                log(&format!("pinch: touchpad creation failed, error {}", unsafe { GetLastError() }));
            }
            None => log("pinch: CreateSyntheticPointerDevice2 not found (needs a newer Windows 11)"),
        }
        if !init_touch_injection() {
            log(&format!("pinch: touch injection unavailable, error {}, using Ctrl + wheel", unsafe {
                GetLastError()
            }));
            return None;
        }
        log("pinch: touch injection ready");
        let origin = POINT { x: 0, y: 0 };
        let range = Range::between(TOUCH_MIN_SPAN, TOUCH_MAX_SPAN);
        Some(Self::new(std::ptr::null_mut(), Kind::Touchscreen { cursor: origin, center: origin, range, down: false }))
    }

    fn new(handle: HSYNTHETICPOINTERDEVICE, kind: Kind) -> Self {
        Self { handle, kind, clock: Instant::now(), last_time: 0 }
    }

    /// Call when a gesture begins: the touchscreen pinch centers on the cursor.
    /// Returns the finger distances to pinch between, in the device's units.
    pub fn start(&mut self) -> Range {
        match &mut self.kind {
            Kind::Touchpad => Range::PAD,
            Kind::Touchscreen { cursor, center, range, .. } => {
                let mut pt = POINT { x: 0, y: 0 };
                unsafe { GetCursorPos(&mut pt) };
                *cursor = pt;
                (*center, *range) = touch_layout(pt);
                let target = super::apps::exe_for_window(unsafe { WindowFromPoint(*center) });
                log(&format!(
                    "pinch: touch at ({}, {}), span {:.0}..{:.0} px, cursor ({}, {}), over {target:?}",
                    center.x, center.y, range.min, range.max, pt.x, pt.y
                ));
                *range
            }
        }
    }

    /// Strictly increasing millisecond timestamps, starting at 1.
    fn next_time(&mut self) -> u32 {
        let elapsed = self.clock.elapsed().as_millis() as u32 + 1;
        self.last_time = elapsed.max(self.last_time + 1);
        self.last_time
    }

    /// Injects one frame (both fingers). Returns false if Windows rejected it.
    pub fn inject(&mut self, frame: Frame) -> bool {
        let (pointer_type, flags, points, restore) = match &mut self.kind {
            Kind::Touchpad => {
                let flags = match frame {
                    Frame::Touch(_) => POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT | POINTER_FLAG_CONFIDENCE,
                    Frame::Lift(_) => POINTER_FLAG_CONFIDENCE,
                };
                (PT_TOUCHPAD, flags, frame.contacts().map(|(x, y)| POINT { x, y }), None)
            }
            Kind::Touchscreen { cursor, center, down, .. } => {
                let (flags, restore): (POINTER_FLAGS, _) = match frame {
                    Frame::Touch(_) if *down => (POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT | POINTER_FLAG_UPDATE, None),
                    Frame::Touch(_) => (POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT | POINTER_FLAG_DOWN, None),
                    // Nothing to lift (a lift is only ever sent for fingers that are down).
                    Frame::Lift(_) if !*down => return true,
                    Frame::Lift(_) => (POINTER_FLAG_UP, Some(*cursor)),
                };
                *down = matches!(frame, Frame::Touch(_));
                // The distance is in pixels here; split it evenly about the center.
                let half = frame.distance() / 2.0;
                let (left, right) = ((center.x as f64 - half).round() as i32, (center.x as f64 + half).round() as i32);
                let points = [POINT { x: left, y: center.y }, POINT { x: right, y: center.y }];
                (PT_TOUCH, flags, points, restore)
            }
        };
        // The touchscreen lets Windows stamp the time; the touchpad needs our own.
        let time = if pointer_type == PT_TOUCHPAD { self.next_time() } else { 0 };
        let mut touches = [POINTER_TOUCH_INFO::default(); 2];
        for (id, (touch, point)) in touches.iter_mut().zip(points).enumerate() {
            touch.pointerInfo.pointerType = pointer_type;
            touch.pointerInfo.pointerId = id as u32;
            touch.pointerInfo.pointerFlags = flags;
            if pointer_type == PT_TOUCHPAD {
                touch.pointerInfo.ptHimetricLocation = point;
            } else {
                touch.pointerInfo.ptPixelLocation = point;
            }
            touch.pointerInfo.dwTime = time;
        }
        let ok = if pointer_type == PT_TOUCHPAD {
            let infos = touches.map(|touch| POINTER_TYPE_INFO {
                r#type: PT_TOUCHPAD,
                Anonymous: POINTER_TYPE_INFO_0 { touchInfo: touch },
            });
            unsafe { InjectSyntheticPointerInput(self.handle, infos.as_ptr(), infos.len() as u32) != 0 }
        } else {
            unsafe { InjectTouchInput(touches.len() as u32, touches.as_ptr()) != 0 }
        };
        if !ok {
            log(&format!("pinch: inject {frame:?} failed, error {}", unsafe { GetLastError() }));
        }
        if let Some(cursor) = restore {
            unsafe { SetCursorPos(cursor.x, cursor.y) };
        }
        ok
    }
}

/// Where the touchscreen fingers go for a pinch at `pt`: centered on it, with
/// as wide a span as fits on its monitor (scaled for its DPI). Near an edge
/// the center moves inward just enough to keep the minimum span.
fn touch_layout(pt: POINT) -> (POINT, Range) {
    let fallback = (pt, Range::between(TOUCH_MIN_SPAN, TOUCH_MAX_SPAN));
    let monitor = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return fallback;
    }
    let (mut dpi_x, mut dpi_y) = (96, 96);
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    let scale = dpi_x.max(96) as f64 / 96.0;
    let min = TOUCH_MIN_SPAN * scale;
    let r = info.rcMonitor;
    let (lo, hi) = (r.left + EDGE_GAP, r.right - 1 - EDGE_GAP);
    let reach = (min / 2.0).ceil() as i32;
    if hi - lo < 2 * reach {
        return fallback;
    }
    let x = pt.x.clamp(lo + reach, hi - reach);
    let room = 2 * (x - lo).min(hi - x);
    let center = POINT { x, y: pt.y.clamp(r.top + 1, r.bottom - 2) };
    (center, Range::between(min, (TOUCH_MAX_SPAN * scale).min(room as f64)))
}

impl Drop for PinchDevice {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { DestroySyntheticPointerDevice(self.handle) };
        }
    }
}
