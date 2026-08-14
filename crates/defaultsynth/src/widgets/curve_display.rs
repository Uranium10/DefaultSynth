//! The remaining dark wells: the LFO shape, the filter response, and the small
//! NOTE / VELO curve boxes in the voicing panel.
//!
//! None of these are hooked to a running signal yet, so each draws the shape its
//! own parameters describe rather than pretending to show live analysis.

use crate::params::LfoShapeParam as LfoShape;
use ds_dsp::FilterMode;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

/// Points along each drawn curve.
const POINTS: usize = 160;
/// Vertical range of the filter response well, in decibels.
const FLOOR_DB: f32 = -40.0;
const CEILING_DB: f32 = 18.0;

/// Strokes `sample(t)` across the well.
///
/// Returning `None` breaks the line rather than clamping it, so a filter slope
/// that has fallen off the bottom of the display simply stops instead of
/// crawling along the floor.
fn stroke_curve(cx: &mut DrawContext, canvas: &mut Canvas, sample: impl Fn(f32) -> Option<f32>) {
    let bounds = cx.bounds();
    let opacity = cx.opacity();

    let mut path = cx.build_path();
    // The default View::draw runs shadows before the background; overriding draw
    // means doing that here too, or the CSS box-shadow never appears.
    cx.draw_shadows(canvas, &mut path);
    cx.draw_background(canvas, &mut path);

    let mut trace: vg::Color = cx.selection_color().into();
    trace.set_alphaf(trace.a * opacity);

    let pad = (bounds.h * 0.14).min(12.0);
    let top = bounds.y + pad;
    let height = bounds.h - pad * 2.0;
    if height <= 1.0 {
        return;
    }

    let mut path = vg::Path::new();
    let mut drawing = false;
    for index in 0..=POINTS {
        let t = index as f32 / POINTS as f32;
        let x = bounds.x + 2.0 + t * (bounds.w - 4.0);
        match sample(t) {
            Some(level) => {
                let y = top + height * (1.0 - level.clamp(0.0, 1.0));
                if drawing {
                    path.line_to(x, y);
                } else {
                    path.move_to(x, y);
                    drawing = true;
                }
            }
            // Off the bottom of the display: lift the pen and wait for it to come back.
            None => drawing = false,
        }
    }

    // Wide translucent pass under a crisp one, matching the design's soft glow.
    let mut glow = vg::Paint::color(vg::Color::rgbaf(trace.r, trace.g, trace.b, trace.a * 0.25));
    glow.set_line_width(5.0);
    glow.set_line_cap(vg::LineCap::Round);
    glow.set_line_join(vg::LineJoin::Round);
    canvas.stroke_path(&path, &glow);

    let mut paint = vg::Paint::color(trace);
    paint.set_line_width(1.8);
    paint.set_line_cap(vg::LineCap::Round);
    paint.set_line_join(vg::LineJoin::Round);
    canvas.stroke_path(&path, &paint);
}

/// The LFO well: draws the running shape, and steps to the next one when clicked.
///
/// The design gives the LFO panel one long selector box, and its label is TRIG,
/// so there is no drawn control left for the shape. Rather than inventing a box
/// the design does not have, the well itself is the shape control.
pub struct LfoDisplay {
    param_base: ParamWidgetBase,
    shapes: usize,
}

impl LfoDisplay {
    pub fn new<L, Params, P, FMap>(cx: &mut Context, params: L, params_to_param: FMap) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        let shapes = ParamWidgetBase::new(cx, params.clone(), params_to_param).step_count().unwrap_or(0) + 1;
        Self { param_base: ParamWidgetBase::new(cx, params, params_to_param), shapes }
            .build(cx, ParamWidgetBase::build_view(params, params_to_param, move |_cx, _data| {}))
    }

    fn shape(&self) -> LfoShape {
        let steps = (self.shapes - 1).max(1) as f32;
        let index = (self.param_base.unmodulated_normalized_value() * steps).round() as usize;
        LfoShape::from_index(index)
    }

    /// Advances to the next shape, wrapping past the last one.
    fn cycle(&self, cx: &mut EventContext) {
        let steps = (self.shapes - 1).max(1) as f32;
        let index = (self.param_base.unmodulated_normalized_value() * steps).round() as usize;
        let next = (index + 1) % self.shapes;
        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, next as f32 / steps);
        self.param_base.end_set_parameter(cx);
    }
}

impl View for LfoDisplay {
    fn element(&self) -> Option<&'static str> {
        Some("lfo-display")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                self.cycle(cx);
                meta.consume();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        if cx.bounds().w <= 4.0 {
            return;
        }
        let shape = self.shape();
        // Two cycles, so a sample-and-hold or a saw reads as repeating rather
        // than as a single ramp.
        stroke_curve(cx, canvas, move |t| Some(lfo_level(shape, (t * 2.0).fract())));
    }
}

/// One LFO cycle mapped to the well's 0..1 vertical range.
///
/// Sample-and-hold is random by nature, so the display shows a fixed set of
/// steps: an honest picture of "stepped random" without redrawing every frame.
fn lfo_level(shape: LfoShape, phase: f32) -> f32 {
    const HELD_STEPS: [f32; 8] = [0.3, -0.7, 0.9, -0.2, 0.55, -0.95, 0.15, -0.45];
    let bipolar = match shape {
        LfoShape::Sine => (phase * std::f32::consts::TAU).sin(),
        LfoShape::Triangle => {
            if phase < 0.25 {
                phase * 4.0
            } else if phase < 0.75 {
                2.0 - phase * 4.0
            } else {
                phase * 4.0 - 4.0
            }
        }
        LfoShape::SawUp => phase * 2.0 - 1.0,
        LfoShape::SawDown => 1.0 - phase * 2.0,
        LfoShape::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        LfoShape::SampleHold => HELD_STEPS[((phase * 8.0) as usize).min(7)],
    };
    bipolar * 0.5 + 0.5
}

/// Magnitude response of a filter, drawn from its own cutoff and resonance.
pub struct FilterResponse<M, C, R> {
    mode: M,
    cutoff: C,
    resonance: R,
}

impl<M, C, R> FilterResponse<M, C, R>
where
    M: Lens<Target = FilterMode>,
    C: Lens<Target = f32>,
    R: Lens<Target = f32>,
{
    pub fn new(cx: &mut Context, mode: M, cutoff: C, resonance: R) -> Handle<'_, Self> {
        Self { mode, cutoff, resonance }.build(cx, |_| {})
    }
}

impl<M, C, R> View for FilterResponse<M, C, R>
where
    M: Lens<Target = FilterMode>,
    C: Lens<Target = f32>,
    R: Lens<Target = f32>,
{
    fn element(&self) -> Option<&'static str> {
        Some("filter-response")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        if cx.bounds().w <= 4.0 {
            return;
        }
        let mode = self.mode.get(cx);
        let cutoff = self.cutoff.get(cx).clamp(20.0, 20_000.0);
        let resonance = self.resonance.get(cx).clamp(0.0, 1.0);
        // Q maps the same way the SVF does, so the drawn peak tracks the knob.
        let q = 0.5 + resonance * 9.5;

        stroke_curve(cx, canvas, move |t| {
            // Log frequency axis over the audible range, as in the design.
            let hz = 20.0 * 1000f32.powf(t);
            let ratio = hz / cutoff;
            let magnitude = analogue_magnitude(mode, ratio, q);
            let db = 20.0 * magnitude.max(1e-6).log10();
            let level = (db - FLOOR_DB) / (CEILING_DB - FLOOR_DB);
            // Below the floor the slope has left the display, so the line stops
            // there rather than flattening out along the bottom edge.
            (level > 0.0).then_some(level)
        });
    }
}

/// Second-order analogue prototype magnitude, which is what the ZDF SVF models.
fn analogue_magnitude(mode: FilterMode, ratio: f32, q: f32) -> f32 {
    let w = ratio.max(1e-4);
    let w2 = w * w;
    // |H| for a normalised biquad with s = jw.
    let denominator = ((1.0 - w2).powi(2) + (w / q).powi(2)).sqrt();
    match mode {
        FilterMode::Lowpass => 1.0 / denominator,
        FilterMode::Highpass => w2 / denominator,
        FilterMode::Bandpass => (w / q) / denominator,
        FilterMode::Notch => (1.0 - w2).abs() / denominator,
    }
}

/// The small NOTE / VELO boxes: a response curve with an exponent.
pub struct CurveBox<E> {
    exponent: E,
}

impl<E: Lens<Target = f32>> CurveBox<E> {
    pub fn new(cx: &mut Context, exponent: E) -> Handle<'_, Self> {
        Self { exponent }.build(cx, |_| {})
    }
}

impl<E: Lens<Target = f32>> View for CurveBox<E> {
    fn element(&self) -> Option<&'static str> {
        Some("curve-box")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        if cx.bounds().w <= 4.0 {
            return;
        }
        let exponent = self.exponent.get(cx).max(0.01);
        stroke_curve(cx, canvas, move |t| Some(t.powf(exponent)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowpass_response_falls_above_its_cutoff() {
        let at_dc = analogue_magnitude(FilterMode::Lowpass, 0.01, 0.7);
        let at_cutoff = analogue_magnitude(FilterMode::Lowpass, 1.0, 0.7);
        let above = analogue_magnitude(FilterMode::Lowpass, 10.0, 0.7);
        assert!(at_dc > at_cutoff, "lowpass should pass DC most");
        assert!(at_cutoff > above, "lowpass should fall past the cutoff");
    }

    #[test]
    fn highpass_is_the_mirror_of_lowpass() {
        let below = analogue_magnitude(FilterMode::Highpass, 0.01, 0.7);
        let above = analogue_magnitude(FilterMode::Highpass, 10.0, 0.7);
        assert!(above > below, "highpass should pass the top");
        assert!(below < 0.01, "highpass let too much DC through: {below}");
    }

    #[test]
    fn resonance_raises_the_peak_at_the_cutoff() {
        let flat = analogue_magnitude(FilterMode::Lowpass, 1.0, 0.5);
        let resonant = analogue_magnitude(FilterMode::Lowpass, 1.0, 10.0);
        assert!(resonant > flat * 5.0, "resonance did not lift the peak");
    }

    #[test]
    fn every_mode_stays_finite_across_the_sweep() {
        for mode in [FilterMode::Lowpass, FilterMode::Highpass, FilterMode::Bandpass, FilterMode::Notch] {
            for step in 0..=200 {
                let ratio = 20.0 * 1000f32.powf(step as f32 / 200.0) / 1_000.0;
                let value = analogue_magnitude(mode, ratio, 10.0);
                assert!(value.is_finite(), "{mode:?} produced {value} at ratio {ratio}");
            }
        }
    }
}
