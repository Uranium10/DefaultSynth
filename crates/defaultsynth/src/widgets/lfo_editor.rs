//! The LFO well: a drawable shape, Serum style.
//!
//! Left-drag a point to move it, double-click empty space to add one and
//! double-click a point to take it away. Hovering over a segment raises a handle
//! at its midpoint; dragging that handle bends the segment, and double-clicking
//! it straightens the segment again.
//!
//! The two ends are one value. An LFO wraps, so a curve whose ends disagree puts
//! a step into the signal on every cycle; dragging either end moves both.
//!
//! Right-click steps through the built-in shapes. The design's LFO panel has a
//! single selector box and its label is TRIG, so there is no drawn control left
//! for the shape — rather than adding a box the design does not have, the well
//! carries it. Editing a built-in shape seeds the curve from it and switches to
//! Custom, so the fixed shapes are starting points rather than dead ends.

use crossbeam::atomic::AtomicCell;
use ds_dsp::LfoCurve;
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use std::sync::Arc;

use crate::params::{DefaultSynthParams, LfoShapeParam};

/// Points along the drawn trace.
const TRACE_POINTS: usize = 240;
/// Hit radius around a breakpoint, in pixels.
const POINT_GRAB_RADIUS: f32 = 10.0;
const POINT_RADIUS: f32 = 4.0;
/// Hit radius around a segment's bend handle, in pixels.
const HANDLE_GRAB_RADIUS: f32 = 9.0;
const HANDLE_RADIUS: f32 = 3.5;
/// Rise a segment needs before its bend is worth showing a handle for.
///
/// A flat segment holds one level whatever its bend is, so a handle there would
/// be a control that visibly does nothing.
const FLAT_SEGMENT: f32 = 0.02;

/// What a drag is moving.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Grip {
    Point(usize),
    Bend(usize),
}

/// What the pointer is over, which is what decides whether a handle is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Hover {
    #[default]
    None,
    Point(usize),
    Segment(usize),
}

pub struct LfoEditor {
    /// The shape being edited, shared with the audio thread.
    curve: Arc<AtomicCell<LfoCurve>>,
    shape: ParamWidgetBase,
    shape_count: usize,
    drag: Option<Grip>,
    hover: Hover,
}

/// Maps between the well's pixels and the curve's `0..1` space.
struct Layout {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

impl Layout {
    fn to_px(&self, x: f32, y: f32) -> (f32, f32) {
        (self.left + x * self.width, self.top + (1.0 - y) * self.height)
    }

    fn from_px(&self, px: f32, py: f32) -> (f32, f32) {
        (
            ((px - self.left) / self.width).clamp(0.0, 1.0),
            (1.0 - (py - self.top) / self.height).clamp(0.0, 1.0),
        )
    }
}

impl LfoEditor {
    pub fn new<L>(
        cx: &mut Context,
        params: L,
        select_curve: fn(&DefaultSynthParams) -> &Arc<AtomicCell<LfoCurve>>,
        select_shape: fn(&DefaultSynthParams) -> &EnumParam<LfoShapeParam>,
    ) -> Handle<'_, Self>
    where
        L: Lens<Target = Arc<DefaultSynthParams>> + Clone,
    {
        // The curve is not a parameter, so it is reached through the Arc rather
        // than through a lens: the editor and the audio thread share one cell.
        let curve = select_curve(&params.get(cx)).clone();
        let shape = ParamWidgetBase::new(cx, params.clone(), move |p| select_shape(p));
        let shape_count = shape.step_count().unwrap_or(0) + 1;
        Self { curve, shape, shape_count, drag: None, hover: Hover::default() }
            .build(cx, ParamWidgetBase::build_view(params, move |p| select_shape(p), move |_cx, _data| {}))
    }

    fn layout(&self, bounds: BoundingBox) -> Layout {
        // Enough inset that a point sitting at the very top or bottom is still
        // drawn as a whole circle rather than half-buried in the border.
        let pad_y = 9.0_f32.min(bounds.h * 0.14);
        let pad_x = 8.0_f32.min(bounds.w * 0.05);
        Layout {
            left: bounds.x + pad_x,
            top: bounds.y + pad_y,
            width: (bounds.w - pad_x * 2.0).max(1.0),
            height: (bounds.h - pad_y * 2.0).max(1.0),
        }
    }

    fn shape(&self) -> LfoShapeParam {
        let steps = (self.shape_count - 1).max(1) as f32;
        LfoShapeParam::from_index((self.shape.unmodulated_normalized_value() * steps).round() as usize)
    }

    fn set_shape(&self, cx: &mut EventContext, shape: LfoShapeParam) {
        let steps = (self.shape_count - 1).max(1) as f32;
        self.shape.begin_set_parameter(cx);
        self.shape.set_normalized_value(cx, shape.to_index() as f32 / steps);
        self.shape.end_set_parameter(cx);
    }

    /// The curve the well is showing.
    ///
    /// A built-in shape has no breakpoints of its own, so the well previews what
    /// editing it would give you: the seed curve, drawn without handles.
    fn displayed_curve(&self) -> LfoCurve {
        match self.shape() {
            LfoShapeParam::Custom => self.curve.load(),
            other => LfoCurve::from_shape(other.to_dsp()),
        }
    }

    fn is_editable(&self) -> bool {
        self.shape() == LfoShapeParam::Custom
    }

    /// Makes the well editable, seeding the curve from whatever was showing.
    ///
    /// Called by the first edit on a built-in shape, so a player never has to
    /// know that "Custom" is a mode they were supposed to select first.
    fn begin_editing(&mut self, cx: &mut EventContext) -> LfoCurve {
        if self.is_editable() {
            return self.curve.load();
        }
        let seeded = LfoCurve::from_shape(self.shape().to_dsp());
        self.curve.store(seeded);
        self.set_shape(cx, LfoShapeParam::Custom);
        seeded
    }

    /// Where a segment's bend handle sits, if that segment has one.
    fn handle_position(&self, curve: &LfoCurve, layout: &Layout, segment: usize) -> Option<(f32, f32)> {
        let points = curve.points();
        let (start, end) = (points.get(segment)?, points.get(segment + 1)?);
        if (end.y - start.y).abs() < FLAT_SEGMENT {
            return None;
        }
        let mid_x = (start.x + end.x) * 0.5;
        Some(layout.to_px(mid_x, curve.sample(mid_x)))
    }

    /// What the pointer is over, preferring points to the segments behind them.
    fn hit(&self, curve: &LfoCurve, layout: &Layout, px: f32, py: f32) -> Hover {
        let (x, y) = layout.from_px(px, py);
        if let Some(index) =
            curve.point_near(x, y, POINT_GRAB_RADIUS / layout.width, POINT_GRAB_RADIUS / layout.height)
        {
            return Hover::Point(index);
        }
        let segment = curve.segment_at(x);
        if let Some((hx, hy)) = self.handle_position(curve, layout, segment) {
            if (hx - px).hypot(hy - py) <= HANDLE_GRAB_RADIUS {
                return Hover::Segment(segment);
            }
        }
        Hover::None
    }

    /// Solves for the bend that puts a segment's midpoint under the cursor.
    ///
    /// Exact rather than incremental, so the curve stays stuck to the pointer
    /// for the whole drag instead of drifting away from it.
    fn bend_towards(curve: &LfoCurve, segment: usize, target: f32) -> Option<f32> {
        let points = curve.points();
        let (start, end) = (points.get(segment)?, points.get(segment + 1)?);
        let span = end.y - start.y;
        if span.abs() < FLAT_SEGMENT {
            return None;
        }
        // Fraction of the segment's rise the midpoint should sit at. Clamped off
        // both ends: the curve can approach them but never reach them, and the
        // logarithms below are undefined there.
        let fraction = ((target - start.y) / span).clamp(0.02, 0.98);
        // The bend is `t^e` with `e = 2^(-3 * tension)`, so at t = 0.5 the
        // midpoint is `0.5^e = fraction`. Undo both steps.
        let exponent = -fraction.log2();
        Some((-exponent.log2() / 3.0).clamp(-1.0, 1.0))
    }

    fn end_drag(&mut self, cx: &mut EventContext) {
        if self.drag.take().is_some() {
            cx.release();
            cx.set_active(false);
            cx.needs_redraw();
        }
    }
}

impl View for LfoEditor {
    fn element(&self) -> Option<&'static str> {
        Some("lfo-editor")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                let (px, py) = (cx.mouse().cursorx, cx.mouse().cursory);
                let layout = self.layout(cx.bounds());
                // Grabbing anything at all is an edit, so a built-in shape turns
                // into a curve before the hit test decides what was grabbed.
                let curve = self.begin_editing(cx);
                self.drag = match self.hit(&curve, &layout, px, py) {
                    Hover::Point(index) => Some(Grip::Point(index)),
                    Hover::Segment(index) => Some(Grip::Bend(index)),
                    Hover::None => None,
                };
                if self.drag.is_some() {
                    cx.capture();
                    cx.set_active(true);
                    meta.consume();
                }
                cx.needs_redraw();
            }
            WindowEvent::MouseDown(MouseButton::Right) => {
                // Steps through the built-in shapes, wrapping back to the first
                // past Custom so right-clicking always eventually gets you home.
                let next = (self.shape().to_index() + 1) % self.shape_count;
                self.set_shape(cx, LfoShapeParam::from_index(next));
                cx.needs_redraw();
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                let (px, py) = (cx.mouse().cursorx, cx.mouse().cursory);
                let layout = self.layout(cx.bounds());
                // The preceding MouseDown already started a drag; a double-click
                // is a different gesture, so drop it before acting.
                self.end_drag(cx);

                let mut curve = self.begin_editing(cx);
                match self.hit(&curve, &layout, px, py) {
                    // The ends are the cycle itself and cannot be removed.
                    Hover::Point(index) => {
                        curve.remove(index);
                    }
                    // Straightens the segment back out.
                    Hover::Segment(index) => curve.set_tension(index, 0.0),
                    Hover::None => {
                        let (x, y) = layout.from_px(px, py);
                        curve.insert(x, y);
                    }
                }
                self.curve.store(curve);
                self.hover = Hover::None;
                cx.needs_redraw();
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                self.end_drag(cx);
                meta.consume();
            }
            WindowEvent::MouseMove(px, py) => {
                let layout = self.layout(cx.bounds());
                let mut curve = self.curve.load();
                match self.drag {
                    Some(Grip::Point(index)) => {
                        let (x, y) = layout.from_px(*px, *py);
                        curve.move_point(index, x, y);
                        self.curve.store(curve);
                        cx.needs_redraw();
                        meta.consume();
                    }
                    Some(Grip::Bend(index)) => {
                        let (_, y) = layout.from_px(*px, *py);
                        if let Some(tension) = Self::bend_towards(&curve, index, y) {
                            curve.set_tension(index, tension);
                            self.curve.store(curve);
                            cx.needs_redraw();
                        }
                        meta.consume();
                    }
                    None => {
                        // Only redraw when the answer changed: a mouse move fires
                        // far more often than the highlight actually moves.
                        let hover = if self.is_editable() {
                            self.hit(&self.displayed_curve(), &layout, *px, *py)
                        } else {
                            Hover::None
                        };
                        if hover != self.hover {
                            self.hover = hover;
                            cx.needs_redraw();
                        }
                    }
                }
            }
            WindowEvent::MouseLeave => {
                if self.hover != Hover::None {
                    self.hover = Hover::None;
                    cx.needs_redraw();
                }
            }
            // No MouseOut handler: the pointer is expected to leave the well
            // while dragging, and `cx.capture()` keeps delivering events anyway.
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 4.0 || bounds.h <= 4.0 {
            return;
        }

        let mut path = cx.build_path();
        // The default View::draw runs shadows before the background; overriding
        // draw means doing that here too, or the CSS box-shadow never appears.
        cx.draw_shadows(canvas, &mut path);
        cx.draw_background(canvas, &mut path);

        let opacity = cx.opacity();
        let layout = self.layout(bounds);
        let curve = self.displayed_curve();
        let editable = self.is_editable();

        let mut accent: vg::Color = cx.selection_color().into();
        accent.set_alphaf(accent.a * opacity);
        let mut fill: vg::Color = cx.font_color().into();
        fill.set_alphaf(fill.a * opacity);

        // The trace, sampled from the same function the audio path reads.
        let mut trace = vg::Path::new();
        for step in 0..=TRACE_POINTS {
            let x = step as f32 / TRACE_POINTS as f32;
            let (px, py) = layout.to_px(x, curve.sample(x));
            if step == 0 {
                trace.move_to(px, py);
            } else {
                trace.line_to(px, py);
            }
        }

        // Wide translucent pass under a crisp one, matching the design's glow.
        let mut glow = vg::Paint::color(vg::Color::rgbaf(accent.r, accent.g, accent.b, accent.a * 0.25));
        glow.set_line_width(5.0);
        glow.set_line_cap(vg::LineCap::Round);
        glow.set_line_join(vg::LineJoin::Round);
        canvas.stroke_path(&trace, &glow);

        let mut paint = vg::Paint::color(accent);
        paint.set_line_width(1.8);
        paint.set_line_cap(vg::LineCap::Round);
        paint.set_line_join(vg::LineJoin::Round);
        canvas.stroke_path(&trace, &paint);

        // A built-in shape has no breakpoints to show; the trace is the whole
        // picture until the player edits it.
        if !editable {
            return;
        }

        // The hovered segment's bend handle. Only one is ever shown: the design
        // is a clean well, and four handles would be four things to misread.
        let handle_segment = match (self.drag, self.hover) {
            (Some(Grip::Bend(index)), _) => Some(index),
            (None, Hover::Segment(index)) => Some(index),
            _ => None,
        };
        if let Some(segment) = handle_segment {
            if let Some((hx, hy)) = self.handle_position(&curve, &layout, segment) {
                let mut path = vg::Path::new();
                path.circle(hx, hy, HANDLE_RADIUS);
                canvas.fill_path(&path, &vg::Paint::color(accent));
                let mut ring = vg::Path::new();
                ring.circle(hx, hy, HANDLE_GRAB_RADIUS);
                let mut paint =
                    vg::Paint::color(vg::Color::rgbaf(accent.r, accent.g, accent.b, accent.a * 0.35));
                paint.set_line_width(1.2);
                canvas.stroke_path(&ring, &paint);
            }
        }

        // Breakpoints: filled while held or hovered, hollow otherwise.
        for (index, point) in curve.points().iter().enumerate() {
            let (px, py) = layout.to_px(point.x, point.y);
            let lit = self.drag == Some(Grip::Point(index)) || self.hover == Hover::Point(index);
            let mut path = vg::Path::new();
            path.circle(px, py, if lit { POINT_RADIUS + 1.0 } else { POINT_RADIUS });
            if lit {
                canvas.fill_path(&path, &vg::Paint::color(accent));
            } else {
                canvas.fill_path(&path, &vg::Paint::color(fill));
                let mut paint = vg::Paint::color(accent);
                paint.set_line_width(1.6);
                canvas.stroke_path(&path, &paint);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> Layout {
        Layout { left: 10.0, top: 20.0, width: 200.0, height: 100.0 }
    }

    #[test]
    fn pixels_and_curve_space_are_inverses() {
        let layout = layout();
        for (x, y) in [(0.0, 0.0), (1.0, 1.0), (0.5, 0.25), (0.3, 0.9)] {
            let (px, py) = layout.to_px(x, y);
            let (back_x, back_y) = layout.from_px(px, py);
            assert!((back_x - x).abs() < 1e-5, "x {x} -> {px} -> {back_x}");
            assert!((back_y - y).abs() < 1e-5, "y {y} -> {py} -> {back_y}");
        }
        // The top of the well is level 1.0, not 0.0.
        assert!(layout.to_px(0.0, 1.0).1 < layout.to_px(0.0, 0.0).1);
    }

    #[test]
    fn bending_puts_the_midpoint_where_the_pointer_asked() {
        let mut curve = LfoCurve::from_points(&[
            ds_dsp::CurvePoint::new(0.0, 0.0),
            ds_dsp::CurvePoint::new(0.5, 1.0),
            ds_dsp::CurvePoint::new(1.0, 0.0),
        ]);
        for target in [0.2, 0.35, 0.5, 0.7, 0.9] {
            let tension = LfoEditor::bend_towards(&curve, 0, target).expect("segment has a rise");
            curve.set_tension(0, tension);
            let midpoint = curve.sample(0.25);
            assert!((midpoint - target).abs() < 0.02, "asked for {target}, curve gave {midpoint}");
        }
    }

    #[test]
    fn a_flat_segment_has_no_bend_to_adjust() {
        // Bending a segment whose ends are level does nothing to the drawn curve,
        // so there is deliberately no handle and no solution to hand back.
        let curve = LfoCurve::from_points(&[
            ds_dsp::CurvePoint::new(0.0, 0.5),
            ds_dsp::CurvePoint::new(0.5, 0.5),
            ds_dsp::CurvePoint::new(1.0, 0.5),
        ]);
        assert_eq!(LfoEditor::bend_towards(&curve, 0, 0.9), None);
    }

    #[test]
    fn bending_stays_finite_at_the_extremes() {
        let curve = LfoCurve::from_points(&[
            ds_dsp::CurvePoint::new(0.0, 0.0),
            ds_dsp::CurvePoint::new(0.5, 1.0),
            ds_dsp::CurvePoint::new(1.0, 0.0),
        ]);
        // Dragging the handle right off the top or bottom of the well.
        for target in [-5.0, 0.0, 1.0, 5.0] {
            let tension = LfoEditor::bend_towards(&curve, 0, target).expect("segment has a rise");
            assert!(tension.is_finite(), "target {target} gave {tension}");
            assert!((-1.0..=1.0).contains(&tension), "target {target} left range: {tension}");
        }
    }
}
