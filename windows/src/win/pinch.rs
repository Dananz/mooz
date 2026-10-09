//! A gesture-only synthetic precision touchpad (Windows 11). Two fingers
//! injected into it are recognized by Windows as a touchpad pinch at the
//! mouse location: Direct Manipulation apps (Chrome, Edge, Firefox, ...) get
//! a native smooth zoom, and other apps get the same fallback a real
//! touchpad produces. Gesture-only means the input can never turn into
//! cursor motion or clicks.
//!
//! `CreateSyntheticPointerDevice2` is newer than the import libraries, so it
//! is looked up at runtime; when it's missing Mooz falls back to Ctrl + wheel.
//! Parameters and frame layout follow Microsoft's Precision Touchpad
//! Programming Guide injection sample.

use super::log::log;
use super::util::wide;
use crate::core::pinch::{Frame, PAD_HEIGHT, PAD_WIDTH};
use std::time::Instant;
use windows_sys::Win32::Foundation::{GetLastError, POINT};
use windows_sys::Win32::Graphics::Gdi::HMONITOR;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::UI::Controls::{
    DestroySyntheticPointerDevice, HSYNTHETICPOINTERDEVICE, POINTER_FEEDBACK_NONE,
    POINTER_TYPE_INFO, POINTER_TYPE_INFO_0,
};
use windows_sys::Win32::UI::Input::Pointer::{
    InjectSyntheticPointerInput, POINTER_FLAG_CONFIDENCE, POINTER_FLAG_INCONTACT,
    POINTER_FLAG_INRANGE, POINTER_TOUCH_INFO,
};
use windows_sys::Win32::UI::WindowsAndMessaging::PT_TOUCHPAD;

const SDCO_PHYSICAL_SIZE: u32 = 0x1;
const SDCO_TOUCHPAD_GESTURE_ONLY: u32 = 0x2;

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
pub fn available() -> bool {
    create_fn().is_some()
}

pub struct PinchDevice {
    handle: HSYNTHETICPOINTERDEVICE,
    clock: Instant,
    last_time: u32,
}

// The handle is created, used and destroyed on the emit thread only.
impl PinchDevice {
    pub fn create() -> Option<Self> {
        let Some(create) = create_fn() else {
            log("pinch: CreateSyntheticPointerDevice2 not found (needs a newer Windows 11), using Ctrl + wheel");
            return None;
        };
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
        if handle.is_null() {
            log(&format!("pinch: device creation failed, error {}, using Ctrl + wheel", unsafe { GetLastError() }));
            return None;
        }
        log("pinch: virtual touchpad ready");
        Some(Self { handle, clock: Instant::now(), last_time: 0 })
    }

    /// Strictly increasing millisecond timestamps, starting at 1.
    fn next_time(&mut self) -> u32 {
        let elapsed = self.clock.elapsed().as_millis() as u32 + 1;
        self.last_time = elapsed.max(self.last_time + 1);
        self.last_time
    }

    /// Injects one frame (both fingers). Returns false if Windows rejected it.
    pub fn inject(&mut self, frame: Frame) -> bool {
        let flags = match frame {
            Frame::Touch(_) => POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT | POINTER_FLAG_CONFIDENCE,
            Frame::Lift(_) => POINTER_FLAG_CONFIDENCE,
        };
        let time = self.next_time();
        let mut infos = [POINTER_TYPE_INFO::default(); 2];
        for (id, (info, (x, y))) in infos.iter_mut().zip(frame.contacts()).enumerate() {
            let mut touch = POINTER_TOUCH_INFO::default();
            touch.pointerInfo.pointerType = PT_TOUCHPAD;
            touch.pointerInfo.pointerId = id as u32;
            touch.pointerInfo.pointerFlags = flags;
            touch.pointerInfo.ptHimetricLocation = POINT { x, y };
            touch.pointerInfo.dwTime = time;
            *info = POINTER_TYPE_INFO { r#type: PT_TOUCHPAD, Anonymous: POINTER_TYPE_INFO_0 { touchInfo: touch } };
        }
        let ok = unsafe { InjectSyntheticPointerInput(self.handle, infos.as_ptr(), infos.len() as u32) } != 0;
        if !ok {
            log(&format!("pinch: inject {frame:?} failed, error {}", unsafe { GetLastError() }));
        }
        ok
    }
}

impl Drop for PinchDevice {
    fn drop(&mut self) {
        unsafe { DestroySyntheticPointerDevice(self.handle) };
    }
}
