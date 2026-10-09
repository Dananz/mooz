//! Finger geometry for the synthetic touchpad pinch. Two contacts sit
//! symmetric about the pad's center on a horizontal line, so their centroid
//! never moves (a pure zoom, no pan). Each magnification step scales their
//! distance by `1 + m`, the same multiplicative meaning as a macOS magnify
//! event. When the fingers would leave the pad, they lift and re-grip at the
//! far end for the current direction; the app sees a new pinch and continues.

/// Pad size in himetric (0.01 mm), as in Microsoft's injection sample.
pub const PAD_WIDTH: u32 = 10_000;
pub const PAD_HEIGHT: u32 = 6_000;
const CENTER_X: f64 = PAD_WIDTH as f64 / 2.0;
const CENTER_Y: f64 = PAD_HEIGHT as f64 / 2.0;

/// Finger distance at the start of a gesture.
pub const START_DISTANCE: f64 = 3_000.0;
/// Closest and farthest the two fingers get before re-gripping.
pub const MIN_DISTANCE: f64 = 600.0;
pub const MAX_DISTANCE: f64 = 9_600.0;
/// Where a re-grip lands: near-closed to keep zooming in, wide to keep zooming out.
const REGRIP_IN: f64 = 1_000.0;
const REGRIP_OUT: f64 = 9_000.0;

/// One injected frame: both fingers down at `distance`, or both lifting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Frame {
    Touch(f64),
    Lift(f64),
}

impl Frame {
    /// The two contact points in pad himetric coordinates.
    pub fn contacts(self) -> [(i32, i32); 2] {
        let d = match self {
            Frame::Touch(d) | Frame::Lift(d) => d,
        };
        let y = CENTER_Y.round() as i32;
        [((CENTER_X - d / 2.0).round() as i32, y), ((CENTER_X + d / 2.0).round() as i32, y)]
    }
}

#[derive(Debug, Default)]
pub struct Pinch {
    distance: f64,
    down: bool,
}

impl Pinch {
    pub fn new() -> Self {
        Self::default()
    }

    /// The frame that repeats the current state (keep-alive while held).
    pub fn current(&self) -> Option<Frame> {
        self.down.then_some(Frame::Touch(self.distance))
    }

    pub fn begin(&mut self) -> Vec<Frame> {
        self.distance = START_DISTANCE;
        self.down = true;
        vec![Frame::Touch(self.distance)]
    }

    /// Applies a magnification step. Positive spreads the fingers (zoom in).
    pub fn change(&mut self, magnification: f64) -> Vec<Frame> {
        if !self.down {
            let mut frames = self.begin();
            frames.extend(self.change(magnification));
            return frames;
        }
        let target = self.distance * (1.0 + magnification).max(0.05);
        let regrip = if target > MAX_DISTANCE {
            Some(REGRIP_IN)
        } else if target < MIN_DISTANCE {
            Some(REGRIP_OUT)
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

    fn distance(frames: &[Frame]) -> f64 {
        match frames.last().unwrap() {
            Frame::Touch(d) | Frame::Lift(d) => *d,
        }
    }

    #[test]
    fn begin_touches_at_start_distance() {
        let mut p = Pinch::new();
        assert_eq!(p.begin(), vec![Frame::Touch(START_DISTANCE)]);
        assert!(p.current().is_some());
    }

    #[test]
    fn magnification_scales_distance_multiplicatively() {
        let mut p = Pinch::new();
        p.begin();
        assert!((distance(&p.change(0.1)) - 3_300.0).abs() < 1e-9);
        assert!((distance(&p.change(-0.5)) - 1_650.0).abs() < 1e-9);
    }

    #[test]
    fn centroid_never_moves() {
        for d in [MIN_DISTANCE, START_DISTANCE, MAX_DISTANCE] {
            let [a, b] = Frame::Touch(d).contacts();
            assert_eq!(a.1, b.1);
            assert_eq!(a.0 + b.0, PAD_WIDTH as i32);
            assert!(a.0 >= 0 && b.0 <= PAD_WIDTH as i32);
        }
    }

    #[test]
    fn regrips_when_fingers_would_leave_the_pad() {
        let mut p = Pinch::new();
        p.begin();
        let mut frames = Vec::new();
        for _ in 0..40 {
            frames = p.change(0.15);
            if frames.len() == 2 {
                break;
            }
        }
        assert!(matches!(frames[0], Frame::Lift(_)));
        assert_eq!(frames[1], Frame::Touch(REGRIP_IN), "zooming in re-grips near-closed");

        let mut p = Pinch::new();
        p.begin();
        for _ in 0..40 {
            frames = p.change(-0.15);
            if frames.len() == 2 {
                break;
            }
        }
        assert_eq!(frames[1], Frame::Touch(REGRIP_OUT), "zooming out re-grips wide");
    }

    #[test]
    fn distance_stays_on_the_pad() {
        let mut p = Pinch::new();
        p.begin();
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
        let mut p = Pinch::new();
        p.begin();
        assert_eq!(p.end().len(), 1);
        assert!(p.end().is_empty());
        assert_eq!(p.current(), None);
    }

    #[test]
    fn change_without_begin_starts_a_pinch() {
        let mut p = Pinch::new();
        let frames = p.change(0.1);
        assert_eq!(frames[0], Frame::Touch(START_DISTANCE));
        assert!(p.current().is_some());
    }
}
