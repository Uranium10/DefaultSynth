//! Small labelled dot toggles, and the tab strips built from the same shape.
//!
//! The design uses a dot-plus-caption for the filter input sources (A / B / C /
//! N), the LFO sync flags (BPM / TRIP / ANCH / DOT) and the voicing modes, so
//! they all share one widget.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

/// A dot that lights up, with its caption beside it.
pub struct RadioDot {
    param_base: ParamWidgetBase,
}

impl RadioDot {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        label: &'static str,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self { param_base: ParamWidgetBase::new(cx, params, params_to_param) }.build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, _data| {
                // The dot itself is painted by this view's own draw; the caption is
                // a normal label so it picks up the stylesheet's font settings.
                Element::new(cx).class("radio-dot-mark").hoverable(false);
                Label::new(cx, label).class("radio-dot-label").hoverable(false);
            }),
        )
    }

    fn is_on(&self) -> bool {
        self.param_base.unmodulated_normalized_value() >= 0.5
    }
}

impl View for RadioDot {
    fn element(&self) -> Option<&'static str> {
        Some("radio-dot")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                let next = if self.is_on() { 0.0 } else { 1.0 };
                self.param_base.begin_set_parameter(cx);
                self.param_base.set_normalized_value(cx, next);
                self.param_base.end_set_parameter(cx);
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
        let mut fill: vg::Color = if self.is_on() {
            cx.selection_color().into()
        } else {
            cx.background_color().into()
        };
        fill.set_alphaf(fill.a * opacity);

        // The mark sits at the left edge, vertically centred, matching the design.
        let radius = (bounds.h * 0.30).min(7.0);
        let mut path = vg::Path::new();
        path.circle(bounds.x + radius + 1.0, bounds.y + bounds.h / 2.0, radius);
        canvas.fill_path(&path, &vg::Paint::color(fill));
    }
}

