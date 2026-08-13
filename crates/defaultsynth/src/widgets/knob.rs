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
            // Deliberately no MouseOut handler. `cx.capture()` already routes every
            // mouse event here for the duration of the gesture, and MouseOut fires
            // the moment the pointer crosses the knob's own edge, so releasing on
            // it would cut the drag short after a few pixels of travel.
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

        // Dial body. The design shades each dial with a diagonal three-stop
        // gradient running from the lower right up to the upper left, which is
        // what gives it its brushed-metal look; a flat fill reads as a dead disc.
        let mut path = vg::Path::new();
        path.circle(centre_x, centre_y, body_radius);
        let from = (centre_x + body_radius, centre_y + body_radius);
        let to = (centre_x - body_radius, centre_y - body_radius);
        let paint = vg::Paint::linear_gradient_stops(
            from.0,
            from.1,
            to.0,
            to.1,
            [
                (0.0, shade(body, DIAL_GRADIENT[0], opacity)),
                (0.5, shade(body, DIAL_GRADIENT[1], opacity)),
                (1.0, shade(body, DIAL_GRADIENT[2], opacity)),
            ],
        );
        canvas.fill_path(&path, &paint);

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

/// The dial's three gradient stops, taken from Synth.svg (`#CACACA`, `#DDDDDD`,
/// `#EEEEEE`), expressed relative to the lightest one so the CSS
/// `background-color` still sets the dial's overall tone.
const DIAL_GRADIENT: [f32; 3] = [0xCA as f32 / 0xEE as f32, 0xDD as f32 / 0xEE as f32, 1.0];

fn colour(source: impl Into<vg::Color>, opacity: f32) -> vg::Color {
    let mut colour: vg::Color = source.into();
    colour.set_alphaf(colour.a * opacity);
    colour
}

/// Scales a colour's brightness, keeping its hue.
fn shade(base: vg::Color, factor: f32, opacity: f32) -> vg::Color {
    vg::Color::rgbaf(
        (base.r * factor).clamp(0.0, 1.0),
        (base.g * factor).clamp(0.0, 1.0),
        (base.b * factor).clamp(0.0, 1.0),
        base.a * opacity,
    )
}
