//! Finger geometry for the synthetic pinch. Two contacts sit symmetric about
//! a center on a horizontal line, so their centroid never moves (a pure zoom,
//! no pan). Each magnification step scales their distance by `1 + m`, the same
//! multiplicative meaning as a macOS magnify event. A pinch starts at the end
//! of the range that leaves the most room for its first direction (narrow to
//! zoom in, wide to zoom out). When the fingers would leave the range, they
//! lift and re-grip at the far end for the current direction; the app sees a
//! new pinch and continues.

/// Pad size in himetric (0.01 mm), as in Microsoft's injection sample.
pub const PAD_WIDTH: u32 = 10_000;
pub const PAD_HEIGHT: u32 = 6_000;
const CENTER_X: f64 = PAD_WIDTH as f64 / 2.0;
const CENTER_Y: f64 = PAD_HEIGHT as f64 / 2.0;

/// The distances a pinch moves between, in the device's own units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    /// Closest and farthest the two fingers get before re-gripping.
    pub min: f64,
    pub max: f64,
    /// Where a grip lands: near-closed to zoom in, wide to zoom out.
    pub grip_in: f64,
    pub grip_out: f64,
}

impl Range {
    /// The virtual touchpad, in pad himetric.
    pub const PAD: Range = Range { min: 600.0, max: 9_600.0, grip_in: 1_000.0, grip_out: 9_000.0 };

    /// A range from its limits, gripping just inside them.
    pub fn between(min: f64, max: f64) -> Range {
        let max = max.max(min * 1.5);
        Range { min, max, grip_in: min * 1.1, grip_out: max * 0.92 }
    }
}

/// One injected frame: both fingers down at `distance`, or both lifting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Frame {
    Touch(f64),
    Lift(f64),
}

impl Frame {
    pub fn distance(self) -> f64 {
        match self {
            Frame::Touch(d) | Frame::Lift(d) => d,
        }
    }

    /// The two contact points in pad himetric coordinates.
    pub fn contacts(self) -> [(i32, i32); 2] {
        let d = self.distance();
        let y = CENTER_Y.round() as i32;
        [((CENTER_X - d / 2.0).round() as i32, y), ((CENTER_X + d / 2.0).round() as i32, y)]
    }
}

#[derive(Debug)]
pub struct Pinch {
    range: Range,
    distance: f64,
    down: bool,
}

impl Default for Pinch {
    fn default() -> Self {
        Self { range: Range::PAD, distance: 0.0, down: false }
    }
}

impl Pinch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes effect from the next grip.
    pub fn set_range(&mut self, range: Range) {
        self.range = range;
    }

    /// The frame that repeats the current state (keep-alive while held).
    pub fn current(&self) -> Option<Frame> {
        self.down.then_some(Frame::Touch(self.distance))
    }

    fn grip(&mut self, at: f64) -> Vec<Frame> {
        self.distance = at;
        self.down = true;
        vec![Frame::Touch(at)]
    }

    /// Applies a magnification step. Positive spreads the fingers (zoom in).
    /// With no fingers down, it grips first, at the end of the range that
    /// suits the direction.
    pub fn change(&mut self, magnification: f64) -> Vec<Frame> {
        if !self.down {
            let at = if magnification >= 0.0 { self.range.grip_in } else { self.range.grip_out };
            let mut frames = self.grip(at);
            frames.extend(self.change(magnification));
            return frames;
        }
        let target = self.distance * (1.0 + magnification).max(0.05);
        let regrip = if target > self.range.max {
            Some(self.range.grip_in)
        } else if target < self.range.min {
            Some(self.range.grip_out)
        } else {
            None
        };
        match regrip {
            Some(at) => {
                let lift = Frame::Lift(self.distance);
                self.distance = at;
                vec![lift, Frame::Touch(at)]
            }
            None => {
                self.distance = target;
                vec![Frame::Touch(target)]
            }
        }
    }

    pub fn end(&mut self) -> Vec<Frame> {
        if !self.down {
            return Vec::new();
        }
        self.down = false;
        vec![Frame::Lift(self.distance)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAD: Range = Range::PAD;

    fn distance(frames: &[Frame]) -> f64 {
        frames.last().unwrap().distance()
    }

    /// A pinch with the fingers down at the zoom-in grip.
    fn gripped() -> Pinch {
        let mut p = Pinch::new();
        assert_eq!(p.change(0.0), vec![Frame::Touch(PAD.grip_in), Frame::Touch(PAD.grip_in)]);
        p
    }

    #[test]
    fn magnification_scales_distance_multiplicatively() {
        let mut p = gripped();
        assert!((distance(&p.change(0.1)) - 1_100.0).abs() < 1e-9);
        assert!((distance(&p.change(-0.2)) - 880.0).abs() < 1e-9);
    }

    #[test]
    fn centroid_never_moves() {
        for d in [PAD.min, PAD.grip_in, PAD.max] {
            let [a, b] = Frame::Touch(d).contacts();
            assert_eq!(a.1, b.1);
            assert_eq!(a.0 + b.0, PAD_WIDTH as i32);
            assert!(a.0 >= 0 && b.0 <= PAD_WIDTH as i32);
        }
    }

    #[test]
    fn regrips_when_fingers_would_leave_the_pad() {
        let mut p = gripped();
        let mut frames = Vec::new();
        for _ in 0..40 {
            frames = p.change(0.15);
            if frames.len() == 2 {
                break;
            }
        }
        assert!(matches!(frames[0], Frame::Lift(_)));
        assert_eq!(frames[1], Frame::Touch(PAD.grip_in), "zooming in re-grips near-closed");

        let mut p = gripped();
        for _ in 0..40 {
            frames = p.change(-0.15);
            if frames.len() == 2 {
                break;
            }
        }
        assert!(matches!(frames[0], Frame::Lift(_)));
        assert_eq!(frames[1], Frame::Touch(PAD.grip_out), "zooming out re-grips wide");
    }

    #[test]
    fn distance_stays_on_the_pad() {
        let mut p = gripped();
        for i in 0..500 {
            let m = if (i / 50) % 2 == 0 { 0.15 } else { -0.15 };
            for f in p.change(m) {
                let [a, b] = f.contacts();
                assert!(a.0 >= 0 && b.0 <= PAD_WIDTH as i32, "frame {f:?} off the pad");
            }
        }
    }

    #[test]
    fn end_lifts_once() {
        let mut p = gripped();
        assert_eq!(p.end().len(), 1);
        assert!(p.end().is_empty());
        assert_eq!(p.current(), None);
    }

    #[test]
    fn first_change_grips_where_the_direction_has_room() {
        let mut p = Pinch::new();
        assert_eq!(p.change(0.1)[0], Frame::Touch(PAD.grip_in), "zoom in starts narrow");
        assert!(p.current().is_some());

        let mut p = Pinch::new();
        assert_eq!(p.change(-0.1)[0], Frame::Touch(PAD.grip_out), "zoom out starts wide");
    }

    #[test]
    fn custom_range_is_respected() {
        let range = Range::between(200.0, 1_000.0);
        let mut p = Pinch::new();
        p.set_range(range);
        for i in 0..400 {
            let m = if (i / 40) % 2 == 0 { 0.12 } else { -0.12 };
            for f in p.change(m) {
                let d = f.distance();
                assert!(d >= range.min - 1e-9 && d <= range.max + 1e-9, "{f:?} outside {range:?}");
            }
        }
    }

    #[test]
    fn between_keeps_room_to_pinch() {
        let r = Range::between(200.0, 220.0);
        assert!(r.max >= 300.0);
        assert!(r.min < r.grip_in && r.grip_in < r.grip_out && r.grip_out < r.max);
    }
}
