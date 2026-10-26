//! Gesture controls for touch screens: drag sideways to shift, tap to rotate
//! (right half clockwise, left half counter-clockwise), drag down to
//! soft-drop, flick down to hard-drop, and swipe up to hold. Touches with
//! more than one finger are ignored.

use std::collections::VecDeque;

use macroquad::input::{touches, Touch, TouchPhase};
use macroquad::math::Vec2;
use macroquad::miniquad::window::dpi_scale;
use macroquad::window::screen_width;

// Distances and speeds are in points so gestures feel the same at any
// screen density.
const SLOP_POINTS: f32 = 10.0;
const COLUMN_POINTS: f32 = 22.0;
const HOLD_POINTS: f32 = 36.0;
const FLICK_POINTS_PER_SECOND: f32 = 900.0;
const TAP_SECONDS: f64 = 0.35;
const VELOCITY_WINDOW_SECONDS: f64 = 0.08;

/// What the fingers asked for this frame.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Gestures {
    /// Columns to shift, negative for left.
    pub shift: i32,
    pub rotate_left: bool,
    pub rotate_right: bool,
    pub soft_drop: bool,
    pub hard_drop: bool,
    pub hold_piece: bool,

    /// Where a single-finger tap landed, in screen pixels, for menus.
    pub tap: Option<Vec2>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Intent {
    Undecided,
    Shift,
    SoftDrop,
    Lift,
    Spent,
}

struct Gesture {
    finger: u64,
    start: Vec2,
    started_at: f64,
    last: Vec2,
    samples: VecDeque<(f64, Vec2)>,
    intent: Intent,
    columns: i32,
    max_fingers: usize,
    cancelled: bool,
}

impl Gesture {
    fn new(touch: &Touch, now: f64) -> Self {
        Self {
            finger: touch.id,
            start: touch.position,
            started_at: now,
            last: touch.position,
            samples: VecDeque::new(),
            intent: Intent::Undecided,
            columns: 0,
            max_fingers: 1,
            cancelled: false,
        }
    }

    fn record(&mut self, now: f64, position: Vec2) {
        self.last = position;
        self.samples.push_back((now, position));

        while self
            .samples
            .front()
            .is_some_and(|&(time, _)| now - time > VELOCITY_WINDOW_SECONDS)
        {
            self.samples.pop_front();
        }
    }

    /// Recent finger velocity in screen pixels per second.
    fn velocity(&self) -> Vec2 {
        let (Some(&(first_time, first)), Some(&(last_time, last))) =
            (self.samples.front(), self.samples.back())
        else {
            return Vec2::ZERO;
        };

        let seconds = (last_time - first_time) as f32;
        if seconds <= 0.0 {
            return Vec2::ZERO;
        }

        (last - first) / seconds
    }

    fn interpret(&mut self, points: f32, gestures: &mut Gestures) {
        let delta = (self.last - self.start) / points;

        if self.intent == Intent::Undecided && delta.length() > SLOP_POINTS {
            self.intent = if delta.x.abs() > delta.y.abs() {
                Intent::Shift
            } else if delta.y > 0.0 {
                Intent::SoftDrop
            } else {
                Intent::Lift
            };
        }

        match self.intent {
            Intent::Shift => {
                let columns = (delta.x / COLUMN_POINTS).trunc() as i32;
                gestures.shift += columns - self.columns;
                self.columns = columns;
            }
            Intent::SoftDrop => gestures.soft_drop = true,
            Intent::Lift if -delta.y > HOLD_POINTS => {
                gestures.hold_piece = true;
                self.intent = Intent::Spent;
            }
            _ => {}
        }

        // A flick has to be mostly downward so a quick sideways drag that
        // dips a little never slams the piece.
        let velocity = self.velocity() / points;
        let flicked = velocity.y > FLICK_POINTS_PER_SECOND && velocity.y > 2.0 * velocity.x.abs();
        if matches!(self.intent, Intent::Shift | Intent::SoftDrop) && flicked {
            gestures.hard_drop = true;
            gestures.soft_drop = false;
            self.intent = Intent::Spent;
        }
    }

    fn finish(&self, now: f64, screen_width: f32, gestures: &mut Gestures) {
        if self.cancelled || self.intent != Intent::Undecided || self.max_fingers >= 2 {
            return;
        }
        if now - self.started_at >= TAP_SECONDS {
            return;
        }

        gestures.tap = Some(self.last);
        if self.last.x >= screen_width / 2.0 {
            gestures.rotate_right = true;
        } else {
            gestures.rotate_left = true;
        }
    }
}

#[derive(Default)]
pub struct TouchControls {
    gesture: Option<Gesture>,

    /// Set after a multi-finger gesture. Lifting one of several fingers
    /// reports every finger as ended, so the ones still down come back as
    /// fresh touches and must not start a gesture of their own.
    ignore_until_clear: bool,
}

impl TouchControls {
    pub fn poll(&mut self, now: f64) -> Gestures {
        self.step(&touches(), now, dpi_scale(), screen_width())
    }

    fn step(&mut self, touches: &[Touch], now: f64, points: f32, screen_width: f32) -> Gestures {
        let mut gestures = Gestures::default();

        if touches.is_empty() {
            self.gesture = None;
            self.ignore_until_clear = false;
            return gestures;
        }

        if self.ignore_until_clear {
            return gestures;
        }

        let gesture = self
            .gesture
            .get_or_insert_with(|| Gesture::new(&touches[0], now));
        gesture.max_fingers = gesture.max_fingers.max(touches.len());
        gesture.cancelled |= touches
            .iter()
            .any(|touch| touch.phase == TouchPhase::Cancelled);

        if let Some(touch) = touches.iter().find(|touch| touch.id == gesture.finger) {
            gesture.record(now, touch.position);
        }

        if gesture.max_fingers == 1 {
            gesture.interpret(points, &mut gestures);
        }

        let lifted = touches
            .iter()
            .all(|touch| matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled));
        if lifted {
            gesture.finish(now, screen_width, &mut gestures);
            self.ignore_until_clear = gesture.max_fingers >= 2;
            self.gesture = None;
        }

        gestures
    }
}

#[cfg(test)]
mod tests {
    use super::{Gestures, TouchControls};
    use macroquad::input::{Touch, TouchPhase};
    use macroquad::math::{vec2, Vec2};

    const POINTS: f32 = 3.0;
    const SCREEN_WIDTH: f32 = 2622.0;
    const FRAME: f64 = 1.0 / 60.0;

    fn touch(id: u64, phase: TouchPhase, position: Vec2) -> Touch {
        Touch {
            id,
            phase,
            position,
        }
    }

    /// Drag one finger along `path` (screen pixels), one point per frame,
    /// and collect what every frame reported.
    fn drag(controls: &mut TouchControls, path: &[Vec2]) -> Vec<Gestures> {
        let last = path.len() - 1;

        path.iter()
            .enumerate()
            .map(|(frame, &position)| {
                let phase = match frame {
                    0 => TouchPhase::Started,
                    _ if frame == last => TouchPhase::Ended,
                    _ => TouchPhase::Moved,
                };
                let touches = [touch(1, phase, position)];
                controls.step(&touches, frame as f64 * FRAME, POINTS, SCREEN_WIDTH)
            })
            .collect()
    }

    #[test]
    fn a_tap_rotates_toward_the_tapped_half() {
        let mut controls = TouchControls::default();
        let right = drag(&mut controls, &[vec2(2000.0, 600.0), vec2(2001.0, 600.0)]);
        let left = drag(&mut controls, &[vec2(300.0, 600.0), vec2(300.0, 602.0)]);

        assert!(right[1].rotate_right && !right[1].rotate_left);
        assert_eq!(right[1].tap, Some(vec2(2001.0, 600.0)));
        assert!(left[1].rotate_left && !left[1].rotate_right);
    }

    #[test]
    fn a_tap_that_starts_and_ends_within_one_frame_still_counts() {
        let mut controls = TouchControls::default();
        let touches = [touch(1, TouchPhase::Ended, vec2(2000.0, 600.0))];

        assert!(
            controls
                .step(&touches, 0.0, POINTS, SCREEN_WIDTH)
                .rotate_right
        );
    }

    #[test]
    fn a_sideways_drag_shifts_one_column_per_step_without_rotating() {
        let mut controls = TouchControls::default();
        let step = super::COLUMN_POINTS * POINTS;
        let path: Vec<Vec2> = (0..=12)
            .map(|i| vec2(1300.0 - i as f32 * step / 4.0, 600.0))
            .collect();
        let frames = drag(&mut controls, &path);

        assert_eq!(frames.iter().map(|g| g.shift).sum::<i32>(), -3);
        assert!(frames
            .iter()
            .all(|g| !g.rotate_left && !g.rotate_right && !g.hard_drop));
    }

    #[test]
    fn a_slow_drag_down_soft_drops_and_a_flick_hard_drops() {
        let mut controls = TouchControls::default();
        let slow: Vec<Vec2> = (0..=30)
            .map(|i| vec2(1300.0, 300.0 + i as f32 * 4.0))
            .collect();
        let slow_frames = drag(&mut controls, &slow);

        assert!(slow_frames.iter().any(|g| g.soft_drop));
        assert!(slow_frames.iter().all(|g| !g.hard_drop));

        let flick: Vec<Vec2> = (0..=4)
            .map(|i| vec2(1300.0, 300.0 + i as f32 * 90.0))
            .collect();
        let flick_frames = drag(&mut controls, &flick);

        assert_eq!(flick_frames.iter().filter(|g| g.hard_drop).count(), 1);
    }

    #[test]
    fn a_swipe_up_holds_once() {
        let mut controls = TouchControls::default();
        let path: Vec<Vec2> = (0..=10)
            .map(|i| vec2(1300.0, 900.0 - i as f32 * 20.0))
            .collect();
        let frames = drag(&mut controls, &path);

        assert_eq!(frames.iter().filter(|g| g.hold_piece).count(), 1);
        assert!(frames.iter().all(|g| !g.hard_drop && !g.soft_drop));
    }

    #[test]
    fn a_two_finger_tap_does_nothing_and_the_remaining_finger_is_ignored() {
        let mut controls = TouchControls::default();
        let a = vec2(800.0, 600.0);
        let b = vec2(1800.0, 600.0);
        let mut step = |touches: &[Touch], frame: u32| {
            controls.step(touches, f64::from(frame) * FRAME, POINTS, SCREEN_WIDTH)
        };

        step(&[touch(1, TouchPhase::Started, a)], 0);
        step(
            &[
                touch(1, TouchPhase::Started, a),
                touch(2, TouchPhase::Started, b),
            ],
            1,
        );
        let lifted = step(
            &[
                touch(1, TouchPhase::Ended, a),
                touch(2, TouchPhase::Ended, b),
            ],
            5,
        );

        assert_eq!(lifted, Gestures::default());

        let lingering = step(&[touch(1, TouchPhase::Ended, a)], 7);
        assert_eq!(lingering, Gestures::default());
        assert_eq!(step(&[], 8), Gestures::default());
        assert!(step(&[touch(3, TouchPhase::Ended, b)], 9).rotate_right);
    }
}
