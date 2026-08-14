//! Dropdown for the design's long grey selector boxes.
//!
//! The waveform, modulation source, noise colour, filter mode and LFO trigger
//! all pick one of a handful of named values, which reads as a dropdown rather
//! than as something you scrub. The entries come from the parameter itself:
//! a discrete parameter reports how many steps it has, and each step formats
//! itself, so a new enum variant appears here without any extra wiring.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

/// Normalised positions and captions for every step of a discrete parameter.
fn entries(param: &ParamWidgetBase) -> Vec<(f32, String)> {
    // `step_count` is one less than the number of values, matching the
    // normalised range's endpoints.
    let Some(steps) = param.step_count() else { return Vec::new() };
    (0..=steps)
        .map(|index| {
            let normalized = index as f32 / steps.max(1) as f32;
            (normalized, param.normalized_value_to_string(normalized, false))
        })
        .collect()
}

pub struct ParamDropdown {
    param_base: ParamWidgetBase,
}

impl ParamDropdown {
    pub fn new<L, Params, P, FMap>(cx: &mut Context, params: L, params_to_param: FMap) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self { param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param) }.build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                let options = entries(&ParamWidgetBase::new(cx, params, params_to_param));

                Dropdown::new(
                    cx,
                    // Closed state: the current value, formatted by the parameter.
                    move |cx| {
                        Label::new(
                            cx,
                            param_data.make_lens(|param| {
                                param.normalized_value_to_string(param.unmodulated_normalized_value(), false)
                            }),
                        )
                        .class("dropdown-value")
                    },
                    // Open state: one row per value.
                    move |cx| {
                        VStack::new(cx, |cx| {
                            for (normalized, caption) in &options {
                                let normalized = *normalized;
                                Label::new(cx, caption.as_str())
                                    .class("dropdown-item")
                                    .on_press(move |cx| {
                                        cx.emit(ParamDropdownEvent::Select(normalized));
                                        cx.emit(PopupEvent::Close);
                                    });
                            }
                        })
                        .class("dropdown-list");
                    },
                )
                .class("dropdown-root");
            }),
        )
    }
}

enum ParamDropdownEvent {
    Select(f32),
}

impl View for ParamDropdown {
    fn element(&self) -> Option<&'static str> {
        Some("param-dropdown")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|dropdown_event, meta| {
            let ParamDropdownEvent::Select(normalized) = dropdown_event;
            // A picked value is one gesture, so it is bracketed like any other edit.
            self.param_base.begin_set_parameter(cx);
            self.param_base.set_normalized_value(cx, *normalized);
            self.param_base.end_set_parameter(cx);
            meta.consume();
        });
    }
}
