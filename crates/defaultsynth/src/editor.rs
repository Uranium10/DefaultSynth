//! VIZIA editor for the OSC tab.
//!
//! Every measurement here is taken from `Synth.svg` and passed through [`d`],
//! which scales design units to window pixels. The design's own panel boxes are
//! recorded as constants so the layout keeps its proportions instead of drifting
//! into an approximation:
//!
//! ```text
//!   OSC A / B / C   x=269  w=1986  h=562, repeating every 640
//!   ENV             x=2316 y=223   1604x824
//!   LFO             x=2317 y=1131  1604x824
//!   NOISE           x=272  y=2099  608x563
//!   FILTER A        x=943  y=2099  967x563
//!   FILTER B        x=1965 y=2099  967x563
//!   VOICING         x=2981 y=2087  966x563
//! ```
//!
//! Heights are budgeted explicitly rather than left to `auto`, because VIZIA
//! clips overflowing children instead of shrinking them: if a row's parts do not
//! add up, the bottom of the window silently loses content.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::params::{DefaultSynthParams, EnvParams, FilterParams, LfoParams, OscParams};
use crate::widgets::{AbSlider, CurveBox, EnvelopeDisplay, FilterResponse, Field, Knob, LfoDisplay, PowerDot, RadioDot, WaveDisplay};

// ---- Scale -------------------------------------------------------------

/// Content the design lays out, measured from its panel boxes: the left edge of
/// OSC A to the right edge of VOICING, and the tab strip down to the bottom row.
const DESIGN_CONTENT_W: f32 = 3678.0;
const DESIGN_CONTENT_H: f32 = 2700.0;

/// Window height in logical pixels.
///
/// This is the binding constraint. A 125%-scaled 1080p desktop leaves about 816
/// logical pixels of working area and the window chrome takes the rest; ask for
/// more and the OS clamps the window while VIZIA carries on at the size it
/// requested, cropping the bottom panels.
const WINDOW_HEIGHT: f32 = 752.0;
const MARGIN: f32 = 7.0;

/// Design units to window pixels, derived from height because the design is
/// 1.35:1 while the usable screen is wider than that, so height runs out first.
const SCALE: f32 = (WINDOW_HEIGHT - MARGIN * 2.0) / DESIGN_CONTENT_H;
const WINDOW_WIDTH: f32 = DESIGN_CONTENT_W * SCALE + MARGIN * 2.0;

/// Converts a measurement taken off the design into window pixels.
const fn d(design_units: f32) -> f32 {
    design_units * SCALE
}

// ---- Design measurements ----------------------------------------------

const TITLE_H: f32 = d(203.0);
const OSC_W: f32 = d(1986.0);
const OSC_H: f32 = d(562.0);
/// OSC A ends at y=765 and OSC B starts at 843.
const PANEL_GAP: f32 = d(78.0);
const SIDE_W: f32 = d(1604.0);
/// ENV spans y=223..1047 and LFO y=1131..1955.
const SIDE_H: f32 = d(824.0);
const BOTTOM_H: f32 = d(563.0);
/// The wave well inside an OSC panel: x=322..1100, y=310..632.
const WAVE_W: f32 = d(778.0);
const WAVE_H: f32 = d(322.0);
/// Dial columns sit 184 apart; the dial itself is about 120 across.
const KNOB_COL: f32 = d(184.0);
const KNOB_SIZE: f32 = d(104.0);
const KNOB_CELL: f32 = d(213.0);
const FIELD_H: f32 = d(60.0);
const HEADER_H: f32 = d(78.0);
const DOT_ROW_H: f32 = d(56.0);

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32))
}

/// Which of the stacked editors each tab strip is showing.
///
/// ENV 2/3 and LFO 2-4 have no DSP behind them yet, so the tabs switch between
/// parameter sets that exist rather than pretending to switch between engines.
#[derive(Lens)]
struct EditorData {
    params: Arc<DefaultSynthParams>,
    env_tab: usize,
    lfo_tab: usize,
    page: usize,
}

enum EditorEvent {
    SelectEnv(usize),
    SelectLfo(usize),
    SelectPage(usize),
}

impl Model for EditorData {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|editor_event, _| match editor_event {
            EditorEvent::SelectEnv(index) => self.env_tab = *index,
            EditorEvent::SelectLfo(index) => self.lfo_tab = *index,
            EditorEvent::SelectPage(index) => self.page = *index,
        });
    }
}

pub fn create(params: Arc<DefaultSynthParams>, state: Arc<ViziaState>) -> Option<Box<dyn Editor>> {
    create_vizia_editor(state, ViziaTheming::Custom, move |cx, _| {
        assets::register_noto_sans_light(cx);
        assets::register_noto_sans_thin(cx);
        cx.add_stylesheet(include_style!("src/theme.css"))
            .expect("the bundled stylesheet should always parse");

        EditorData { params: params.clone(), env_tab: 0, lfo_tab: 0, page: 0 }.build(cx);

        VStack::new(cx, |cx| {
            title_bar(cx);

            HStack::new(cx, |cx| {
                VStack::new(cx, |cx| {
                    osc_panel(cx, "OSC A", |params| &params.osc_a);
                    osc_panel(cx, "OSC B", |params| &params.osc_b);
                    osc_panel(cx, "OSC C", |params| &params.osc_c);
                })
                .row_between(Pixels(PANEL_GAP))
                .width(Pixels(OSC_W));

                VStack::new(cx, |cx| {
                    env_panel(cx);
                    lfo_panel(cx);
                })
                .row_between(Pixels(PANEL_GAP))
                .width(Pixels(SIDE_W));
            })
            .col_between(Pixels(d(61.0)))
            .height(Pixels(OSC_H * 3.0 + PANEL_GAP * 2.0));

            HStack::new(cx, |cx| {
                noise_panel(cx);
                filter_panel(cx, "FILTER A", true);
                filter_panel(cx, "FILTER B", false);
                voicing_panel(cx);
            })
            .col_between(Pixels(d(56.0)))
            .height(Pixels(BOTTOM_H));
        })
        .class("synth-root");
    })
}

// ---- Title bar ---------------------------------------------------------

const PAGES: [&str; 4] = ["OSC", "EFFECT", "MATRIX", "GLOBAL"];

fn title_bar(cx: &mut Context) {
    HStack::new(cx, |cx| {
        Element::new(cx).class("brand-mark");
        VStack::new(cx, |cx| {
            Label::new(cx, "DefaultSynth").class("brand-name");
            Element::new(cx).class("brand-underline");
        })
        .row_between(Pixels(d(14.0)))
        .width(Pixels(d(700.0)));

        // Page tabs over the preset browser, as in the design. Only OSC has a
        // page behind it so far; the others select but have nothing to show yet.
        VStack::new(cx, |cx| {
            HStack::new(cx, |cx| {
                for (index, name) in PAGES.iter().enumerate() {
                    Button::new(cx, move |cx| cx.emit(EditorEvent::SelectPage(index)), |cx| Label::new(cx, *name))
                        .class("page-tab")
                        .checked(EditorData::page.map(move |page| *page == index))
                        .width(Stretch(1.0));
                }
            })
            .col_between(Pixels(d(6.0)))
            .height(Pixels(d(66.0)));

            HStack::new(cx, |cx| {
                Element::new(cx).class("preset-tick");
                Label::new(cx, "SAW BASS 1").class("preset-name");
                Element::new(cx).width(Stretch(1.0));
                Element::new(cx).class("preset-knob");
            })
            .class("preset-bar")
            .height(Pixels(d(86.0)))
            .col_between(Pixels(d(24.0)));
        })
        .row_between(Pixels(d(10.0)))
        .width(Pixels(d(1740.0)));

        Element::new(cx).width(Stretch(1.0));
        knob_cell(cx, "MASTER", |params| &params.master_gain);
    })
    .class("title-bar")
    .height(Pixels(TITLE_H))
    .col_between(Pixels(d(40.0)));
}

// ---- Oscillator panels -------------------------------------------------

fn osc_panel(
    cx: &mut Context,
    title: &'static str,
    select: impl Fn(&Arc<DefaultSynthParams>) -> &OscParams + Copy + 'static,
) {
    VStack::new(cx, move |cx| {
        // Header: lit dot and title, then FILTER / A—B / DIR on the right.
        HStack::new(cx, move |cx| {
            PowerDot::new(cx, EditorData::params, move |params| &select(params).enabled)
                .class("power-dot");
            Label::new(cx, title).class("panel-title");
            Element::new(cx).width(Stretch(1.0));

            RadioDot::new(cx, EditorData::params, move |params| &select(params).filter_enabled, "FILTER")
                .class("radio-dot")
                .width(Pixels(d(230.0)));
            VStack::new(cx, move |cx| {
                HStack::new(cx, |cx| {
                    Label::new(cx, "A").class("ab-end");
                    Element::new(cx).width(Stretch(1.0));
                    Label::new(cx, "B").class("ab-end");
                })
                .height(Pixels(d(30.0)));
                AbSlider::new(cx, EditorData::params, move |params| &select(params).filter_send)
                    .class("ab-slider")
                    .height(Pixels(d(26.0)));
            })
            .width(Pixels(d(190.0)));
            RadioDot::new(cx, EditorData::params, move |params| &select(params).direct_out, "DIR")
                .class("radio-dot")
                .width(Pixels(d(180.0)));
        })
        .class("panel-header")
        .height(Pixels(HEADER_H))
        .col_between(Pixels(d(20.0)));

        HStack::new(cx, move |cx| {
            // Wave well with its shape selector beneath, as in the design.
            VStack::new(cx, move |cx| {
                WaveDisplay::new(
                    cx,
                    EditorData::params.map(move |params| select(params).waveform.value().to_dsp()),
                    EditorData::params.map(move |params| select(params).warp.value()),
                )
                .class("display-well")
                .height(Pixels(WAVE_H));
                Field::new(cx, EditorData::params, move |params| &select(params).waveform)
                    .class("selector")
                    .height(Pixels(FIELD_H));
            })
            .row_between(Pixels(d(34.0)))
            .width(Pixels(WAVE_W));

            // OCT / FINE / UNISON stacked in their own narrow column.
            VStack::new(cx, move |cx| {
                field_cell(cx, "OCT", move |params| &select(params).octave);
                field_cell(cx, "FINE", move |params| &select(params).fine);
                field_cell(cx, "UNISON", move |params| &select(params).unison);
            })
            .row_between(Pixels(d(14.0)))
            .width(Pixels(d(160.0)));

            // DETUNE / BLEND over WARP / FILTER.
            VStack::new(cx, move |cx| {
                HStack::new(cx, move |cx| {
                    knob_cell(cx, "DETUNE", move |params| &select(params).detune);
                    knob_cell(cx, "BLEND", move |params| &select(params).blend);
                })
                .height(Pixels(KNOB_CELL));
                HStack::new(cx, move |cx| {
                    knob_cell(cx, "WARP", move |params| &select(params).warp);
                    knob_cell(cx, "FILTER", move |params| &select(params).filter_send);
                })
                .height(Pixels(KNOB_CELL));
            })
            .width(Pixels(KNOB_COL * 2.0));

            // PHASE / RAND over the modulation-source selector.
            VStack::new(cx, move |cx| {
                HStack::new(cx, move |cx| {
                    knob_cell(cx, "PHASE", move |params| &select(params).phase);
                    knob_cell(cx, "RAND", move |params| &select(params).phase_random);
                })
                .height(Pixels(KNOB_CELL));
                Field::new(cx, EditorData::params, move |params| &select(params).mod_source)
                    .class("selector")
                    .height(Pixels(FIELD_H));
            })
            .row_between(Pixels(d(40.0)))
            .width(Pixels(KNOB_COL * 2.0));

            VStack::new(cx, move |cx| {
                knob_cell(cx, "PAN", move |params| &select(params).pan);
                knob_cell(cx, "VOLUME", move |params| &select(params).level);
            })
            .width(Pixels(KNOB_COL));
        })
        .col_between(Pixels(d(24.0)))
        .height(Stretch(1.0));
    })
    .class("panel")
    .height(Pixels(OSC_H));
}

// ---- Envelope panel ----------------------------------------------------

const ENV_TABS: [&str; 3] = ["ENV 1", "ENV 2", "ENV 3"];

fn env_panel(cx: &mut Context) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            for (index, name) in ENV_TABS.iter().enumerate() {
                Button::new(cx, move |cx| cx.emit(EditorEvent::SelectEnv(index)), |cx| Label::new(cx, *name))
                    .class("strip-tab")
                    .checked(EditorData::env_tab.map(move |tab| *tab == index))
                    .width(Stretch(1.0));
            }
        })
        .col_between(Pixels(d(6.0)))
        .height(Pixels(d(70.0)));

        // One editor, three parameter sets. Amp and filter envelopes are live;
        // the third is a spare that the modulation matrix will claim.
        Binding::new(cx, EditorData::env_tab, |cx, tab| match tab.get(cx) {
            0 => env_body(cx, |params| &params.amp_env),
            1 => env_body(cx, |params| &params.filter_env),
            _ => env_body(cx, |params| &params.mod_env),
        });
    })
    .class("panel")
    .height(Pixels(SIDE_H));
}

fn env_body(cx: &mut Context, select: fn(&DefaultSynthParams) -> &EnvParams) {
    VStack::new(cx, move |cx| {
        EnvelopeDisplay::new(cx, EditorData::params, select)
            .class("display-well")
            .height(Stretch(1.0));

        HStack::new(cx, move |cx| {
            knob_cell(cx, "ATTACK", move |params| &select(params).attack);
            knob_cell(cx, "HOLD", move |params| &select(params).hold);
            knob_cell(cx, "DECAY", move |params| &select(params).decay);
            knob_cell(cx, "SUSTAIN", move |params| &select(params).sustain);
            knob_cell(cx, "RELEASE", move |params| &select(params).release);
        })
        .height(Pixels(KNOB_CELL));
    })
    .row_between(Pixels(d(24.0)))
    .height(Stretch(1.0));
}

// ---- LFO panel ---------------------------------------------------------

const LFO_TABS: [&str; 4] = ["LFO 1", "LFO 2", "LFO 3", "LFO 4"];

fn lfo_panel(cx: &mut Context) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            for (index, name) in LFO_TABS.iter().enumerate() {
                Button::new(cx, move |cx| cx.emit(EditorEvent::SelectLfo(index)), |cx| Label::new(cx, *name))
                    .class("strip-tab")
                    .checked(EditorData::lfo_tab.map(move |tab| *tab == index))
                    .width(Stretch(1.0));
            }
        })
        .col_between(Pixels(d(6.0)))
        .height(Pixels(d(70.0)));

        LfoDisplay::new(cx)
            .class("display-well")
            .height(Stretch(1.0));

        Binding::new(cx, EditorData::lfo_tab, |cx, tab| match tab.get(cx) {
            0 => lfo_controls(cx, |params| &params.lfo1),
            1 => lfo_controls(cx, |params| &params.lfo2),
            2 => lfo_controls(cx, |params| &params.lfo3),
            _ => lfo_controls(cx, |params| &params.lfo4),
        });
    })
    .row_between(Pixels(d(24.0)))
    .class("panel")
    .height(Pixels(SIDE_H));
}

fn lfo_controls(cx: &mut Context, select: fn(&DefaultSynthParams) -> &LfoParams) {
    HStack::new(cx, move |cx| {
        VStack::new(cx, move |cx| {
            Field::new(cx, EditorData::params, move |params| &select(params).trigger)
                .class("selector")
                .height(Pixels(FIELD_H));
        })
        .width(Pixels(d(300.0)));

        // BPM / TRIP over ANCH / DOT, matching the design's two-by-two dots.
        VStack::new(cx, move |cx| {
            HStack::new(cx, move |cx| {
                RadioDot::new(cx, EditorData::params, move |params| &select(params).sync_bpm, "BPM").class("radio-dot");
                RadioDot::new(cx, EditorData::params, move |params| &select(params).anchor, "ANCH").class("radio-dot");
            })
            .height(Pixels(DOT_ROW_H));
            HStack::new(cx, move |cx| {
                RadioDot::new(cx, EditorData::params, move |params| &select(params).triplet, "TRIP").class("radio-dot");
                RadioDot::new(cx, EditorData::params, move |params| &select(params).dotted, "DOT").class("radio-dot");
            })
            .height(Pixels(DOT_ROW_H));
        })
        .row_between(Pixels(d(20.0)))
        .width(Pixels(d(420.0)));

        Element::new(cx).width(Stretch(1.0));

        HStack::new(cx, move |cx| {
            knob_cell(cx, "RATE", move |params| &select(params).rate);
            knob_cell(cx, "RISE", move |params| &select(params).rise);
            knob_cell(cx, "DELAY", move |params| &select(params).delay);
        })
        .width(Pixels(KNOB_COL * 3.0));
    })
    .col_between(Pixels(d(20.0)))
    .height(Pixels(KNOB_CELL));
}

// ---- Bottom row --------------------------------------------------------

fn noise_panel(cx: &mut Context) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            PowerDot::new(cx, EditorData::params, |params| &params.noise.enabled).class("power-dot");
            Label::new(cx, "NOISE").class("panel-title");
        })
        .class("panel-header")
        .height(Pixels(HEADER_H))
        .col_between(Pixels(d(20.0)));

        Field::new(cx, EditorData::params, |params| &params.noise.colour)
            .class("selector")
            .height(Pixels(FIELD_H));

        RadioDot::new(cx, EditorData::params, |params| &params.noise.keytrack, "KEYTRACK")
            .class("radio-dot")
            .height(Pixels(DOT_ROW_H));

        HStack::new(cx, |cx| {
            knob_cell(cx, "PITCH", |params| &params.noise.pitch);
            knob_cell(cx, "LEVEL", |params| &params.noise.level);
            knob_cell(cx, "PAN", |params| &params.noise.pan);
        })
        .height(Pixels(KNOB_CELL));
    })
    .class("panel")
    .row_between(Pixels(d(20.0)))
    .width(Pixels(d(608.0)));
}

fn filter_panel(cx: &mut Context, title: &'static str, is_a: bool) {
    let select: fn(&DefaultSynthParams) -> &FilterParams =
        if is_a { |params| &params.filter_a } else { |params| &params.filter_b };

    VStack::new(cx, move |cx| {
        HStack::new(cx, move |cx| {
            PowerDot::new(cx, EditorData::params, move |params| &select(params).enabled).class("power-dot");
            Label::new(cx, title).class("panel-title");
            Element::new(cx).width(Stretch(1.0));
            // Input sources. Filter B gains F1, which takes filter A's output and
            // turns the pair from parallel into series.
            RadioDot::new(cx, EditorData::params, move |params| &select(params).input_a, "A").class("radio-dot compact");
            RadioDot::new(cx, EditorData::params, move |params| &select(params).input_b, "B").class("radio-dot compact");
            RadioDot::new(cx, EditorData::params, move |params| &select(params).input_c, "C").class("radio-dot compact");
            RadioDot::new(cx, EditorData::params, move |params| &select(params).input_noise, "N").class("radio-dot compact");
            if !is_a {
                RadioDot::new(cx, EditorData::params, |params| &params.filter_b.input_from_filter_a, "F1")
                    .class("radio-dot compact");
            }
        })
        .class("panel-header")
        .height(Pixels(HEADER_H))
        .col_between(Pixels(d(16.0)));

        HStack::new(cx, move |cx| {
            VStack::new(cx, move |cx| {
                FilterResponse::new(
                    cx,
                    EditorData::params.map(move |params| select(params).mode.value().to_dsp()),
                    EditorData::params.map(move |params| select(params).cutoff.value()),
                    EditorData::params.map(move |params| select(params).resonance.value()),
                )
                .class("display-well")
                .height(Stretch(1.0));
                Field::new(cx, EditorData::params, move |params| &select(params).mode)
                    .class("selector")
                    .height(Pixels(FIELD_H));
            })
            .row_between(Pixels(d(24.0)))
            .width(Pixels(d(425.0)));

            VStack::new(cx, move |cx| {
                HStack::new(cx, move |cx| {
                    knob_cell(cx, "CUT", move |params| &select(params).cutoff);
                    knob_cell(cx, "RES", move |params| &select(params).resonance);
                    knob_cell(cx, "PAN", move |params| &select(params).pan);
                })
                .height(Pixels(KNOB_CELL));
                HStack::new(cx, move |cx| {
                    knob_cell(cx, "DRIVE", move |params| &select(params).drive);
                    knob_cell(cx, "FREQ", move |params| &select(params).freq);
                    knob_cell(cx, "MIX", move |params| &select(params).mix);
                })
                .height(Pixels(KNOB_CELL));
            })
            .width(Stretch(1.0));
        })
        .col_between(Pixels(d(24.0)))
        .height(Stretch(1.0));
    })
    .class("panel")
    .row_between(Pixels(d(16.0)))
    .width(Pixels(d(967.0)));
}

fn voicing_panel(cx: &mut Context) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            PowerDot::new(cx, EditorData::params, |params| &params.voicing.always_glide).class("power-dot");
            Label::new(cx, "VOICING").class("panel-title");
            Element::new(cx).width(Stretch(1.0));
            VStack::new(cx, |cx| {
                Label::new(cx, "POLY").class("readout-label");
            })
            .width(Pixels(d(200.0)));
        })
        .class("panel-header")
        .height(Pixels(HEADER_H))
        .col_between(Pixels(d(16.0)));

        HStack::new(cx, |cx| {
            VStack::new(cx, |cx| {
                RadioDot::new(cx, EditorData::params, |params| &params.voicing.always_glide, "ALWAYS").class("radio-dot");
                mode_dot(cx, "MONO", crate::params::VoiceModeParam::Mono);
                mode_dot(cx, "LEGATO", crate::params::VoiceModeParam::Legato);
            })
            .row_between(Pixels(d(18.0)))
            .width(Pixels(d(300.0)));

            knob_cell(cx, "PORTA", |params| &params.voicing.portamento);

            VStack::new(cx, |cx| {
                Field::new(cx, EditorData::params, |params| &params.voicing.polyphony)
                    .class("readout")
                    .height(Pixels(FIELD_H));
                HStack::new(cx, |cx| {
                    curve_cell(cx, "NOTE", |params| &params.voicing.note_curve);
                    curve_cell(cx, "VELO", |params| &params.voicing.velocity_curve);
                })
                .col_between(Pixels(d(20.0)))
                .height(Stretch(1.0));
            })
            .row_between(Pixels(d(20.0)))
            .width(Pixels(d(420.0)));
        })
        .col_between(Pixels(d(20.0)))
        .height(Stretch(1.0));
    })
    .class("panel")
    .row_between(Pixels(d(16.0)))
    .width(Pixels(d(966.0)));
}

/// A dot that selects one entry of the voice-mode enum.
fn mode_dot(cx: &mut Context, label: &'static str, mode: crate::params::VoiceModeParam) {
    HStack::new(cx, move |cx| {
        Element::new(cx)
            .class("mode-dot-mark")
            .checked(EditorData::params.map(move |params| params.voicing.mode.value() == mode));
        Label::new(cx, label).class("radio-dot-label");
    })
    .class("mode-dot")
    .height(Pixels(DOT_ROW_H))
    .col_between(Pixels(d(18.0)));
}

// ---- Cells -------------------------------------------------------------

/// Dial with its readout and caption underneath.
fn knob_cell<P, FMap>(cx: &mut Context, label: &'static str, select: FMap)
where
    P: Param + 'static,
    FMap: Fn(&Arc<DefaultSynthParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        Knob::new(cx, EditorData::params, select)
            .width(Pixels(KNOB_SIZE))
            .height(Pixels(KNOB_SIZE));
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
    .width(Stretch(1.0));
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

/// A small response curve with its caption, as used by NOTE and VELO.
fn curve_cell<FMap>(cx: &mut Context, label: &'static str, select: FMap)
where
    FMap: Fn(&Arc<DefaultSynthParams>) -> &FloatParam + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        CurveBox::new(cx, EditorData::params.map(move |params| select(params).value()))
            .class("display-well")
            .height(Stretch(1.0));
        Label::new(cx, label).class("readout-label");
    })
    .row_between(Pixels(d(8.0)))
    .width(Stretch(1.0));
}
