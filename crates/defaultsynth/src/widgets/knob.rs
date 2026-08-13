//! Circular knob matching the reference design.
//!
//! The design's knob is a pale dial with a cyan arc tracking the value and a
//! short pointer line. VIZIA has no built-in rotary control, so this draws
//! straight onto the canvas and drives the parameter through `ParamWidgetBase`.

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;

/// Dial sweep, in degrees, centred on straight up. 270° is the DAW convention.
const SWEEP_DEGREES: f32 = 270.0;
/// Vertical pixels of travel needed to cross the full range.
const DRAG_RANGE_PX: f32 = 200.0;
/// Fine-drag divisor while Shift is held.
const GRANULAR_MULTIPLIER: f32 = 0.1;

#[derive(Clone, Copy)]
struct DragState {
    /// Pointer Y when the drag (or the last modifier change) began.
    anchor_y: f32,
    /// Normalised value at that same moment.
    anchor_value: f32,
    granular: bool,
}

pub struct Knob {
    param_base: ParamWidgetBase,
    drag: Option<DragState>,
}

impl Knob {
    pub fn new<L, Params, P, FMap>(cx: &mut Context, params: L, params_to_param: FMap) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: nih_plug::prelude::Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param_base: ParamWidgetBase::new(cx, params, params_to_param),
            drag: None,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |_cx, _param_data| {}),
        )
    }

    /// Re-anchors on modifier changes so toggling Shift adjusts from the current
    /// position instead of rescaling the whole gesture and jumping the value.
    fn drag_to(&mut self, cx: &mut EventContext, y: f32, granular: bool) {
        let Some(drag) = self.drag.as_mut() else { return };
        if drag.granular != granular {
            drag.granular = granular;
            drag.anchor_y = y;
            drag.anchor_value = self.param_base.unmodulated_normalized_value();
        }
        let multiplier = if granular { GRANULAR_MULTIPLIER } else { 1.0 };
        // Up increases, which is why the delta is inverted.
        let delta = (drag.anchor_y - y) / DRAG_RANGE_PX * multiplier;
        let value = (drag.anchor_value + delta).clamp(0.0, 1.0);
        self.param_base.set_normalized_value(cx, value);
    }

    fn begin_drag(&mut self, cx: &mut EventContext, y: f32, granular: bool) {
        cx.capture();
        cx.set_active(true);
        self.param_base.begin_set_parameter(cx);
        self.drag = Some(DragState {
            anchor_y: y,
            anchor_value: self.param_base.unmodulated_normalized_value(),
            granular,
        });
    }

    fn end_drag(&mut self, cx: &mut EventContext) {
        if self.drag.take().is_some() {
            self.param_base.end_set_parameter(cx);
            cx.release();
            cx.set_active(false);
        }
    }

    fn reset_to_default(&self, cx: &mut EventContext) {
        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, self.param_base.default_normalized_value());
        self.param_base.end_set_parameter(cx);
    }
}

impl View for Knob {
    fn element(&self) -> Option<&'static str> {
        Some("knob")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                // Ctrl-click means "give me the default back", as in most DAWs.
                if cx.modifiers().command() {
                    self.reset_to_default(cx);
                } else {
                    self.begin_drag(cx, cx.mouse().cursory, cx.modifiers().shift());
                }
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                // A double-click arrives after the first MouseDown, so the drag it
                // started has to be torn down before the reset lands.
                self.end_drag(cx);
                self.reset_to_default(cx);
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Right) => {
                self.reset_to_default(cx);
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                self.end_drag(cx);
                meta.consume();
            }
            WindowEvent::MouseMove(_, y) => {
                if self.drag.is_some() {
                    self.drag_to(cx, *y, cx.modifiers().shift());
                    meta.consume();
                }
            }
            WindowEvent::MouseScroll(_, delta_y) => {
                // A scroll step is one parameter step for discrete parameters and a
                // small nudge for continuous ones.
                let current = self.param_base.unmodulated_normalized_value();
                let finer = cx.modifiers().shift();
                let next = if *delta_y > 0.0 {
                    self.param_base.next_normalized_step(current, finer)
                } else {
                    self.param_base.previous_normalized_step(current, finer)
                };
                self.param_base.begin_set_parameter(cx);
                self.param_base.set_normalized_value(cx, next);
                self.param_base.end_set_parameter(cx);
                meta.consume();
            }
            // The pointer can leave the window mid-gesture; without this the knob
            // would stay latched to the mouse after the button is released.
            WindowEvent::MouseOut => self.end_drag(cx),
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 0.0 || bounds.h <= 0.0 {
            return;
        }

        let opacity = cx.opacity();
        let value = self.param_base.unmodulated_normalized_value().clamp(0.0, 1.0);
        let default = self.param_base.default_normalized_value().clamp(0.0, 1.0);

        // Keep the dial round inside whatever box the layout gave us.
        let diameter = bounds.w.min(bounds.h);
        let centre_x = bounds.x + bounds.w / 2.0;
        let centre_y = bounds.y + bounds.h / 2.0;
        let arc_radius = diameter / 2.0 - 1.0;
        let body_radius = arc_radius - 4.0;
        if body_radius <= 1.0 {
            return;
        }

        let sweep = SWEEP_DEGREES.to_radians();
        // Straight up is -90°; the arc starts half a sweep anticlockwise of that.
        let start_angle = -std::f32::consts::FRAC_PI_2 - sweep / 2.0;
        let value_angle = start_angle + sweep * value;

        let track = colour(cx.border_color(), opacity);
        let accent = colour(cx.selection_color(), opacity);
        let body = colour(cx.background_color(), opacity);
        let pointer = colour(cx.font_color(), opacity);

        // Unfilled track behind the value arc.
        let mut path = vg::Path::new();
        path.arc(centre_x, centre_y, arc_radius, start_angle, start_angle + sweep, vg::Solidity::Hole);
        let mut paint = vg::Paint::color(track);
        paint.set_line_width(3.0);
        paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&path, &paint);

        // Value arc. Bipolar parameters read far better filling out from their
        // centre detent than from the far left, so anchor on the default.
        let bipolar = (default - 0.5).abs() < 0.02;
        let arc_from = if bipolar { start_angle + sweep * 0.5 } else { start_angle };
        if (value_angle - arc_from).abs() > 1e-4 {
            let mut path = vg::Path::new();
            let (from, to, solidity) = if value_angle >= arc_from {
                (arc_from, value_angle, vg::Solidity::Hole)
            } else {
                (value_angle, arc_from, vg::Solidity::Hole)
            };
            path.arc(centre_x, centre_y, arc_radius, from, to, solidity);
            let mut paint = vg::Paint::color(accent);
            paint.set_line_width(3.0);
            paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&path, &paint);
        }

        // Dial body.
        let mut path = vg::Path::new();
        path.circle(centre_x, centre_y, body_radius);
        canvas.fill_path(&path, &vg::Paint::color(body));

        // Pointer line from just outside the hub to the rim.
        let mut path = vg::Path::new();
        let (sin, cos) = value_angle.sin_cos();
        path.move_to(centre_x + cos * body_radius * 0.28, centre_y + sin * body_radius * 0.28);
        path.line_to(centre_x + cos * body_radius * 0.86, centre_y + sin * body_radius * 0.86);
        let mut paint = vg::Paint::color(pointer);
        paint.set_line_width(2.0);
        paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&path, &paint);
    }
}

fn colour(source: impl Into<vg::Color>, opacity: f32) -> vg::Color {
    let mut colour: vg::Color = source.into();
    colour.set_alphaf(colour.a * opacity);
    colour
}
