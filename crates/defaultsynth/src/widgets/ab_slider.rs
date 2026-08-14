//! The A—B routing slider above each oscillator's filter send.
//!
//! A horizontal track with a square handle and an A and B tick, matching the
//! design. It exists as its own widget because `ParamSlider` fills from one end,
//! which reads as a level rather than as a position between two destinations.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;

const GRANULAR_MULTIPLIER: f32 = 0.2;

pub struct AbSlider {
    param_base: ParamWidgetBase,
    dragging: bool,
}

impl AbSlider {
    pub fn new<L, Params, P, FMap>(cx: &mut Context, params: L, params_to_param: FMap) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self { param_base: ParamWidgetBase::new(cx, params, params_to_param), dragging: false }.build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |_cx, _data| {}),
        )
    }

    /// Maps a pointer x onto the track, so the handle lands under the cursor.
    fn set_from_x(&self, cx: &mut EventContext, x: f32) {
        let bounds = cx.bounds();
        let usable = (bounds.w - HANDLE_W).max(1.0);
        let value = (x - bounds.x - HANDLE_W / 2.0) / usable;
        self.param_base.set_normalized_value(cx, value.clamp(0.0, 1.0));
    }
}

const HANDLE_W: f32 = 11.0;

impl View for AbSlider {
    fn element(&self) -> Option<&'static str> {
        Some("ab-slider")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                cx.capture();
                cx.set_active(true);
                self.dragging = true;
                self.param_base.begin_set_parameter(cx);
                self.set_from_x(cx, cx.mouse().cursorx);
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.dragging {
                    self.dragging = false;
                    self.param_base.end_set_parameter(cx);
                    cx.release();
                    cx.set_active(false);
                }
                meta.consume();
            }
            WindowEvent::MouseMove(x, _) => {
                if self.dragging {
                    if cx.modifiers().shift() {
                        // Fine mode nudges from where it is rather than jumping.
                        let current = self.param_base.unmodulated_normalized_value();
                        let bounds = cx.bounds();
                        let target = (*x - bounds.x - HANDLE_W / 2.0) / (bounds.w - HANDLE_W).max(1.0);
                        let value = current + (target - current) * GRANULAR_MULTIPLIER;
                        self.param_base.set_normalized_value(cx, value.clamp(0.0, 1.0));
                    } else {
                        self.set_from_x(cx, *x);
                    }
                    meta.consume();
                }
            }
            WindowEvent::MouseDown(MouseButton::Right) => {
                self.param_base.begin_set_parameter(cx);
                self.param_base.set_normalized_value(cx, self.param_base.default_normalized_value());
                self.param_base.end_set_parameter(cx);
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= HANDLE_W || bounds.h <= 2.0 {
            return;
        }
        let opacity = cx.opacity();
        let mut track: vg::Color = cx.background_color().into();
        track.set_alphaf(track.a * opacity);
        let mut handle: vg::Color = cx.selection_color().into();
        handle.set_alphaf(handle.a * opacity);

        // Track.
        let track_h = 2.0;
        let mut path = vg::Path::new();
        path.rect(bounds.x, bounds.y + (bounds.h - track_h) / 2.0, bounds.w, track_h);
        canvas.fill_path(&path, &vg::Paint::color(track));

        // End ticks marking the two destinations.
        for x in [bounds.x, bounds.x + bounds.w - 1.0] {
            let mut path = vg::Path::new();
            path.rect(x, bounds.y + bounds.h * 0.25, 1.0, bounds.h * 0.5);
            canvas.fill_path(&path, &vg::Paint::color(track));
        }

        let value = self.param_base.unmodulated_normalized_value().clamp(0.0, 1.0);
        let x = bounds.x + value * (bounds.w - HANDLE_W);
        let mut path = vg::Path::new();
        path.rounded_rect(x, bounds.y, HANDLE_W, bounds.h, 2.0);
        canvas.fill_path(&path, &vg::Paint::color(handle));
    }
}
