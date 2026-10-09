//! The output thread: turns the engine's gesture stream into zoom. With the
//! Pinch method it drives the virtual touchpad (paced at ~120 Hz, with
//! keep-alive frames while the fingers are down, like a real touchpad's
//! continuous reports); with the Wheel method, or when the touchpad isn't
//! available, it sends Ctrl + wheel ticks.

use super::emit;
use super::log::log;
use super::pinch::PinchDevice;
use super::util::now_seconds;
use crate::core::config::{ZoomMethod, ZoomStyle};
use crate::core::gesture::Phase;
use crate::core::pinch::Pinch;
use crate::core::wheel::WheelMapper;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// Minimum spacing between pinch frames.
const FRAME_INTERVAL: f64 = 0.008;
/// Longest gap between frames while the fingers are down.
const KEEPALIVE_INTERVAL: f64 = 0.030;

pub enum Command {
    Gesture(Phase, f64),
    Configure { method: ZoomMethod, style: ZoomStyle },
}

pub fn send(command: Command) {
    static SENDER: OnceLock<Mutex<Sender<Command>>> = OnceLock::new();
    let sender = SENDER.get_or_init(|| {
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("mooz-output".into())
            .spawn(move || Output::new().run(rx))
            .expect("spawn output thread");
        Mutex::new(tx)
    });
    if let Ok(tx) = sender.lock() {
        let _ = tx.send(command);
    }
}

struct Output {
    method: ZoomMethod,
    device: Option<PinchDevice>,
    /// Set after an injection failure: Ctrl + wheel for the rest of the session.
    pinch_broken: bool,
    /// Whether the current gesture goes through the touchpad.
    pinching: bool,
    active: bool,
    pinch: Pinch,
    /// Magnification not yet injected, as a scale factor.
    pending: f64,
    last_frame: f64,
    frames: u32,
    regrips: u32,
    mapper: WheelMapper,
}

impl Output {
    fn new() -> Self {
        Self {
            method: ZoomMethod::Pinch,
            device: None,
            pinch_broken: false,
            pinching: false,
            active: false,
            pinch: Pinch::new(),
            pending: 1.0,
            last_frame: 0.0,
            frames: 0,
            regrips: 0,
            mapper: WheelMapper::new(ZoomStyle::Steps),
        }
    }

    fn run(mut self, rx: Receiver<Command>) {
        loop {
            let command = if self.active {
                match rx.recv_timeout(Duration::from_secs_f64(FRAME_INTERVAL)) {
                    Ok(c) => Some(c),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match rx.recv() {
                    Ok(c) => Some(c),
                    Err(_) => break,
                }
            };
            match command {
                Some(Command::Configure { method, style }) => self.configure(method, style),
                Some(Command::Gesture(phase, magnification)) => self.gesture(phase, magnification),
                None => {}
            }
            self.tick();
        }
    }

    fn configure(&mut self, method: ZoomMethod, style: ZoomStyle) {
        self.method = method;
        self.mapper.style = style;
        if method == ZoomMethod::Pinch && self.device.is_none() && !self.pinch_broken {
            self.device = PinchDevice::create();
            // Don't retry (and re-log) on every settings change.
            self.pinch_broken = self.device.is_none();
        }
    }

    fn gesture(&mut self, phase: Phase, magnification: f64) {
        let now = now_seconds();
        match phase {
            Phase::Began => {
                self.active = true;
                self.pinching = self.method == ZoomMethod::Pinch && self.device.is_some();
                if self.pinching {
                    self.pending = 1.0;
                    self.frames = 0;
                    self.regrips = 0;
                    let frames = self.pinch.begin();
                    self.inject(frames, now);
                } else {
                    self.wheel(magnification, phase, now);
                }
            }
            Phase::Changed if self.pinching => {
                self.pending *= 1.0 + magnification;
            }
            Phase::Changed => self.wheel(magnification, phase, now),
            Phase::Ended => {
                if self.pinching {
                    self.flush_pinch(now);
                    let frames = self.pinch.end();
                    self.inject(frames, now);
                    log(&format!("pinch ended: frames={} regrips={}", self.frames, self.regrips));
                } else {
                    self.wheel(magnification, phase, now);
                }
                self.active = false;
                self.pinching = false;
            }
        }
    }

    fn tick(&mut self) {
        if !self.active {
            return;
        }
        let now = now_seconds();
        if self.pinching {
            if now - self.last_frame < FRAME_INTERVAL {
                return;
            }
            if (self.pending - 1.0).abs() > 1e-6 {
                self.flush_pinch(now);
            } else if now - self.last_frame >= KEEPALIVE_INTERVAL {
                if let Some(frame) = self.pinch.current() {
                    self.inject(vec![frame], now);
                }
            }
        } else if let Some(delta) = self.mapper.pump(now) {
            emit::send_zoom(delta);
        }
    }

    fn flush_pinch(&mut self, now: f64) {
        if (self.pending - 1.0).abs() <= 1e-6 {
            return;
        }
        let frames = self.pinch.change(self.pending - 1.0);
        self.pending = 1.0;
        if frames.len() > 1 {
            self.regrips += 1;
        }
        self.inject(frames, now);
    }

    fn inject(&mut self, frames: Vec<crate::core::pinch::Frame>, now: f64) {
        let Some(device) = self.device.as_mut() else { return };
        for frame in frames {
            if !device.inject(frame) {
                // Don't keep failing every frame: switch to Ctrl + wheel.
                self.device = None;
                self.pinch_broken = true;
                self.pinching = false;
                log("pinch: disabled for this session after an injection failure");
                return;
            }
            self.frames += 1;
        }
        self.last_frame = now;
    }

    fn wheel(&mut self, magnification: f64, phase: Phase, now: f64) {
        if let Some(delta) = self.mapper.feed(magnification, phase, now) {
            emit::send_zoom(delta);
        }
    }
}
