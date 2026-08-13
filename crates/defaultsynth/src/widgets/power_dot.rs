//! The lit/unlit circle in front of every panel title.
//!
//! `ParamButton` draws the parameter's name inside itself, which collides with
//! the panel heading it sits next to. This is just the dot.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

pub struct PowerDot {
    param_base: ParamWidgetBase,
}

impl PowerDot {
    pub fn new<L, Params, P, FMap>(cx: &mut Context, params: L, params_to_param: FMap) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self { param_base: ParamWidgetBase::new(cx, params, params_to_param) }
            .build(cx, ParamWidgetBase::build_view(params, params_to_param, move |_cx, _data| {}))
    }

    fn is_on(&self) -> bool {
        self.param_base.unmodulated_normalized_value() >= 0.5
    }

    fn toggle(&self, cx: &mut EventContext) {
        let next = if self.is_on() { 0.0 } else { 1.0 };
        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, next);
        self.param_base.end_set_parameter(cx);
    }
}

impl View for PowerDot {
    fn element(&self) -> Option<&'static str> {
        Some("power-dot")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                self.toggle(cx);
                meta.consume();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 1.0 || bounds.h <= 1.0 {
            return;
        }
        let opacity = cx.opacity();
        // selection-color is the lit state, background-color the unlit one.
        let mut fill: vg::Color = if self.is_on() { cx.selection_color().into() } else { cx.background_color().into() };
        fill.set_alphaf(fill.a * opacity);

        let radius = bounds.w.min(bounds.h) / 2.0;
        let mut path = vg::Path::new();
        path.circle(bounds.x + bounds.w / 2.0, bounds.y + bounds.h / 2.0, radius);
        canvas.fill_path(&path, &vg::Paint::color(fill));
    }
}
