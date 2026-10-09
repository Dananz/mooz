//! Turns the gesture's magnification stream into Ctrl + mouse wheel deltas,
//! the signal Windows apps already treat as zoom.
//!
//! Many apps zoom one step per WM_MOUSEWHEEL regardless of its delta, so the
//! default style only ever sends whole notches and rate-limits them. The
//! smooth style sends fine-grained deltas for apps that honor the size.

use super::config::ZoomStyle;
use super::gesture::Phase;

pub const WHEEL_DELTA: i32 = 120;
/// Magnification that adds up to one wheel notch (one zoom step in most apps).
pub const MAGNIFICATION_PER_NOTCH: f64 = 0.12;
/// Minimum spacing between whole notches in Steps style (about 28 per second).
pub const STEP_INTERVAL: f64 = 0.035;
/// Minimum spacing between fine-grained deltas in Smooth style.
pub const SMOOTH_INTERVAL: f64 = 0.012;
/// Bound on queued-up movement, so a fast flick can't keep zooming afterwards.
/// Three notches fits the largest single wheel event the engine produces.
const MAX_BACKLOG: f64 = 3.0 * WHEEL_DELTA as f64;

#[derive(Debug)]
pub struct WheelMapper {
    pub style: ZoomStyle,
    /// Pending movement in wheel units.
    backlog: f64,
    last_emit: f64,
}

impl WheelMapper {
    pub fn new(style: ZoomStyle) -> Self {
        Self { style, backlog: 0.0, last_emit: f64::NEG_INFINITY }
    }

    /// Feeds one gesture event; returns a wheel delta to send now, if any.
    pub fn feed(&mut self, magnification: f64, phase: Phase, now: f64) -> Option<i32> {
        match phase {
            Phase::Began => {
                self.backlog = 0.0;
                None
            }
            Phase::Changed => {
                self.backlog += magnification * f64::from(WHEEL_DELTA) / MAGNIFICATION_PER_NOTCH;
                self.backlog = self.backlog.clamp(-MAX_BACKLOG, MAX_BACKLOG);
                self.drain(now, false)
            }
            Phase::Ended => {
                // Smooth style flushes what's left; a partial notch is dropped.
                let out = match self.style {
                    ZoomStyle::Smooth => self.drain(now, true),
                    ZoomStyle::Steps => None,
                };
                self.backlog = 0.0;
                out
            }
        }
    }

    /// Sends queued movement between input events (called from the watchdog
    /// timer), so a single large wheel notch still plays out in full.
    pub fn pump(&mut self, now: f64) -> Option<i32> {
        self.drain(now, false)
    }

    fn drain(&mut self, now: f64, force: bool) -> Option<i32> {
        match self.style {
            ZoomStyle::Steps => {
                let notch = f64::from(WHEEL_DELTA);
                // Tolerance so float sums like 0.06 + 0.06 count as a full notch.
                if self.backlog.abs() + 1e-9 < notch || now - self.last_emit < STEP_INTERVAL {
                    return None;
                }
                let sign = self.backlog.signum();
                self.backlog -= sign * notch;
                if self.backlog.abs() < 1e-9 {
                    self.backlog = 0.0;
                }
                self.last_emit = now;
                Some(sign as i32 * WHEEL_DELTA)
            }
            ZoomStyle::Smooth => {
                if self.backlog.abs() < 1.0 || (!force && now - self.last_emit < SMOOTH_INTERVAL) {
                    return None;
                }
                let delta = self.backlog.trunc();
                self.backlog -= delta;
                self.last_emit = now;
                Some(delta as i32)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_emit_whole_notches_only() {
        let mut m = WheelMapper::new(ZoomStyle::Steps);
        m.feed(0.0, Phase::Began, 0.0);
        assert_eq!(m.feed(0.06, Phase::Changed, 0.0), None);
        assert_eq!(m.feed(0.06, Phase::Changed, 0.01), Some(120));
        assert_eq!(m.feed(-0.06, Phase::Changed, 0.1), None, "half a notch back");
        assert_eq!(m.feed(-0.06, Phase::Changed, 0.2), Some(-120));
    }

    #[test]
    fn steps_are_rate_limited_and_backlog_is_bounded() {
        let mut m = WheelMapper::new(ZoomStyle::Steps);
        m.feed(0.0, Phase::Began, 0.0);
        assert_eq!(m.feed(0.15, Phase::Changed, 0.000), Some(120));
        // A burst within the interval queues instead of sending.
        for i in 1..10 {
            assert_eq!(m.feed(0.15, Phase::Changed, 0.001 * f64::from(i)), None);
        }
        // At most three notches survive the burst.
        assert_eq!(m.feed(0.0, Phase::Changed, 0.04), Some(120));
        assert_eq!(m.feed(0.0, Phase::Changed, 0.08), Some(120));
        assert_eq!(m.feed(0.0, Phase::Changed, 0.12), Some(120));
        assert_eq!(m.feed(0.0, Phase::Changed, 0.16), None);
    }

    #[test]
    fn pump_plays_out_queued_notches_without_new_input() {
        let mut m = WheelMapper::new(ZoomStyle::Steps);
        m.feed(0.0, Phase::Began, 0.0);
        // One wheel event worth three notches.
        assert_eq!(m.feed(0.36, Phase::Changed, 0.0), Some(120));
        assert_eq!(m.pump(0.01), None, "rate limited");
        assert_eq!(m.pump(0.05), Some(120));
        assert_eq!(m.pump(0.10), Some(120));
        assert_eq!(m.pump(0.15), None);
    }

    #[test]
    fn steps_drop_partial_notch_on_end() {
        let mut m = WheelMapper::new(ZoomStyle::Steps);
        m.feed(0.0, Phase::Began, 0.0);
        m.feed(0.1, Phase::Changed, 0.0);
        assert_eq!(m.feed(0.0, Phase::Ended, 0.1), None);
        m.feed(0.0, Phase::Began, 0.2);
        assert_eq!(m.feed(0.05, Phase::Changed, 0.3), None, "no carry-over between gestures");
    }

    #[test]
    fn smooth_sends_fine_deltas_and_flushes_on_end() {
        let mut m = WheelMapper::new(ZoomStyle::Smooth);
        m.feed(0.0, Phase::Began, 0.0);
        assert_eq!(m.feed(0.01, Phase::Changed, 0.0), Some(10));
        assert_eq!(m.feed(0.01, Phase::Changed, 0.005), None, "rate limited");
        assert_eq!(m.feed(0.0, Phase::Ended, 0.006), Some(10));
    }

    #[test]
    fn zoom_out_is_negative() {
        let mut m = WheelMapper::new(ZoomStyle::Smooth);
        m.feed(0.0, Phase::Began, 0.0);
        assert!(m.feed(-0.05, Phase::Changed, 0.0).unwrap() < 0);
    }
}
