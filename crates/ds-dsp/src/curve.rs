//! Editable LFO shapes: a breakpoint curve with per-segment bend.
//!
//! Fixed capacity and `Copy` on purpose. The audio thread reads the whole curve
//! once per block, so it must never allocate, and keeping it plain data means
//! the editor can hand one over without any shared-mutability machinery.
//!
//! The first and last points are the same value by construction: an LFO loops,
//! and a curve whose ends disagree puts a step in the signal on every wrap.

/// Most breakpoints a curve can hold.
///
/// Generous for hand-drawn shapes while keeping the struct small enough to copy
/// per block without thinking about it.
pub const MAX_CURVE_POINTS: usize = 32;

/// How far a bend can be pushed either way.
const MAX_BEND_EXPONENT: f32 = 3.0;

/// One breakpoint.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CurvePoint {
    /// Position through the cycle, `0.0..=1.0`.
    pub x: f32,
    /// Level, `0.0..=1.0`, where 1.0 is the top of the display.
    pub y: f32,
    /// Bend of the segment that *starts* here, `-1.0..=1.0`. Zero is a straight
    /// line; positive leaves this point quickly, negative lingers.
    pub tension: f32,
}

impl CurvePoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y, tension: 0.0 }
    }
}

/// A closed LFO shape made of breakpoints.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(into = "Vec<CurvePoint>", from = "Vec<CurvePoint>"))]
pub struct LfoCurve {
    points: [CurvePoint; MAX_CURVE_POINTS],
    len: usize,
}

impl Default for LfoCurve {
    fn default() -> Self {
        Self::ramp_down()
    }
}

impl LfoCurve {
    /// A falling ramp: the shape a new LFO starts on, and the one whose ends
    /// most obviously have to agree.
    pub fn ramp_down() -> Self {
        Self::from_points(&[CurvePoint::new(0.0, 1.0), CurvePoint::new(1.0, 1.0)]).bent_first_segment(0.0)
    }

    /// Builds a curve from a slice, sorting it and forcing the ends to agree.
    ///
    /// Anything past [`MAX_CURVE_POINTS`] is dropped rather than silently
    /// reshaping the curve to fit.
    pub fn from_points(points: &[CurvePoint]) -> Self {
        let mut curve = Self { points: [CurvePoint::new(0.0, 0.5); MAX_CURVE_POINTS], len: 0 };
        for point in points.iter().take(MAX_CURVE_POINTS) {
            curve.points[curve.len] = CurvePoint {
                x: point.x.clamp(0.0, 1.0),
                y: point.y.clamp(0.0, 1.0),
                tension: point.tension.clamp(-1.0, 1.0),
            };
            curve.len += 1;
        }
        if curve.len < 2 {
            curve.points[0] = CurvePoint::new(0.0, 0.5);
            curve.points[1] = CurvePoint::new(1.0, 0.5);
            curve.len = 2;
        }
        curve.points[..curve.len].sort_by(|a, b| a.x.total_cmp(&b.x));
        curve.normalise_ends();
        curve
    }

    fn bent_first_segment(mut self, tension: f32) -> Self {
        self.points[0].tension = tension;
        self
    }

    /// Pins the ends to the edges of the cycle and makes them agree.
    ///
    /// The last point's level follows the first: the loop has to close, and the
    /// first point is the one the player thinks of as the start of the shape.
    fn normalise_ends(&mut self) {
        let last = self.len - 1;
        self.points[0].x = 0.0;
        self.points[last].x = 1.0;
        self.points[last].y = self.points[0].y;
        // The final point starts no segment, so its bend would never be drawn.
        self.points[last].tension = 0.0;
    }

    pub fn points(&self) -> &[CurvePoint] {
        &self.points[..self.len]
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// True once no more points will fit.
    pub fn is_full(&self) -> bool {
        self.len >= MAX_CURVE_POINTS
    }

    /// Level at a position through the cycle, in `0.0..=1.0`.
    pub fn sample(&self, phase: f32) -> f32 {
        let x = phase - phase.floor();
        let points = self.points();
        // Linear scan: a hand-drawn curve is a handful of points, and a binary
        // search would cost more in branches than it saves in comparisons.
        let mut index = 0;
        while index + 2 < points.len() && points[index + 1].x <= x {
            index += 1;
        }
        let start = points[index];
        let end = points[index + 1];
        let span = end.x - start.x;
        if span <= f32::EPSILON {
            return end.y;
        }
        let t = ((x - start.x) / span).clamp(0.0, 1.0);
        start.y + (end.y - start.y) * bend(t, start.tension)
    }

    /// Same level mapped to the bipolar range the modulation matrix works in.
    pub fn sample_bipolar(&self, phase: f32) -> f32 {
        self.sample(phase) * 2.0 - 1.0
    }

    /// Adds a point, returning where it landed.
    ///
    /// Refuses to sit on top of an existing point: two breakpoints at the same
    /// position make a segment of zero width that nothing can grab hold of.
    pub fn insert(&mut self, x: f32, y: f32) -> Option<usize> {
        if self.is_full() {
            return None;
        }
        let x = x.clamp(0.0, 1.0);
        let y = y.clamp(0.0, 1.0);
        let mut index = 0;
        while index < self.len && self.points[index].x < x {
            index += 1;
        }
        // Never before the first point or after the last: those two are the ends
        // of the cycle and stay where they are.
        let index = index.clamp(1, self.len - 1);
        if (self.points[index - 1].x - x).abs() < 1e-4 || (self.points[index].x - x).abs() < 1e-4 {
            return None;
        }

        // The new point splits a segment, so it inherits that segment's bend and
        // the two halves keep curving the same way.
        let tension = self.points[index - 1].tension;
        self.points.copy_within(index..self.len, index + 1);
        self.points[index] = CurvePoint { x, y, tension };
        self.len += 1;
        Some(index)
    }

    /// Removes an interior point. The two ends cannot be removed.
    pub fn remove(&mut self, index: usize) -> bool {
        if index == 0 || index + 1 >= self.len || self.len <= 2 {
            return false;
        }
        self.points.copy_within(index + 1..self.len, index);
        self.len -= 1;
        true
    }

    /// Moves a point, keeping it between its neighbours.
    ///
    /// The ends only move vertically, and they move together: this is what keeps
    /// the loop seamless no matter what the player drags.
    pub fn move_point(&mut self, index: usize, x: f32, y: f32) {
        if index >= self.len {
            return;
        }
        let y = y.clamp(0.0, 1.0);
        if index == 0 || index + 1 == self.len {
            self.points[0].y = y;
            self.points[self.len - 1].y = y;
            return;
        }
        // A small gap either side, so a dragged point can always be told apart
        // from its neighbours and never collapses a segment to nothing.
        const GAP: f32 = 1e-3;
        let low = self.points[index - 1].x + GAP;
        let high = self.points[index + 1].x - GAP;
        self.points[index].x = if low <= high { x.clamp(low, high) } else { self.points[index].x };
        self.points[index].y = y;
    }

    /// Sets the bend of the segment starting at `index`.
    pub fn set_tension(&mut self, index: usize, tension: f32) {
        if index + 1 < self.len {
            self.points[index].tension = tension.clamp(-1.0, 1.0);
        }
    }

    pub fn tension(&self, index: usize) -> f32 {
        self.points.get(index).map_or(0.0, |point| point.tension)
    }

    /// Index of the point within `radius` of a position, nearest first.
    pub fn point_near(&self, x: f32, y: f32, radius_x: f32, radius_y: f32) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (index, point) in self.points().iter().enumerate() {
            let dx = (point.x - x) / radius_x;
            let dy = (point.y - y) / radius_y;
            let distance = dx * dx + dy * dy;
            if distance <= 1.0 && best.is_none_or(|(_, previous)| distance < previous) {
                best = Some((index, distance));
            }
        }
        best.map(|(index, _)| index)
    }

    /// Index of the segment a position falls in, i.e. the point it starts from.
    pub fn segment_at(&self, x: f32) -> usize {
        let points = self.points();
        let mut index = 0;
        while index + 2 < points.len() && points[index + 1].x <= x {
            index += 1;
        }
        index
    }

    /// Replaces the curve with a sampled version of one of the fixed shapes.
    ///
    /// This is what makes the built-in shapes a starting point rather than a
    /// dead end: pick one, then drag it into whatever you actually wanted.
    pub fn from_shape(shape: crate::LfoShape) -> Self {
        use crate::LfoShape;
        match shape {
            // A seed, not a reproduction: five points a player can actually
            // grab, bent to the tension whose midpoint matches a real sine
            // (t^0.5 passes through 0.7071 at the halfway mark).
            LfoShape::Sine => Self::from_points(&[
                CurvePoint { x: 0.0, y: 0.5, tension: 1.0 / 3.0 },
                CurvePoint { x: 0.25, y: 1.0, tension: -1.0 / 3.0 },
                CurvePoint { x: 0.5, y: 0.5, tension: 1.0 / 3.0 },
                CurvePoint { x: 0.75, y: 0.0, tension: -1.0 / 3.0 },
                CurvePoint { x: 1.0, y: 0.5, tension: 0.0 },
            ]),
            LfoShape::Triangle => Self::from_points(&[
                CurvePoint::new(0.0, 0.5),
                CurvePoint::new(0.25, 1.0),
                CurvePoint::new(0.75, 0.0),
                CurvePoint::new(1.0, 0.5),
            ]),
            // A saw needs a vertical jump, which is two points a hair apart.
            LfoShape::SawUp => Self::from_points(&[
                CurvePoint::new(0.0, 0.0),
                CurvePoint::new(0.999, 1.0),
                CurvePoint::new(1.0, 0.0),
            ]),
            LfoShape::SawDown => Self::from_points(&[
                CurvePoint::new(0.0, 1.0),
                CurvePoint::new(0.999, 0.0),
                CurvePoint::new(1.0, 1.0),
            ]),
            LfoShape::Square => Self::from_points(&[
                CurvePoint::new(0.0, 1.0),
                CurvePoint::new(0.499, 1.0),
                CurvePoint::new(0.5, 0.0),
                CurvePoint::new(0.999, 0.0),
                CurvePoint::new(1.0, 1.0),
            ]),
            // Random by nature, so there is nothing meaningful to draw; a flat
            // line is at least honest about the curve not being what runs.
            LfoShape::SampleHold | LfoShape::Custom => Self::from_points(&[
                CurvePoint::new(0.0, 0.5),
                CurvePoint::new(1.0, 0.5),
            ]),
        }
    }
}

impl From<Vec<CurvePoint>> for LfoCurve {
    fn from(points: Vec<CurvePoint>) -> Self {
        Self::from_points(&points)
    }
}

impl From<LfoCurve> for Vec<CurvePoint> {
    fn from(curve: LfoCurve) -> Self {
        curve.points().to_vec()
    }
}

/// Warps a `0..1` position along a segment.
///
/// A power curve rather than a spline: it is monotonic, hits both ends exactly,
/// and one number describes the whole bend, which is what a single drag handle
/// can express.
fn bend(t: f32, tension: f32) -> f32 {
    if tension.abs() < 1e-4 {
        return t;
    }
    let exponent = 2f32.powf(-tension.clamp(-1.0, 1.0) * MAX_BEND_EXPONENT);
    t.powf(exponent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_ends_always_hold_the_same_level() {
        let mut curve = LfoCurve::from_points(&[
            CurvePoint::new(0.0, 0.2),
            CurvePoint::new(0.5, 1.0),
            // Deliberately disagreeing with the first point.
            CurvePoint::new(1.0, 0.9),
        ]);
        assert_eq!(curve.sample(0.0), curve.sample(1.0));

        // Dragging either end has to keep them together, or the LFO would step
        // every time it wrapped.
        curve.move_point(0, 0.0, 0.75);
        assert!((curve.sample(0.0) - 0.75).abs() < 1e-6);
        assert!((curve.sample(0.9999) - 0.75).abs() < 1e-3);

        curve.move_point(curve.len() - 1, 1.0, 0.1);
        assert!((curve.sample(0.0) - 0.1).abs() < 1e-6);
    }

    #[test]
    fn the_ends_stay_pinned_to_the_edges_of_the_cycle() {
        let mut curve = LfoCurve::from_points(&[
            CurvePoint::new(0.0, 0.5),
            CurvePoint::new(0.5, 1.0),
            CurvePoint::new(1.0, 0.5),
        ]);
        curve.move_point(0, 0.4, 0.5);
        curve.move_point(2, 0.6, 0.5);
        assert_eq!(curve.points()[0].x, 0.0);
        assert_eq!(curve.points()[2].x, 1.0);
    }

    #[test]
    fn a_point_can_be_added_and_taken_away_again() {
        let mut curve = LfoCurve::from_points(&[CurvePoint::new(0.0, 0.5), CurvePoint::new(1.0, 0.5)]);
        let index = curve.insert(0.5, 1.0).expect("insert should have found room");
        assert_eq!(index, 1);
        assert_eq!(curve.len(), 3);
        assert!((curve.sample(0.5) - 1.0).abs() < 1e-5);

        assert!(curve.remove(index));
        assert_eq!(curve.len(), 2);
        assert!((curve.sample(0.5) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn the_end_points_cannot_be_removed() {
        let mut curve = LfoCurve::from_points(&[
            CurvePoint::new(0.0, 0.5),
            CurvePoint::new(0.5, 1.0),
            CurvePoint::new(1.0, 0.5),
        ]);
        assert!(!curve.remove(0), "the start of the cycle was removed");
        assert!(!curve.remove(2), "the end of the cycle was removed");
        assert_eq!(curve.len(), 3);
    }

    #[test]
    fn a_point_never_crosses_its_neighbours() {
        let mut curve = LfoCurve::from_points(&[
            CurvePoint::new(0.0, 0.5),
            CurvePoint::new(0.3, 1.0),
            CurvePoint::new(0.6, 0.0),
            CurvePoint::new(1.0, 0.5),
        ]);
        // Dragged far past the point on its right.
        curve.move_point(1, 0.95, 1.0);
        let points = curve.points();
        assert!(points[1].x < points[2].x, "points crossed: {points:?}");
        curve.move_point(2, -1.0, 0.0);
        let points = curve.points();
        assert!(points[1].x < points[2].x, "points crossed: {points:?}");
    }

    #[test]
    fn the_curve_never_leaves_its_range_or_goes_backwards() {
        let mut curve = LfoCurve::from_shape(crate::LfoShape::Sine);
        curve.set_tension(0, 1.0);
        curve.set_tension(1, -1.0);
        for step in 0..=1_000 {
            let value = curve.sample(step as f32 / 1_000.0);
            assert!(value.is_finite(), "sample was {value}");
            assert!((0.0..=1.0).contains(&value), "sample left range: {value}");
        }
        // Ordering has to hold for every point, or the scan in sample() breaks.
        for pair in curve.points().windows(2) {
            assert!(pair[0].x <= pair[1].x, "points out of order: {pair:?}");
        }
    }

    #[test]
    fn bend_leaves_the_ends_of_a_segment_alone() {
        for tension in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            assert!((bend(0.0, tension) - 0.0).abs() < 1e-6, "start moved at {tension}");
            assert!((bend(1.0, tension) - 1.0).abs() < 1e-6, "end moved at {tension}");
        }
        // Positive tension leaves the first point sooner, negative later.
        assert!(bend(0.5, 0.6) > 0.5);
        assert!(bend(0.5, -0.6) < 0.5);
        assert!((bend(0.5, 0.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_sampled_built_in_shape_matches_the_shape_it_came_from() {
        use crate::LfoShape;

        // The sampled sine is an approximation, so this checks it tracks rather
        // than matches: peak near a quarter, trough near three quarters.
        let sine = LfoCurve::from_shape(LfoShape::Sine);
        assert!((sine.sample(0.25) - 1.0).abs() < 0.02, "peak was {}", sine.sample(0.25));
        assert!(sine.sample(0.75) < 0.02, "trough was {}", sine.sample(0.75));
        assert!((sine.sample(0.0) - 0.5).abs() < 0.02);

        let square = LfoCurve::from_shape(LfoShape::Square);
        assert!(square.sample(0.25) > 0.98);
        assert!(square.sample(0.75) < 0.02);

        let saw = LfoCurve::from_shape(LfoShape::SawUp);
        assert!(saw.sample(0.5) > saw.sample(0.1), "saw did not rise");
    }

    #[test]
    fn a_full_curve_refuses_more_points() {
        let mut curve = LfoCurve::from_points(&[CurvePoint::new(0.0, 0.5), CurvePoint::new(1.0, 0.5)]);
        // Spread out enough that none of them land on top of each other.
        for step in 1..MAX_CURVE_POINTS {
            let x = step as f32 / MAX_CURVE_POINTS as f32;
            if curve.insert(x, 0.5).is_none() {
                break;
            }
        }
        assert!(curve.is_full(), "only reached {} points", curve.len());
        assert_eq!(curve.insert(0.123_45, 0.5), None, "a full curve took another point");
    }

    #[test]
    fn points_on_top_of_each_other_are_refused() {
        let mut curve = LfoCurve::from_points(&[
            CurvePoint::new(0.0, 0.5),
            CurvePoint::new(0.5, 1.0),
            CurvePoint::new(1.0, 0.5),
        ]);
        assert_eq!(curve.insert(0.5, 0.2), None, "a duplicate position was accepted");
        assert_eq!(curve.insert(0.500_01, 0.2), None, "a near-duplicate was accepted");
        assert!(curve.insert(0.75, 0.2).is_some());
    }
}
