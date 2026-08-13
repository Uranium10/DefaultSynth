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
use crate::widgets::{EnvelopeDisplay, Field, Knob, PowerDot, WaveDisplay};

// Sizes below are the reference design's own measurements from Synth.svg,
// multiplied by SCALE. The design canvas is 3872x2859, which is far larger than
// any plugin window, but keeping its proportions is what makes the layout read
// like the drawing instead of an approximation of it.

/// Design canvas, from the SVG's root element.
const DESIGN_WIDTH: f32 = 3872.0;
const DESIGN_HEIGHT: f32 = 2859.0;

/// Window height in logical pixels.
///
/// This is the binding constraint: a 125%-scaled 1080p desktop leaves about 816
/// logical pixels of working area, and the window chrome eats the rest. Anything
/// larger gets clamped by the OS while VIZIA carries on laying out at the size it
/// asked for, which crops the bottom panels.
const WINDOW_HEIGHT: f32 = 752.0;

/// Design units to window pixels.
///
/// Derived from the height, not the width. The design is 1.35:1 and the usable
/// screen area is wider than that, so height is what runs out first; scaling by
/// width overflows the bottom of the window.
const SCALE: f32 = WINDOW_HEIGHT / DESIGN_HEIGHT;
const WINDOW_WIDTH: f32 = DESIGN_WIDTH * SCALE;

/// Converts a measurement taken off the design into window pixels.
const fn d(design_units: f32) -> f32 {
    design_units * SCALE
}

/// OSC A's title sits at y=278 and OSC B's at y=918, so panels repeat every 640.
const OSC_PITCH: f32 = 640.0;
const OSC_PANEL: f32 = d(OSC_PITCH - 50.0);
/// The bottom strip's titles are at y=2197 and its lowest labels at y=2631.
const BOTTOM_STRIP: f32 = d(560.0);
const TITLE_BAR: f32 = d(230.0);
/// DETUNE at x=1295 and BLEND at x=1479 put dial columns 184 units apart.
const KNOB_CELL_W: f32 = d(184.0);
const KNOB_SIZE: f32 = d(132.0);
const KNOB_CELL: f32 = d(212.0);
/// The OCT / FINE / UNISON boxes and the wave selector.
const FIELD_H: f32 = d(58.0);
/// The wave display well runs from the panel's left edge to the OCT column.
const WAVE_W: f32 = d(700.0);

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32))
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
    .height(Pixels(d(66.0)))
    .col_between(Pixels(8.0));
}

fn osc_panel(
    cx: &mut Context,
    title: &'static str,
    select: impl Fn(&Arc<DefaultSynthParams>) -> &OscParams + Copy + 'static,
) {
    VStack::new(cx, move |cx| {
        // Header: the lit dot and title on the left, the filter routing controls
        // on the right, which is where FILTER / A|B / DIR sit in the design.
        HStack::new(cx, move |cx| {
            PowerDot::new(cx, EditorData::params, move |params| &select(params).enabled)
                .class("power-dot");
            Label::new(cx, title).class("panel-title");
            Element::new(cx).width(Stretch(1.0));
            ParamButton::new(cx, EditorData::params, move |params| &select(params).filter_enabled)
                .with_label("FILTER")
                .class("toggle")
                .width(Pixels(d(190.0)))
                .height(Pixels(d(52.0)));
        })
        .class("panel-header")
        .height(Pixels(d(66.0)))
        .col_between(Pixels(d(24.0)));

        HStack::new(cx, move |cx| {
            WaveDisplay::new(
                cx,
                EditorData::params.map(move |params| select(params).waveform.value().to_dsp()),
                EditorData::params.map(move |params| select(params).warp.value()),
            )
            .class("display-well")
            .width(Pixels(WAVE_W))
            .height(Stretch(1.0));

            // The design stacks OCT / FINE / UNISON vertically in a narrow column
            // beside the wave display, not in a row across the panel.
            VStack::new(cx, move |cx| {
                field_cell(cx, "OCT", move |params| &select(params).octave);
                field_cell(cx, "FINE", move |params| &select(params).fine);
                field_cell(cx, "UNISON", move |params| &select(params).unison);
            })
            .row_between(Pixels(d(24.0)))
            .width(Pixels(d(150.0)));

            // Then a 2x2 dial block, PHASE and RAND above the routing selector,
            // and PAN over VOLUME on the right, matching the drawing's grouping.
            VStack::new(cx, move |cx| {
                HStack::new(cx, move |cx| {
                    knob_cell(cx, "DETUNE", move |params| &select(params).detune);
                    knob_cell(cx, "BLEND", move |params| &select(params).blend);
                })
                .height(Pixels(KNOB_CELL));
                HStack::new(cx, move |cx| {
                    knob_cell(cx, "WARP", move |params| &select(params).warp);
                    knob_cell(cx, "A / B", move |params| &select(params).filter_send);
                })
                .height(Pixels(KNOB_CELL));
            })
            .row_between(Pixels(d(20.0)))
            .width(Pixels(KNOB_CELL_W * 2.0));

            VStack::new(cx, move |cx| {
                HStack::new(cx, move |cx| {
                    knob_cell(cx, "PHASE", move |params| &select(params).phase);
                    knob_cell(cx, "RAND", move |params| &select(params).phase_random);
                })
                .height(Pixels(KNOB_CELL));
                Field::new(cx, EditorData::params, move |params| &select(params).waveform)
                    .class("selector")
                    .height(Pixels(FIELD_H));
            })
            .row_between(Pixels(d(30.0)))
            .width(Pixels(KNOB_CELL_W * 2.0));

            VStack::new(cx, move |cx| {
                knob_cell(cx, "PAN", move |params| &select(params).pan);
                knob_cell(cx, "VOLUME", move |params| &select(params).level);
            })
            .width(Pixels(KNOB_CELL_W));
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
        .col_between(Pixels(0.0))
        .height(Pixels(KNOB_CELL));
    })
    .class("panel")
    .height(Pixels(OSC_PANEL * 2.0 + 8.0));
}

fn voicing_panel(cx: &mut Context) {
    VStack::new(cx, |cx| {
        panel_header(cx, "VOICING", |cx| {
            Field::new(cx, EditorData::params, |params| &params.voicing.mode)
                .class("selector")
                .width(Pixels(d(460.0)));
        });

        HStack::new(cx, |cx| {
            VStack::new(cx, |cx| {
                field_cell(cx, "POLY", |params| &params.voicing.polyphony);
            })
            .width(Pixels(d(320.0)));
            toggle_cell(cx, "ALWAYS GLIDE", |params| &params.voicing.always_glide);
            Element::new(cx).width(Stretch(1.0));
        })
        .col_between(Pixels(d(24.0)))
        .height(Pixels(FIELD_H + d(30.0)));

        HStack::new(cx, |cx| {
            knob_cell(cx, "PORTA", |params| &params.voicing.portamento);
            knob_cell(cx, "VELO", |params| &params.voicing.velocity_curve);
            // Filter-envelope depth lives here so the second envelope has a home
            // until the ENV 2 tab exists.
            knob_cell(cx, "F.ENV A", |params| &params.filter_a.env_amount);
        })
        .col_between(Pixels(0.0))
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
        .height(Pixels(d(66.0)))
        .col_between(Pixels(8.0));

        Field::new(cx, EditorData::params, |params| &params.noise.colour)
            .class("selector")
            .height(Pixels(FIELD_H));

        HStack::new(cx, |cx| {
            knob_cell(cx, "LEVEL", |params| &params.noise.level);
            knob_cell(cx, "PAN", |params| &params.noise.pan);
        })
        .col_between(Pixels(0.0))
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
            Label::new(cx, title).class("panel-title");
            Element::new(cx).width(Stretch(1.0));
            if is_a {
                Field::new(cx, EditorData::params, |params| &params.filter_a.mode)
                    .class("selector")
                    .width(Pixels(d(460.0)));
            } else {
                Field::new(cx, EditorData::params, |params| &params.filter_b.mode)
                    .class("selector")
                    .width(Pixels(d(460.0)));
            }
        })
        .class("panel-header")
        .height(Pixels(d(66.0)))
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
        .col_between(Pixels(0.0))
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
        Knob::new(cx, EditorData::params, select).width(Pixels(KNOB_SIZE)).height(Pixels(KNOB_SIZE));
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

/// Inset numeric box with its caption underneath, like OCT / FINE / UNISON.
fn field_cell<P, FMap>(cx: &mut Context, label: &'static str, select: FMap)
where
    P: Param + 'static,
    FMap: Fn(&Arc<DefaultSynthParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        Field::new(cx, EditorData::params, select)
            .class("readout")
            .height(Pixels(FIELD_H));
        Label::new(cx, label).class("readout-label");
    })
    .row_between(Pixels(d(8.0)))
    .width(Stretch(1.0));
}

fn toggle_cell<FMap>(cx: &mut Context, label: &'static str, select: FMap)
where
    FMap: Fn(&Arc<DefaultSynthParams>) -> &BoolParam + Copy + 'static,
{
    ParamButton::new(cx, EditorData::params, select)
        .with_label(label)
        .class("toggle")
        .height(Pixels(FIELD_H));
}
