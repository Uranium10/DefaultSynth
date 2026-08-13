//! Inset dark fields: the dropdown-style selectors and the small numeric boxes.
//!
//! These replace `ParamSlider`, which paints a step indicator and a fill bar
//! behind its text. Against the design's flat dark boxes that indicator shows up
//! as a stray pale rectangle next to every label, and its text is centred inside
//! a bar rather than in a box. Both of these draw the field themselves and
//! render the value through VIZIA's normal label pipeline.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;

/// Vertical pixels of travel needed to cross a numeric field's whole range.
const DRAG_RANGE_PX: f32 = 160.0;
const GRANULAR_MULTIPLIER: f32 = 0.15;

#[derive(Clone, Copy)]
struct DragState {
    anchor_y: f32,
    anchor_value: f32,
    granular: bool,
    moved: bool,
}

/// A dark field showing a parameter's formatted value.
///
/// Dragging vertically changes the value, the wheel steps it, and for a discrete
/// parameter a plain click advances to the next value, which is what makes the
/// waveform and filter-mode selectors behave like the design's dropdowns.
pub struct Field {
    param_base: ParamWidgetBase,
    drag: Option<DragState>,
    /// Discrete parameters cycle on click; continuous ones only drag.
    cycles_on_click: bool,
}

impl Field {
    pub fn new<L, Params, P, FMap>(cx: &mut Context, params: L, params_to_param: FMap) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        let param_base = ParamWidgetBase::new(cx, params.clone(), params_to_param);
        // A handful of steps means a picker; a long ramp means a drag field.
        let cycles_on_click = matches!(param_base.step_count(), Some(count) if count <= 16);
        Self { param_base, drag: None, cycles_on_click }.build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                Label::new(
                    cx,
                    param_data.make_lens(|param| {
                        param.normalized_value_to_string(param.unmodulated_normalized_value(), true)
                    }),
                )
                .class("field-text")
                .hoverable(false);
            }),
        )
    }

    fn set(&self, cx: &mut EventContext, normalized: f32) {
        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, normalized.clamp(0.0, 1.0));
        self.param_base.end_set_parameter(cx);
    }
}

impl View for Field {
    fn element(&self) -> Option<&'static str> {
        Some("field")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                if cx.modifiers().command() {
                    self.set(cx, self.param_base.default_normalized_value());
                } else {
                    cx.capture();
                    cx.set_active(true);
                    self.param_base.begin_set_parameter(cx);
                    self.drag = Some(DragState {
                        anchor_y: cx.mouse().cursory,
                        anchor_value: self.param_base.unmodulated_normalized_value(),
                        granular: cx.modifiers().shift(),
                        moved: false,
                    });
                }
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if let Some(drag) = self.drag.take() {
                    self.param_base.end_set_parameter(cx);
                    cx.release();
                    cx.set_active(false);
                    // A click that never moved advances a picker to its next value.
                    if !drag.moved && self.cycles_on_click {
                        let current = self.param_base.unmodulated_normalized_value();
                        let next = self.param_base.next_normalized_step(current, false);
                        // Wrap around rather than sticking at the last entry.
                        let next = if (next - current).abs() < f32::EPSILON { 0.0 } else { next };
                        self.set(cx, next);
                    }
                }
                meta.consume();
            }
            WindowEvent::MouseMove(_, y) => {
                let Some(mut drag) = self.drag else { return };
                let granular = cx.modifiers().shift();
                if drag.granular != granular {
                    drag.granular = granular;
                    drag.anchor_y = *y;
                    drag.anchor_value = self.param_base.unmodulated_normalized_value();
                }
                let travel = drag.anchor_y - *y;
                if travel.abs() > 2.0 {
                    drag.moved = true;
                }
                let multiplier = if granular { GRANULAR_MULTIPLIER } else { 1.0 };
                let value = drag.anchor_value + travel / DRAG_RANGE_PX * multiplier;
                self.param_base.set_normalized_value(cx, value.clamp(0.0, 1.0));
                self.drag = Some(drag);
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Right) => {
                self.set(cx, self.param_base.default_normalized_value());
                meta.consume();
            }
            WindowEvent::MouseScroll(_, delta_y) => {
                let current = self.param_base.unmodulated_normalized_value();
                let finer = cx.modifiers().shift();
                let next = if *delta_y > 0.0 {
                    self.param_base.next_normalized_step(current, finer)
                } else {
                    self.param_base.previous_normalized_step(current, finer)
                };
                self.set(cx, next);
                meta.consume();
            }
            _ => {}
        });
    }
}
