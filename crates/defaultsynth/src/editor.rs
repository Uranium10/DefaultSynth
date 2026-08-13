//! VIZIA editor.
//!
//! This is the panel skeleton for the reference design: title bar, the three OSC
//! panels, noise, both filters, the amp envelope and voicing. Custom-drawn
//! widgets (the waveform wells, envelope and LFO editors, the circular knobs)
//! come next; every control here is already bound to a real parameter so the
//! layout can be checked against a host while the visuals are built up.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::params::{DefaultSynthParams, OscParams};
use crate::widgets::{EnvelopeDisplay, Knob, WaveDisplay};

/// Matches the 4:3-ish proportions of the reference design.
pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (1180, 860))
}

#[derive(Lens)]
struct EditorData {
    params: Arc<DefaultSynthParams>,
}

impl Model for EditorData {}

pub fn create(params: Arc<DefaultSynthParams>, state: Arc<ViziaState>) -> Option<Box<dyn Editor>> {
    create_vizia_editor(state, ViziaTheming::Custom, move |cx, _| {
        assets::register_noto_sans_light(cx);
        assets::register_noto_sans_thin(cx);
        cx.add_stylesheet(include_style!("src/theme.css"))
            .expect("the bundled stylesheet should always parse");

        EditorData { params: params.clone() }.build(cx);

        VStack::new(cx, |cx| {
            title_bar(cx);
            HStack::new(cx, |cx| {
                VStack::new(cx, |cx| {
                    osc_panel(cx, "OSC A", |params| &params.osc_a);
                    osc_panel(cx, "OSC B", |params| &params.osc_b);
                    osc_panel(cx, "OSC C", |params| &params.osc_c);
                })
                .row_between(Pixels(8.0))
                .width(Percentage(54.0));

                VStack::new(cx, |cx| {
                    amp_env_panel(cx);
                    voicing_panel(cx);
                })
                .row_between(Pixels(8.0))
                .width(Stretch(1.0));
            })
            .col_between(Pixels(8.0));

            HStack::new(cx, |cx| {
                noise_panel(cx);
                filter_panel(cx, "FILTER A", true);
                filter_panel(cx, "FILTER B", false);
            })
            .col_between(Pixels(8.0))
            .height(Pixels(200.0));
        })
        .class("synth-root");
    })
}

fn title_bar(cx: &mut Context) {
    HStack::new(cx, |cx| {
        Element::new(cx).class("brand-mark");
        VStack::new(cx, |cx| {
            Label::new(cx, "DefaultSynth").class("brand-name");
            Element::new(cx).class("brand-underline");
        })
        .row_between(Pixels(4.0))
        .width(Auto);

        // Spacer pushes the master control to the right edge.
        Element::new(cx).width(Stretch(1.0));

        VStack::new(cx, |cx| {
            Label::new(cx, "MASTER").class("knob-label");
            ParamSlider::new(cx, EditorData::params, |params| &params.master_gain).class("param-slider");
        })
        .width(Pixels(150.0))
        .row_between(Pixels(3.0));
    })
    .class("title-bar")
    .height(Pixels(54.0));
}

/// One OSC panel. `select` picks which of the three parameter groups it drives.
fn osc_panel(
    cx: &mut Context,
    title: &str,
    select: impl Fn(&Arc<DefaultSynthParams>) -> &OscParams + Copy + 'static,
) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            ParamButton::new(cx, EditorData::params, move |params| &select(params).enabled)
                .class("power-dot")
                .width(Pixels(17.0))
                .height(Pixels(17.0));
            Label::new(cx, title).class("panel-title");
            Element::new(cx).width(Stretch(1.0));
            ParamSlider::new(cx, EditorData::params, move |params| &select(params).waveform)
                .class("selector")
                .width(Pixels(120.0));
        })
        .class("panel-header")
        .col_between(Pixels(8.0));

        // Waveform well on the left, dials on the right, as in the design.
        HStack::new(cx, |cx| {
            WaveDisplay::new(
                cx,
                EditorData::params.map(move |params| select(params).waveform.value().to_dsp()),
                EditorData::params.map(move |params| select(params).warp.value()),
            )
            .class("display-well")
            .width(Pixels(196.0));

            VStack::new(cx, move |cx| {
                HStack::new(cx, move |cx| {
                    readout(cx, "OCT", move |params| &select(params).octave);
                    readout(cx, "FINE", move |params| &select(params).fine);
                    readout(cx, "UNISON", move |params| &select(params).unison);
                })
                .col_between(Pixels(4.0))
                .height(Auto);

                HStack::new(cx, move |cx| {
                    knob_cell(cx, "DETUNE", move |params| &select(params).detune);
                    knob_cell(cx, "BLEND", move |params| &select(params).blend);
                    knob_cell(cx, "WARP", move |params| &select(params).warp);
                    knob_cell(cx, "PHASE", move |params| &select(params).phase);
                })
                .col_between(Pixels(2.0))
                .height(Auto);

                HStack::new(cx, move |cx| {
                    knob_cell(cx, "RAND", move |params| &select(params).phase_random);
                    knob_cell(cx, "PAN", move |params| &select(params).pan);
                    knob_cell(cx, "VOLUME", move |params| &select(params).level);
                    knob_cell(cx, "FILTER", move |params| &select(params).filter_send);
                })
                .col_between(Pixels(2.0))
                .height(Auto);
            })
            .row_between(Pixels(6.0))
            .width(Stretch(1.0));
        })
        .col_between(Pixels(9.0))
        .height(Stretch(1.0));
    })
    .class("panel");
}

/// Compact inset numeric field, like the design's OCT / FINE / UNISON boxes.
fn readout<P, FMap>(cx: &mut Context, label: &'static str, select: FMap)
where
    P: nih_plug::prelude::Param + 'static,
    FMap: Fn(&Arc<DefaultSynthParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        ParamSlider::new(cx, EditorData::params, select).class("readout");
        Label::new(cx, label).class("readout-label");
    })
    .row_between(Pixels(2.0))
    .width(Stretch(1.0))
    .height(Auto);
}

fn filter_panel(cx: &mut Context, title: &str, is_a: bool) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            if is_a {
                ParamButton::new(cx, EditorData::params, |params| &params.filter_a.enabled).class("power-dot");
            } else {
                ParamButton::new(cx, EditorData::params, |params| &params.filter_b.enabled).class("power-dot");
            }
            Label::new(cx, title).class("panel-title");
        })
        .class("panel-header")
        .col_between(Pixels(8.0));

        // The two filters carry identical controls; the branch only picks the group.
        if is_a {
            filter_controls(cx, |params| &params.filter_a.mode, |params| &params.filter_a.cutoff, |params| &params.filter_a.resonance, |params| &params.filter_a.env_amount);
        } else {
            filter_controls(cx, |params| &params.filter_b.mode, |params| &params.filter_b.cutoff, |params| &params.filter_b.resonance, |params| &params.filter_b.env_amount);
            // F1 exists only on filter B: it is the series-routing input that takes
            // filter A's output. Filter A has no equivalent, since feeding a filter
            // its own output would be a loop.
            labelled(cx, "INPUT F1", |cx| {
                ParamButton::new(cx, EditorData::params, |params| &params.filter_b.input_from_filter_a)
                    .class("param-slider");
            });
        }
    })
    .class("panel")
    .width(Stretch(1.0));
}

fn filter_controls(
    cx: &mut Context,
    mode: impl Fn(&Arc<DefaultSynthParams>) -> &EnumParam<crate::params::FilterModeParam> + Copy + 'static,
    cutoff: impl Fn(&Arc<DefaultSynthParams>) -> &FloatParam + Copy + 'static,
    resonance: impl Fn(&Arc<DefaultSynthParams>) -> &FloatParam + Copy + 'static,
    env_amount: impl Fn(&Arc<DefaultSynthParams>) -> &FloatParam + Copy + 'static,
) {
    VStack::new(cx, move |cx| {
        ParamSlider::new(cx, EditorData::params, mode).class("selector");
        HStack::new(cx, move |cx| {
            knob_cell(cx, "CUT", cutoff);
            knob_cell(cx, "RES", resonance);
            knob_cell(cx, "ENV", env_amount);
        })
        .col_between(Pixels(2.0))
        .height(Auto);
    })
    .row_between(Pixels(6.0));
}

fn noise_panel(cx: &mut Context) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            ParamButton::new(cx, EditorData::params, |params| &params.noise.enabled).class("power-dot");
            Label::new(cx, "NOISE").class("panel-title");
        })
        .class("panel-header")
        .col_between(Pixels(8.0));

        ParamSlider::new(cx, EditorData::params, |params| &params.noise.colour).class("selector");
        HStack::new(cx, |cx| {
            knob_cell(cx, "LEVEL", |params| &params.noise.level);
            knob_cell(cx, "PAN", |params| &params.noise.pan);
        })
        .col_between(Pixels(2.0))
        .height(Auto);
    })
    .class("panel")
    .width(Stretch(1.0));
}

fn amp_env_panel(cx: &mut Context) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            Label::new(cx, "ENV 1 · AMP").class("panel-title");
        })
        .class("panel-header");

        EnvelopeDisplay::new(cx, EditorData::params, |params| &params.amp_env)
            .class("display-well")
            .height(Stretch(1.0));

        HStack::new(cx, |cx| {
            knob_cell(cx, "ATTACK", |params| &params.amp_env.attack);
            knob_cell(cx, "HOLD", |params| &params.amp_env.hold);
            knob_cell(cx, "DECAY", |params| &params.amp_env.decay);
            knob_cell(cx, "SUSTAIN", |params| &params.amp_env.sustain);
            knob_cell(cx, "RELEASE", |params| &params.amp_env.release);
        })
        .col_between(Pixels(2.0))
        .height(Auto);
    })
    .class("panel");
}

fn voicing_panel(cx: &mut Context) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            Label::new(cx, "VOICING").class("panel-title");
        })
        .class("panel-header");

        HStack::new(cx, |cx| {
            labelled(cx, "MODE", |cx| {
                ParamSlider::new(cx, EditorData::params, |params| &params.voicing.mode).class("selector");
            });
            readout(cx, "POLY", |params| &params.voicing.polyphony);
            labelled(cx, "ALWAYS", |cx| {
                ParamButton::new(cx, EditorData::params, |params| &params.voicing.always_glide).class("param-slider");
            });
        })
        .col_between(Pixels(6.0))
        .height(Auto);

        HStack::new(cx, |cx| {
            knob_cell(cx, "PORTA", |params| &params.voicing.portamento);
            knob_cell(cx, "VELO", |params| &params.voicing.velocity_curve);
            knob_cell(cx, "MASTER", |params| &params.master_gain);
        })
        .col_between(Pixels(2.0))
        .height(Auto);
    })
    .class("panel");
}

/// Caption-above-control cell, the layout used throughout the design.
fn labelled(cx: &mut Context, label: &str, content: impl Fn(&mut Context) + 'static) {
    VStack::new(cx, move |cx| {
        Label::new(cx, label).class("knob-label");
        content(cx);
    })
    .row_between(Pixels(3.0))
    .width(Stretch(1.0));
}

/// Dial with its readout and caption underneath, as drawn throughout the design.
fn knob_cell<P, FMap>(cx: &mut Context, label: &'static str, select: FMap)
where
    P: nih_plug::prelude::Param + 'static,
    FMap: Fn(&Arc<DefaultSynthParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        Knob::new(cx, EditorData::params, select).class("dial");
        // The readout tracks the parameter's own formatter, so it picks up units
        // and the custom pan/gain strings for free.
        Label::new(
            cx,
            EditorData::params.map(move |params| {
                let param = select(params);
                param.normalized_value_to_string(param.unmodulated_normalized_value(), true)
            }),
        )
        .class("knob-value");
        Label::new(cx, label).class("knob-label");
    })
    .class("knob-cell");
}
