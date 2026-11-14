// Added by ibis inc. (#209; modified by ibis inc.): the two-finger pinch tracking that every platform with a
// touch screen shares (it was the web platform's own). Not part of upstream gpui-pre; Apache-2.0.
//! Turns the positions of touch fingers into two-finger pinch gestures.
//!
//! Pure logic (no OS types, no DOM), so every platform feeds it its own finger ids and positions and it is unit
//! tested once. The rules:
//!
//! * One finger down is a single touch ([`PinchDown::Single`]); the platform handles it as it did before.
//! * A second finger interrupts the first: [`PinchDown::Begin`] names the first finger so the platform can cancel
//!   its single-touch gesture (`TouchSessionEvent` with `Cancelled`) and start the pinch.
//! * While pinching, the change of the distance between the two fingers and their midpoint are reported
//!   ([`PinchMove::Update`], the `delta` of a [`PinchEvent`](crate::PinchEvent)).
//! * Fingers beyond two, and the remaining finger after a pinch, are [`Ignored`](PinchDown::Ignored) until every
//!   finger has been lifted: a pinch never turns back into a single touch half way.

use std::collections::HashMap;

use crate::{Pixels, Point, point, px};

/// What a finger going down means.
#[derive(Debug, PartialEq)]
pub enum PinchDown {
    /// The only finger: an ordinary single touch.
    Single,
    /// The second finger went down: cancel the first finger's single touch (`cancelled`: its id and last
    /// position) and start a pinch at `center`.
    Begin {
        /// The first finger (id, last position).
        cancelled: (i64, Point<Pixels>),
        /// The midpoint of the two fingers.
        center: Point<Pixels>,
    },
    /// A third finger, or a finger while a finished pinch's fingers are still down: not a touch at all.
    Ignored,
}

/// What a finger moving means.
#[derive(Debug, PartialEq)]
pub enum PinchMove {
    /// A single touch moved.
    Single,
    /// `delta` is the change of the distance between the two fingers since the previous event as a ratio
    /// (0.1 = 10% wider apart), `center` their midpoint.
    Update {
        /// The midpoint of the two fingers.
        center: Point<Pixels>,
        /// The ratio of the distance change.
        delta: f32,
    },
    /// Not part of any touch the platform should act on.
    Ignored,
}

/// What a finger going up (or its pointer being cancelled) means.
#[derive(Debug, PartialEq)]
pub enum PinchUp {
    /// A single touch ended.
    Single,
    /// One of the two pinching fingers left; the pinch ends at `center`.
    End {
        /// The midpoint of the two fingers.
        center: Point<Pixels>,
    },
    /// Not part of any touch the platform should act on.
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PinchMode {
    Single,
    Pinching { a: i64, b: i64, last_distance: f32 },
    /// A pinch is over but fingers remain; they are not single touches until every finger has been lifted.
    Suppressed,
}

/// Turns the positions of touch fingers into two-finger pinch gestures (see the module docs).
pub struct PinchTracker {
    fingers: HashMap<i64, Point<Pixels>>,
    mode: PinchMode,
}

impl Default for PinchTracker {
    fn default() -> Self {
        Self { fingers: HashMap::new(), mode: PinchMode::Single }
    }
}

fn pinch_geometry(a: Point<Pixels>, b: Point<Pixels>) -> (Point<Pixels>, f32) {
    let dx = f32::from(a.x) - f32::from(b.x);
    let dy = f32::from(a.y) - f32::from(b.y);
    let center = point(px((f32::from(a.x) + f32::from(b.x)) / 2.0), px((f32::from(a.y) + f32::from(b.y)) / 2.0));
    (center, (dx * dx + dy * dy).sqrt())
}

impl PinchTracker {
    /// A finger went down.
    pub fn down(&mut self, id: i64, position: Point<Pixels>) -> PinchDown {
        let previous: Vec<(i64, Point<Pixels>)> =
            self.fingers.iter().filter(|(other, _)| **other != id).map(|(other, pos)| (*other, *pos)).collect();
        self.fingers.insert(id, position);
        match self.mode {
            PinchMode::Single => match previous.as_slice() {
                [] => PinchDown::Single,
                [(other, other_position)] => {
                    let (center, distance) = pinch_geometry(*other_position, position);
                    self.mode = PinchMode::Pinching { a: *other, b: id, last_distance: distance };
                    PinchDown::Begin { cancelled: (*other, *other_position), center }
                }
                _ => {
                    self.mode = PinchMode::Suppressed;
                    PinchDown::Ignored
                }
            },
            PinchMode::Pinching { .. } | PinchMode::Suppressed => PinchDown::Ignored,
        }
    }

    /// A finger moved.
    pub fn moved(&mut self, id: i64, position: Point<Pixels>) -> PinchMove {
        if let Some(finger) = self.fingers.get_mut(&id) {
            *finger = position;
        }
        match self.mode {
            PinchMode::Single => PinchMove::Single,
            PinchMode::Pinching { a, b, last_distance } if id == a || id == b => {
                let (Some(&pa), Some(&pb)) = (self.fingers.get(&a), self.fingers.get(&b)) else {
                    return PinchMove::Ignored;
                };
                let (center, distance) = pinch_geometry(pa, pb);
                let delta = if last_distance > f32::EPSILON { distance / last_distance - 1.0 } else { 0.0 };
                self.mode = PinchMode::Pinching { a, b, last_distance: distance };
                PinchMove::Update { center, delta }
            }
            _ => PinchMove::Ignored,
        }
    }

    /// A finger went up or its pointer was cancelled.
    pub fn up(&mut self, id: i64, position: Point<Pixels>) -> PinchUp {
        if let Some(finger) = self.fingers.get_mut(&id) {
            *finger = position;
        }
        let result = match self.mode {
            PinchMode::Single => PinchUp::Single,
            PinchMode::Pinching { a, b, .. } if id == a || id == b => {
                let center = match (self.fingers.get(&a), self.fingers.get(&b)) {
                    (Some(&pa), Some(&pb)) => pinch_geometry(pa, pb).0,
                    _ => position,
                };
                self.mode = PinchMode::Suppressed;
                PinchUp::End { center }
            }
            _ => PinchUp::Ignored,
        };
        self.fingers.remove(&id);
        if self.fingers.is_empty() {
            self.mode = PinchMode::Single;
        }
        result
    }

    /// Forget every finger (the window lost the touch device, or focus).
    pub fn clear(&mut self) {
        self.fingers.clear();
        self.mode = PinchMode::Single;
    }

    /// Whether a pinch is going on now.
    pub fn is_pinching(&self) -> bool {
        matches!(self.mode, PinchMode::Pinching { .. })
    }

    /// Whether any finger is down.
    pub fn has_fingers(&self) -> bool {
        !self.fingers.is_empty()
    }
}

// Unit tests: crates/ibis-office-app/src/tests/shell/touch_pinch.rs (gpui-pre's own test target needs upstream assets this fork does not carry).
