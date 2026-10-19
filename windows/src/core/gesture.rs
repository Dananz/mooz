//! The gesture state machine, ported from the macOS `GestureInterceptor`
//! without any OS calls so it runs under `cargo test` anywhere. The Win32
//! layer feeds it hook events and the current modifier state, and obeys the
//! returned verdict (swallow the event or let it through).
//!
//! Windows differs from macOS in one useful way: swallowing a WM_MOUSEMOVE in
//! a low-level hook keeps the cursor where it is, so anchoring needs no warp.
//! The hook's point is where the cursor *would* go, so while anchored the
//! movement is `pt - anchor`.

use super::config::{Config, InputSource};
use super::modifiers::Modifiers;

pub type Point = (i32, i32);

/// Smoothing weight of the newest raw delta (the rest is the running value).
const SMOOTHING_NEW: f64 = 0.35;
/// Magnification per pixel of vertical movement, before sensitivity.
const MAGNIFICATION_PER_PIXEL: f64 = 0.01;
/// Per-event clamp so one large movement can't jump the zoom.
const MAX_MAGNIFICATION_PER_EVENT: f64 = 0.15;
/// Magnification per wheel notch (WHEEL_DELTA = 120), before sensitivity.
/// At the default sensitivity one notch in equals one zoom step out.
pub const MAGNIFICATION_PER_WHEEL_NOTCH: f64 = 0.24;
/// Wheel events get a higher clamp than drags: a notch is one deliberate
/// event, and the drag clamp (0.15) would leave the slider no range above ~0.6.
const MAX_WHEEL_MAGNIFICATION_PER_EVENT: f64 = 0.36;
/// A gesture with no input for this long is ended by the watchdog.
pub const STALL_TIMEOUT: f64 = 0.2;
const MIN_DRAG_MAGNIFICATION: f64 = 0.0002;
const MIN_WHEEL_MAGNIFICATION: f64 = 0.0005;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Began,
    Changed,
    Ended,
}

pub trait Emitter {
    fn emit(&mut self, magnification: f64, phase: Phase, now: f64);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Swallow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GestureSettings {
    pub required: Modifiers,
    pub input_source: InputSource,
    pub sensitivity: f64,
    pub anchor_cursor: bool,
}

impl From<&Config> for GestureSettings {
    fn from(c: &Config) -> Self {
        Self {
            required: c.modifiers,
            input_source: c.input_source,
            sensitivity: c.sensitivity,
            anchor_cursor: c.anchor_cursor,
        }
    }
}

/// Per-event inputs the OS layer supplies.
#[derive(Clone, Copy, Debug)]
pub struct Context {
    /// Modifiers held right now.
    pub held: Modifiers,
    /// Blocklist/allowlist verdict for the foreground app.
    pub intercept: bool,
    /// Monotonic seconds.
    pub now: f64,
}

#[derive(Debug, Default)]
pub struct Engine {
    active: bool,
    smoothed_dy: f64,
    last_input_time: f64,
    anchor: Point,
    /// Where the cursor is, as far as the hook has seen.
    cursor: Option<Point>,
}

fn clamp_magnification(value: f64) -> f64 {
    value.clamp(-MAX_MAGNIFICATION_PER_EVENT, MAX_MAGNIFICATION_PER_EVENT)
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn anchor(&self) -> Point {
        self.anchor
    }

    /// Seeds the known cursor position (e.g. from GetCursorPos at startup).
    pub fn set_cursor(&mut self, pt: Point) {
        self.cursor = Some(pt);
    }

    fn begin(&mut self, at: Point, now: f64, emitter: &mut dyn Emitter) {
        self.active = true;
        self.anchor = at;
        self.smoothed_dy = 0.0;
        self.last_input_time = now;
        emitter.emit(0.0, Phase::Began, now);
    }

    /// Ends the gesture, if one is running.
    pub fn end(&mut self, now: f64, emitter: &mut dyn Emitter) {
        if !self.active {
            return;
        }
        self.active = false;
        self.smoothed_dy = 0.0;
        emitter.emit(0.0, Phase::Ended, now);
    }

    /// A mouse move (also fires while a button is held). `pt` is where the
    /// cursor would move to.
    pub fn on_move(
        &mut self,
        pt: Point,
        ctx: Context,
        s: &GestureSettings,
        emitter: &mut dyn Emitter,
    ) -> Verdict {
        let previous = self.cursor.unwrap_or(pt);

        if !s.required.satisfied_by(ctx.held) || !ctx.intercept {
            self.end(ctx.now, emitter);
            self.cursor = Some(pt);
            return Verdict::Pass;
        }

        // In scroll mode a move only exists to keep the cursor anchored.
        // With anchoring off there is nothing to do: let it move.
        if s.input_source != InputSource::MouseDrag && !s.anchor_cursor {
            self.cursor = Some(pt);
            return Verdict::Pass;
        }

        if !self.active {
            self.begin(previous, ctx.now, emitter);
        }
        self.last_input_time = ctx.now;

        if s.input_source == InputSource::MouseDrag {
            // While anchored the cursor never left the anchor, so the whole
            // offset is this event's movement.
            let origin = if s.anchor_cursor { self.anchor } else { previous };
            let raw_dy = f64::from(pt.1 - origin.1);
            self.smoothed_dy = SMOOTHING_NEW * raw_dy + (1.0 - SMOOTHING_NEW) * self.smoothed_dy;
            // Moving up (negative dy) zooms in.
            let magnification =
                clamp_magnification(-self.smoothed_dy * s.sensitivity * MAGNIFICATION_PER_PIXEL);
            if magnification.abs() > MIN_DRAG_MAGNIFICATION {
                emitter.emit(magnification, Phase::Changed, ctx.now);
            }
        }

        if s.anchor_cursor {
            self.cursor = Some(self.anchor);
            Verdict::Swallow
        } else {
            self.cursor = Some(pt);
            Verdict::Pass
        }
    }

    /// A vertical wheel event. `delta` is in WHEEL_DELTA units (120 per
    /// notch), positive away from the user.
    pub fn on_wheel(
        &mut self,
        pt: Point,
        delta: i32,
        ctx: Context,
        s: &GestureSettings,
        emitter: &mut dyn Emitter,
    ) -> Verdict {
        if !s.required.satisfied_by(ctx.held) || !ctx.intercept {
            self.end(ctx.now, emitter);
            return Verdict::Pass;
        }
        if s.input_source != InputSource::ScrollWheel {
            return Verdict::Pass;
        }
        if !self.active {
            self.begin(self.cursor.unwrap_or(pt), ctx.now, emitter);
        }
        self.last_input_time = ctx.now;
        let magnification = (f64::from(delta) / 120.0 * MAGNIFICATION_PER_WHEEL_NOTCH * s.sensitivity)
            .clamp(-MAX_WHEEL_MAGNIFICATION_PER_EVENT, MAX_WHEEL_MAGNIFICATION_PER_EVENT);
        if magnification.abs() > MIN_WHEEL_MAGNIFICATION {
            emitter.emit(magnification, Phase::Changed, ctx.now);
        }
        Verdict::Swallow
    }

    /// Periodic watchdog. Ends the gesture when the combo is no longer held
    /// (a missed key-up can't strand it) or input has stalled.
    pub fn tick(&mut self, ctx: Context, s: &GestureSettings, emitter: &mut dyn Emitter) {
        if !self.active {
            return;
        }
        if !s.required.satisfied_by(ctx.held) || ctx.now - self.last_input_time > STALL_TIMEOUT {
            self.end(ctx.now, emitter);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder(Vec<(f64, Phase)>);
    impl Emitter for Recorder {
        fn emit(&mut self, magnification: f64, phase: Phase, _now: f64) {
            self.0.push((magnification, phase));
        }
    }
    impl Recorder {
        fn phases(&self) -> Vec<Phase> {
            self.0.iter().map(|e| e.1).collect()
        }
        fn total(&self) -> f64 {
            self.0.iter().map(|e| e.0).sum()
        }
    }

    fn drag() -> GestureSettings {
        GestureSettings {
            required: Modifiers::SHIFT,
            input_source: InputSource::MouseDrag,
            sensitivity: 0.5,
            anchor_cursor: true,
        }
    }

    fn held(now: f64) -> Context {
        Context { held: Modifiers::SHIFT, intercept: true, now }
    }

    fn released(now: f64) -> Context {
        Context { held: Modifiers::NONE, intercept: true, now }
    }

    #[test]
    fn moves_pass_through_without_modifier() {
        let mut e = Engine::new();
        let mut r = Recorder::default();
        assert_eq!(e.on_move((10, 10), released(0.0), &drag(), &mut r), Verdict::Pass);
        assert!(r.0.is_empty());
        assert!(!e.is_active());
    }

    #[test]
    fn gesture_begins_anchors_and_swallows_while_held() {
        let mut e = Engine::new();
        let mut r = Recorder::default();
        e.set_cursor((100, 100));
        assert_eq!(e.on_move((100, 90), held(0.0), &drag(), &mut r), Verdict::Swallow);
        assert!(e.is_active());
        assert_eq!(e.anchor(), (100, 100), "anchors where the cursor was, not where it was going");
        assert_eq!(r.phases(), vec![Phase::Began, Phase::Changed]);
    }

    #[test]
    fn moving_up_zooms_in_and_down_zooms_out() {
        let mut e = Engine::new();
        let mut r = Recorder::default();
        e.set_cursor((0, 0));
        for i in 0..10 {
            e.on_move((0, -5), held(f64::from(i) * 0.01), &drag(), &mut r);
        }
        assert!(r.total() > 0.0);

        let mut e = Engine::new();
        let mut r = Recorder::default();
        e.set_cursor((0, 0));
        for i in 0..10 {
            e.on_move((0, 5), held(f64::from(i) * 0.01), &drag(), &mut r);
        }
        assert!(r.total() < 0.0);
    }

    #[test]
    fn anchored_delta_is_measured_from_the_anchor() {
        // Cursor is pinned at the anchor, so a steady 4px/event drag reports
        // pt = anchor + 4 every time and the smoothed delta converges on 4.
        let mut e = Engine::new();
        let mut r = Recorder::default();
        e.set_cursor((50, 50));
        for i in 0..60 {
            e.on_move((50, 54), held(f64::from(i) * 0.008), &drag(), &mut r);
        }
        let last = r.0.last().unwrap().0;
        assert!((last - (-4.0 * 0.5 * 0.01)).abs() < 1e-4, "last={last}");
    }

    #[test]
    fn per_event_magnification_is_clamped() {
        let mut e = Engine::new();
        let mut r = Recorder::default();
        let s = GestureSettings { sensitivity: 3.0, ..drag() };
        e.set_cursor((0, 0));
        for i in 0..20 {
            e.on_move((0, -2000), held(f64::from(i) * 0.01), &s, &mut r);
        }
        assert!(r.0.iter().all(|(m, _)| m.abs() <= MAX_MAGNIFICATION_PER_EVENT + 1e-12));
    }

    #[test]
    fn release_on_next_move_ends_gesture() {
        let mut e = Engine::new();
        let mut r = Recorder::default();
        e.on_move((0, 0), held(0.0), &drag(), &mut r);
        assert_eq!(e.on_move((0, 5), released(0.01), &drag(), &mut r), Verdict::Pass);
        assert!(!e.is_active());
        assert_eq!(r.phases().last(), Some(&Phase::Ended));
    }

    #[test]
    fn no_anchor_lets_cursor_move_but_still_zooms() {
        let mut e = Engine::new();
        let mut r = Recorder::default();
        let s = GestureSettings { anchor_cursor: false, ..drag() };
        e.set_cursor((0, 0));
        assert_eq!(e.on_move((0, -10), held(0.0), &s, &mut r), Verdict::Pass);
        assert_eq!(e.on_move((0, -20), held(0.01), &s, &mut r), Verdict::Pass);
        assert!(r.total() > 0.0);
    }

    #[test]
    fn scroll_mode_anchors_on_move_without_zooming() {
        let mut e = Engine::new();
        let mut r = Recorder::default();
        let s = GestureSettings { input_source: InputSource::ScrollWheel, ..drag() };
        assert_eq!(e.on_move((0, 50), held(0.0), &s, &mut r), Verdict::Swallow);
        assert_eq!(r.phases(), vec![Phase::Began]);
    }

    #[test]
    fn scroll_mode_without_anchor_passes_moves() {
        let mut e = Engine::new();
        let mut r = Recorder::default();
        let s = GestureSettings { input_source: InputSource::ScrollWheel, anchor_cursor: false, ..drag() };
        assert_eq!(e.on_move((0, 50), held(0.0), &s, &mut r), Verdict::Pass);
        assert!(r.0.is_empty());
    }

    #[test]
    fn wheel_zooms_in_scroll_mode_only() {
        let s = GestureSettings { input_source: InputSource::ScrollWheel, ..drag() };
        let mut e = Engine::new();
        let mut r = Recorder::default();
        assert_eq!(e.on_wheel((0, 0), 120, held(0.0), &s, &mut r), Verdict::Swallow);
        assert_eq!(r.phases(), vec![Phase::Began, Phase::Changed]);
        assert!((r.total() - 0.12).abs() < 1e-9, "one notch at default sensitivity = 0.12");

        let mut e = Engine::new();
        let mut r = Recorder::default();
        assert_eq!(e.on_wheel((0, 0), 120, held(0.0), &drag(), &mut r), Verdict::Pass);
        assert!(r.0.is_empty());
    }

    #[test]
    fn wheel_sensitivity_has_range() {
        let notch = |sensitivity: f64| {
            let s = GestureSettings { input_source: InputSource::ScrollWheel, sensitivity, ..drag() };
            let mut e = Engine::new();
            let mut r = Recorder::default();
            e.on_wheel((0, 0), 120, held(0.0), &s, &mut r);
            r.total()
        };
        assert!(notch(1.0) > notch(0.5));
        assert!(notch(1.5) > notch(1.0));
        assert!(notch(3.0) <= MAX_WHEEL_MAGNIFICATION_PER_EVENT + 1e-12);
    }

    #[test]
    fn wheel_without_modifier_passes() {
        let s = GestureSettings { input_source: InputSource::ScrollWheel, ..drag() };
        let mut e = Engine::new();
        let mut r = Recorder::default();
        assert_eq!(e.on_wheel((0, 0), -120, released(0.0), &s, &mut r), Verdict::Pass);
    }

    #[test]
    fn blocked_app_passes_and_ends() {
        let mut e = Engine::new();
        let mut r = Recorder::default();
        e.on_move((0, 0), held(0.0), &drag(), &mut r);
        let ctx = Context { intercept: false, ..held(0.01) };
        assert_eq!(e.on_move((0, 5), ctx, &drag(), &mut r), Verdict::Pass);
        assert!(!e.is_active());
    }

    #[test]
    fn watchdog_ends_on_release_even_while_moving() {
        let mut e = Engine::new();
        let mut r = Recorder::default();
        e.on_move((0, 0), held(0.0), &drag(), &mut r);
        e.tick(released(0.05), &drag(), &mut r);
        assert!(!e.is_active());
        assert_eq!(r.phases().last(), Some(&Phase::Ended));
    }

    #[test]
    fn watchdog_ends_stalled_gesture() {
        let mut e = Engine::new();
        let mut r = Recorder::default();
        e.on_move((0, 0), held(0.0), &drag(), &mut r);
        e.tick(held(0.1), &drag(), &mut r);
        assert!(e.is_active());
        e.tick(held(0.25), &drag(), &mut r);
        assert!(!e.is_active());
    }

    #[test]
    fn ended_is_emitted_exactly_once() {
        let mut e = Engine::new();
        let mut r = Recorder::default();
        e.on_move((0, 0), held(0.0), &drag(), &mut r);
        e.end(0.1, &mut r);
        e.end(0.2, &mut r);
        e.tick(released(0.3), &drag(), &mut r);
        assert_eq!(r.phases().iter().filter(|p| **p == Phase::Ended).count(), 1);
    }

    #[test]
    fn soak_repeated_gestures_never_stick() {
        let mut e = Engine::new();
        let mut r = Recorder::default();
        let mut now = 0.0;
        for round in 0..200 {
            for _ in 0..5 {
                now += 0.008;
                e.on_move((0, if round % 2 == 0 { 3 } else { -3 }), held(now), &drag(), &mut r);
            }
            now += 0.05;
            if round % 3 == 0 {
                e.tick(released(now), &drag(), &mut r);
            } else {
                e.on_move((0, 1), released(now), &drag(), &mut r);
            }
            assert!(!e.is_active(), "round {round} stuck");
        }
        let began = r.phases().iter().filter(|p| **p == Phase::Began).count();
        let ended = r.phases().iter().filter(|p| **p == Phase::Ended).count();
        assert_eq!(began, ended);
    }
}
