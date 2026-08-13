//! VIZIA editor.
//!
//! Heights are budgeted explicitly rather than left to `auto`. VIZIA clips
//! overflowing children instead of shrinking them, so every row's size has to
//! add up to the window: 3 OSC panels + the bottom strip must fit inside the
//! main area, or the last row of knobs silently disappears.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::params::{DefaultSynthParams, OscParams};
use crate::widgets::{EnvelopeDisplay, Knob, PowerDot, WaveDisplay};

/// Height of one dial plus its readout and caption.
const KNOB_CELL: f32 = 78.0;
/// Height of one OSC panel. Three of these plus the bottom strip fill the window.
const OSC_PANEL: f32 = 166.0;
/// Height of the NOISE / FILTER strip along the bottom.
const BOTTOM_STRIP: f32 = 160.0;
/// Height of the branding row above the panels.
const TITLE_BAR: f32 = 50.0;

/// Window size.
///
/// Height matters more than it looks: a 125%-scaled 1080p desktop only offers
/// about 816 logical pixels of working area, and asking for more makes the OS
/// clamp the window while VIZIA keeps laying out at the requested size, which
/// silently crops the bottom panels. The panel constants above add up to this.
pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (1280, 760))
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
                .width(Percentage(56.0));

                VStack::new(cx, |cx| {
                    env_panel(cx);
                    voicing_panel(cx);
                })
                .row_between(Pixels(8.0))
                .width(Stretch(1.0));
            })
            .col_between(Pixels(8.0))
            .height(Pixels(OSC_PANEL * 3.0 + 16.0));

            HStack::new(cx, |cx| {
                noise_panel(cx);
                filter_panel(cx, "FILTER A", true);
                filter_panel(cx, "FILTER B", false);
            })
            .col_between(Pixels(8.0))
            .height(Pixels(BOTTOM_STRIP));
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
        .width(Pixels(210.0));

        Element::new(cx).width(Stretch(1.0));

        knob_cell(cx, "MASTER", |params| &params.master_gain);
    })
    .class("title-bar")
    .height(Pixels(TITLE_BAR))
    .col_between(Pixels(12.0));
}

/// Panel heading: the lit dot, the title, and an optional trailing control.
fn panel_header(cx: &mut Context, title: &str, trailing: impl Fn(&mut Context) + 'static) {
    HStack::new(cx, move |cx| {
        Label::new(cx, title).class("panel-title").width(Pixels(76.0));
        Element::new(cx).width(Stretch(1.0));
        trailing(cx);
    })
    .class("panel-header")
    .height(Pixels(24.0))
    .col_between(Pixels(8.0));
}

fn osc_panel(
    cx: &mut Context,
    title: &'static str,
    select: impl Fn(&Arc<DefaultSynthParams>) -> &OscParams + Copy + 'static,
) {
    VStack::new(cx, move |cx| {
        HStack::new(cx, move |cx| {
            PowerDot::new(cx, EditorData::params, move |params| &select(params).enabled)
                .class("power-dot");
            Label::new(cx, title).class("panel-title").width(Pixels(66.0));
            Element::new(cx).width(Stretch(1.0));
            ParamSlider::new(cx, EditorData::params, move |params| &select(params).waveform)
                .set_style(ParamSliderStyle::CurrentStep { even: true })
                .class("selector")
                .width(Pixels(140.0));
        })
        .class("panel-header")
        .height(Pixels(24.0))
        .col_between(Pixels(8.0));

        HStack::new(cx, move |cx| {
            WaveDisplay::new(
                cx,
                EditorData::params.map(move |params| select(params).waveform.value().to_dsp()),
                EditorData::params.map(move |params| select(params).warp.value()),
            )
            .class("display-well")
            .width(Pixels(190.0))
            .height(Stretch(1.0));

            // One knob row, not two: a second row does not fit in the height the
            // screen allows, so the pitch and routing values sit in the inset
            // boxes above instead.
            VStack::new(cx, move |cx| {
                HStack::new(cx, move |cx| {
                    readout(cx, "OCT", move |params| &select(params).octave);
                    readout(cx, "FINE", move |params| &select(params).fine);
                    readout(cx, "UNISON", move |params| &select(params).unison);
                    readout(cx, "RAND", move |params| &select(params).phase_random);
                    readout(cx, "A / B", move |params| &select(params).filter_send);
                    toggle_cell(cx, "FILTER", move |params| &select(params).filter_enabled);
                })
                .col_between(Pixels(4.0))
                .height(Pixels(30.0));

                HStack::new(cx, move |cx| {
                    knob_cell(cx, "DETUNE", move |params| &select(params).detune);
                    knob_cell(cx, "BLEND", move |params| &select(params).blend);
                    knob_cell(cx, "WARP", move |params| &select(params).warp);
                    knob_cell(cx, "PHASE", move |params| &select(params).phase);
                    knob_cell(cx, "PAN", move |params| &select(params).pan);
                    knob_cell(cx, "VOLUME", move |params| &select(params).level);
                })
                .col_between(Pixels(2.0))
                .height(Pixels(KNOB_CELL));
            })
            .row_between(Pixels(4.0))
            .width(Stretch(1.0));
        })
        .col_between(Pixels(9.0))
        .height(Stretch(1.0));
    })
    .class("panel")
    .height(Pixels(OSC_PANEL));
}

fn env_panel(cx: &mut Context) {
    VStack::new(cx, |cx| {
        panel_header(cx, "ENV 1 · AMP", |cx| {
            Label::new(cx, "드래그하여 편집").class("status-line");
        });

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
        .height(Pixels(KNOB_CELL));
    })
    .class("panel")
    .height(Pixels(OSC_PANEL * 2.0 + 8.0));
}

fn voicing_panel(cx: &mut Context) {
    VStack::new(cx, |cx| {
        panel_header(cx, "VOICING", |cx| {
            ParamSlider::new(cx, EditorData::params, |params| &params.voicing.mode)
                .set_style(ParamSliderStyle::CurrentStep { even: true })
                .class("selector")
                .width(Pixels(150.0));
        });

        HStack::new(cx, |cx| {
            readout(cx, "POLY", |params| &params.voicing.polyphony);
            toggle_cell(cx, "ALWAYS GLIDE", |params| &params.voicing.always_glide);
        })
        .col_between(Pixels(5.0))
        .height(Pixels(34.0));

        HStack::new(cx, |cx| {
            knob_cell(cx, "PORTA", |params| &params.voicing.portamento);
            knob_cell(cx, "VELO", |params| &params.voicing.velocity_curve);
            // Filter-envelope depth lives here so the second envelope has a home
            // until the ENV 2 tab exists.
            knob_cell(cx, "F.ENV A", |params| &params.filter_a.env_amount);
        })
        .col_between(Pixels(2.0))
        .height(Pixels(KNOB_CELL));
    })
    .class("panel")
    .height(Pixels(OSC_PANEL - 8.0));
}

fn noise_panel(cx: &mut Context) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            PowerDot::new(cx, EditorData::params, |params| &params.noise.enabled).class("power-dot");
            Label::new(cx, "NOISE").class("panel-title");
        })
        .class("panel-header")
        .height(Pixels(24.0))
        .col_between(Pixels(8.0));

        ParamSlider::new(cx, EditorData::params, |params| &params.noise.colour)
            .set_style(ParamSliderStyle::CurrentStep { even: true })
            .class("selector")
            .height(Pixels(26.0));

        HStack::new(cx, |cx| {
            knob_cell(cx, "LEVEL", |params| &params.noise.level);
            knob_cell(cx, "PAN", |params| &params.noise.pan);
        })
        .col_between(Pixels(2.0))
        .height(Pixels(KNOB_CELL));
    })
    .class("panel")
    .row_between(Pixels(6.0))
    .width(Stretch(1.0));
}

fn filter_panel(cx: &mut Context, title: &'static str, is_a: bool) {
    VStack::new(cx, move |cx| {
        HStack::new(cx, move |cx| {
            if is_a {
                PowerDot::new(cx, EditorData::params, |params| &params.filter_a.enabled).class("power-dot");
            } else {
                PowerDot::new(cx, EditorData::params, |params| &params.filter_b.enabled).class("power-dot");
            }
            Label::new(cx, title).class("panel-title").width(Pixels(78.0));
            Element::new(cx).width(Stretch(1.0));
            if is_a {
                ParamSlider::new(cx, EditorData::params, |params| &params.filter_a.mode)
                    .set_style(ParamSliderStyle::CurrentStep { even: true })
                    .class("selector")
                    .width(Pixels(150.0));
            } else {
                ParamSlider::new(cx, EditorData::params, |params| &params.filter_b.mode)
                    .set_style(ParamSliderStyle::CurrentStep { even: true })
                    .class("selector")
                    .width(Pixels(150.0));
            }
        })
        .class("panel-header")
        .height(Pixels(24.0))
        .col_between(Pixels(8.0));

        HStack::new(cx, move |cx| {
            if is_a {
                knob_cell(cx, "CUT", |params| &params.filter_a.cutoff);
                knob_cell(cx, "RES", |params| &params.filter_a.resonance);
                knob_cell(cx, "KEY", |params| &params.filter_a.keytrack);
            } else {
                knob_cell(cx, "CUT", |params| &params.filter_b.cutoff);
                knob_cell(cx, "RES", |params| &params.filter_b.resonance);
                knob_cell(cx, "KEY", |params| &params.filter_b.keytrack);
            }
        })
        .col_between(Pixels(2.0))
        .height(Pixels(KNOB_CELL));

        // F1 is filter B's series input, taking filter A's output. Filter A has no
        // equivalent because feeding a filter its own output would be a loop.
        if is_a {
            Label::new(cx, "오실레이터 A / B 슬라이더로 입력을 배분합니다").class("status-line");
        } else {
            toggle_cell(cx, "INPUT F1", |params| &params.filter_b.input_from_filter_a);
        }
    })
    .class("panel")
    .row_between(Pixels(6.0))
    .width(Stretch(1.0));
}

/// Dial with its readout and caption underneath.
fn knob_cell<P, FMap>(cx: &mut Context, label: &'static str, select: FMap)
where
    P: Param + 'static,
    FMap: Fn(&Arc<DefaultSynthParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        Knob::new(cx, EditorData::params, select);
        // The readout uses the parameter's own formatter, so units and the custom
        // pan and gain strings come along for free.
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
    .class("knob-cell")
    .height(Pixels(KNOB_CELL));
}

/// Compact inset numeric field, like the design's OCT / FINE / UNISON boxes.
fn readout<P, FMap>(cx: &mut Context, label: &'static str, select: FMap)
where
    P: Param + 'static,
    FMap: Fn(&Arc<DefaultSynthParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        ParamSlider::new(cx, EditorData::params, select)
            .set_style(ParamSliderStyle::CurrentStep { even: true })
            .class("readout")
            .height(Pixels(22.0));
        Label::new(cx, label).class("readout-label");
    })
    .row_between(Pixels(2.0))
    .width(Stretch(1.0))
    .height(Pixels(34.0));
}

fn toggle_cell<FMap>(cx: &mut Context, label: &'static str, select: FMap)
where
    FMap: Fn(&Arc<DefaultSynthParams>) -> &BoolParam + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        ParamButton::new(cx, EditorData::params, select)
            .with_label(label)
            .class("toggle")
            .height(Pixels(22.0));
        Element::new(cx).height(Pixels(10.0));
    })
    .row_between(Pixels(2.0))
    .width(Stretch(1.0))
    .height(Pixels(34.0));
}
