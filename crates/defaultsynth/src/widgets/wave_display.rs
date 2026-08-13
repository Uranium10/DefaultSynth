//! Dark oscilloscope-style well showing an oscillator's current shape.
//!
//! Mirrors the design's OSC panels: a recessed dark panel with the waveform
//! drawn as a glowing cyan trace that responds to the waveform and warp settings.

use ds_dsp::Waveform;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

/// Horizontal resolution of the trace. One cycle is drawn across the well.
const POINTS: usize = 256;

pub struct WaveDisplay<W, R> {
    waveform: W,
    warp: R,
}

impl<W, R> WaveDisplay<W, R>
where
    W: Lens<Target = Waveform>,
    R: Lens<Target = f32>,
{
    pub fn new(cx: &mut Context, waveform: W, warp: R) -> Handle<'_, Self> {
        Self { waveform, warp }.build(cx, |_| {})
    }
}

impl<W, R> View for WaveDisplay<W, R>
where
    W: Lens<Target = Waveform>,
    R: Lens<Target = f32>,
{
    fn element(&self) -> Option<&'static str> {
        Some("wave-display")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 2.0 || bounds.h <= 2.0 {
            return;
        }

        let opacity = cx.opacity();
        let waveform = self.waveform.get(cx);
        let warp = self.warp.get(cx).clamp(0.01, 0.99);

        let mut trace: vg::Color = cx.font_color().into();
        trace.set_alphaf(trace.a * opacity);

        // Let VIZIA paint the well itself so the CSS background and corner radius
        // apply without this widget having to re-read them.
        let mut path = cx.build_path();
        cx.draw_background(canvas, &mut path);

        // Inset so the trace never touches the well's edge.
        let pad = (bounds.h * 0.16).min(14.0);
        let top = bounds.y + pad;
        let height = bounds.h - pad * 2.0;
        let centre = top + height / 2.0;

        let mut path = vg::Path::new();
        for index in 0..POINTS {
            let phase = index as f32 / (POINTS - 1) as f32;
            let sample = ideal_shape(waveform, phase, warp);
            let x = bounds.x + 1.0 + phase * (bounds.w - 2.0);
            let y = centre - sample * height / 2.0;
            if index == 0 {
                path.move_to(x, y);
            } else {
                path.line_to(x, y);
            }
        }

        // Two passes give the design's soft glow: a wide translucent stroke under
        // a crisp one.
        let mut glow = vg::Paint::color(vg::Color::rgbaf(trace.r, trace.g, trace.b, trace.a * 0.28));
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
}

/// The mathematically ideal shape, without band-limiting.
///
/// This is a picture, not audio: PolyBLEP's corrections exist to stop aliasing
/// and would only show up here as distracting wobble around the edges.
fn ideal_shape(waveform: Waveform, phase: f32, warp: f32) -> f32 {
    match waveform {
        Waveform::Sine => (phase * std::f32::consts::TAU).sin(),
        Waveform::Triangle => {
            if phase < 0.5 {
                4.0 * phase - 1.0
            } else {
                3.0 - 4.0 * phase
            }
        }
        Waveform::Saw => 1.0 - 2.0 * phase,
        Waveform::Square => {
            if phase < warp {
                1.0
            } else {
                -1.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_stay_inside_the_display_range() {
        for waveform in [Waveform::Sine, Waveform::Triangle, Waveform::Saw, Waveform::Square] {
            for step in 0..=100 {
                let value = ideal_shape(waveform, step as f32 / 100.0, 0.5);
                assert!((-1.0..=1.0).contains(&value), "{waveform:?} produced {value}");
            }
        }
    }

    #[test]
    fn saw_falls_across_the_cycle() {
        // The design draws a falling ramp, matching the OSC panel artwork.
        assert!(ideal_shape(Waveform::Saw, 0.0, 0.5) > 0.9);
        assert!(ideal_shape(Waveform::Saw, 1.0, 0.5) < -0.9);
    }

    #[test]
    fn warp_moves_the_pulse_edge() {
        assert_eq!(ideal_shape(Waveform::Square, 0.2, 0.1), -1.0);
        assert_eq!(ideal_shape(Waveform::Square, 0.2, 0.9), 1.0);
    }
}
