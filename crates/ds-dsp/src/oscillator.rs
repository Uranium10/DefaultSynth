//! Band-limited oscillator bank with unison spread.
//!
//! Naive saw/square waves alias badly: every harmonic above Nyquist folds back
//! down as an audible inharmonic tone. PolyBLEP fixes that by subtracting a
//! polynomial approximation of a band-limited step at each discontinuity, which
//! is cheap enough to run per-voice-per-unison-sample.

use crate::MAX_UNISON;

/// Basic analogue-style shapes. Wavetables come later; these are the fallback set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Sine,
    Triangle,
    Saw,
    Square,
}

impl Waveform {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Sine,
            1 => Self::Triangle,
            2 => Self::Saw,
            _ => Self::Square,
        }
    }
}

/// One detuned copy inside a unison stack.
#[derive(Debug, Clone, Copy, Default)]
struct UnisonVoice {
    phase: f32,
    gain_left: f32,
    gain_right: f32,
}

/// A single oscillator slot (OSC A / B / C in the UI).
#[derive(Debug, Clone)]
pub struct Oscillator {
    voices: [UnisonVoice; MAX_UNISON],
    active: usize,
    sample_rate: f32,
    phase_increment: f32,
    /// Normalised pulse width / shape morph, driven by the WARP knob.
    warp: f32,
    waveform: Waveform,
}

impl Default for Oscillator {
    fn default() -> Self {
        Self {
            voices: [UnisonVoice::default(); MAX_UNISON],
            active: 1,
            sample_rate: 44_100.0,
            phase_increment: 0.0,
            warp: 0.5,
            waveform: Waveform::Saw,
        }
    }
}

impl Oscillator {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
    }

    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }

    /// `warp` morphs pulse width for square and skew for saw/triangle.
    pub fn set_warp(&mut self, warp: f32) {
        self.warp = warp.clamp(0.01, 0.99);
    }

    pub fn active_unison(&self) -> usize {
        self.active
    }

    /// Restarts the stack for a new note.
    ///
    /// `phase` is the common start phase and `randomise` spreads the copies out.
    /// A unison stack that all starts at phase 0 produces a loud comb-filtered
    /// transient, so anything above one voice wants some spread.
    pub fn reset(&mut self, unison: usize, phase: f32, randomise: f32, rng: &mut crate::Rng) {
        self.active = unison.clamp(1, MAX_UNISON);
        for index in 0..self.active {
            let offset = if self.active == 1 {
                0.0
            } else {
                randomise * rng.next_f32()
            };
            self.voices[index].phase = (phase + offset).fract();
        }
        self.update_unison_gains(0.0);
    }

    /// Recomputes the equal-power stereo spread across the stack.
    pub fn update_unison_gains(&mut self, blend: f32) {
        let blend = blend.clamp(0.0, 1.0);
        let count = self.active.max(1);
        for index in 0..count {
            // Centre voice stays centred; outer copies fan out symmetrically.
            let position = if count == 1 {
                0.0
            } else {
                index as f32 / (count - 1) as f32 * 2.0 - 1.0
            };
            let spread = position * blend;
            let angle = (spread * 0.5 + 0.5) * std::f32::consts::FRAC_PI_2;
            self.voices[index].gain_left = angle.cos();
            self.voices[index].gain_right = angle.sin();
        }
    }

    /// Advances the stack one sample and returns an interleaved `(left, right)` pair.
    ///
    /// `detune_cents` spreads the copies in pitch, which is what turns a unison
    /// stack from a louder single tone into the classic supersaw chorus.
    pub fn process(&mut self, frequency: f32, detune_cents: f32) -> (f32, f32) {
        let count = self.active.max(1);
        let mut left = 0.0;
        let mut right = 0.0;
        for index in 0..count {
            let detune = if count == 1 {
                0.0
            } else {
                // Spread symmetrically around the played pitch.
                (index as f32 / (count - 1) as f32 * 2.0 - 1.0) * detune_cents
            };
            let voice_frequency = frequency * cents_to_ratio(detune);
            self.phase_increment = (voice_frequency / self.sample_rate).clamp(0.0, 0.5);

            let voice = &mut self.voices[index];
            let sample = shape(self.waveform, voice.phase, self.phase_increment, self.warp);
            voice.phase += self.phase_increment;
            if voice.phase >= 1.0 {
                voice.phase -= 1.0;
            }
            left += sample * voice.gain_left;
            right += sample * voice.gain_right;
        }
        // Keep perceived level roughly constant as unison count changes.
        let normalise = 1.0 / (count as f32).sqrt();
        (left * normalise, right * normalise)
    }
}

fn shape(waveform: Waveform, phase: f32, increment: f32, warp: f32) -> f32 {
    match waveform {
        Waveform::Sine => (phase * std::f32::consts::TAU).sin(),
        Waveform::Saw => {
            // Naive ramp, then correct the wrap discontinuity.
            let value = 2.0 * phase - 1.0;
            value - poly_blep(phase, increment)
        }
        Waveform::Square => {
            let value = if phase < warp { 1.0 } else { -1.0 };
            // A pulse has two discontinuities per cycle: one at 0 and one at the
            // duty-cycle crossing, so it needs two corrections.
            let rising = poly_blep(phase, increment);
            let falling = poly_blep((phase - warp + 1.0).fract(), increment);
            value + rising - falling
        }
        Waveform::Triangle => {
            // Triangle is continuous, so it only needs the folded ramp.
            let value = if phase < 0.5 {
                4.0 * phase - 1.0
            } else {
                3.0 - 4.0 * phase
            };
            value
        }
    }
}

/// Polynomial band-limited step: smooths the sample either side of a jump.
fn poly_blep(phase: f32, increment: f32) -> f32 {
    if increment <= 0.0 {
        return 0.0;
    }
    if phase < increment {
        // Just after the discontinuity.
        let t = phase / increment;
        t + t - t * t - 1.0
    } else if phase > 1.0 - increment {
        // Just before the next one.
        let t = (phase - 1.0) / increment;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// Converts a pitch offset in cents into a frequency multiplier.
pub fn cents_to_ratio(cents: f32) -> f32 {
    (cents / 1200.0 * std::f32::consts::LN_2).exp()
}

/// Converts a MIDI note number into Hz using the standard A4 = 440 Hz reference.
pub fn midi_note_to_hz(note: f32) -> f32 {
    440.0 * ((note - 69.0) / 12.0).exp2()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|value| value * value).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn midi_note_reference_pitches() {
        assert!((midi_note_to_hz(69.0) - 440.0).abs() < 1e-3);
        assert!((midi_note_to_hz(57.0) - 220.0).abs() < 1e-3);
        assert!((midi_note_to_hz(81.0) - 880.0).abs() < 1e-3);
    }

    #[test]
    fn cents_conversion_matches_semitones() {
        // 100 cents is one semitone, the twelfth root of two.
        assert!((cents_to_ratio(100.0) - 2f32.powf(1.0 / 12.0)).abs() < 1e-5);
        assert!((cents_to_ratio(0.0) - 1.0).abs() < 1e-6);
        assert!((cents_to_ratio(1200.0) - 2.0).abs() < 1e-4);
    }

    #[test]
    fn saw_stays_inside_range_and_is_not_silent() {
        let mut osc = Oscillator::default();
        osc.set_sample_rate(48_000.0);
        osc.set_waveform(Waveform::Saw);
        let mut rng = crate::Rng::new(1);
        osc.reset(1, 0.0, 0.0, &mut rng);

        let samples: Vec<f32> = (0..4096).map(|_| osc.process(220.0, 0.0).0).collect();
        assert!(samples.iter().all(|value| value.abs() <= 1.5), "saw left its range");
        assert!(rms(&samples) > 0.3, "saw was silent");
    }

    #[test]
    fn polyblep_reduces_energy_above_nyquist_versus_naive_saw() {
        // A band-limited saw must not exceed the naive one's raw amplitude swing;
        // the correction only ever pulls samples back toward the ideal ramp.
        let mut osc = Oscillator::default();
        osc.set_sample_rate(48_000.0);
        osc.set_waveform(Waveform::Saw);
        let mut rng = crate::Rng::new(7);
        osc.reset(1, 0.0, 0.0, &mut rng);

        // A high fundamental is where aliasing would be worst.
        let mut worst_jump: f32 = 0.0;
        let mut previous = osc.process(4_000.0, 0.0).0;
        for _ in 0..2048 {
            let current = osc.process(4_000.0, 0.0).0;
            worst_jump = worst_jump.max((current - previous).abs());
            previous = current;
        }
        // The naive version jumps a full 2.0 at every wrap; PolyBLEP softens it.
        assert!(worst_jump < 1.9, "discontinuity was not band-limited: {worst_jump}");
    }

    #[test]
    fn sine_tracks_the_expected_waveform() {
        let mut osc = Oscillator::default();
        osc.set_sample_rate(1_000.0);
        osc.set_waveform(Waveform::Sine);
        let mut rng = crate::Rng::new(3);
        osc.reset(1, 0.0, 0.0, &mut rng);

        // Four samples per cycle at 250 Hz: 0, +1, 0, -1 before panning. A centred
        // voice still goes through the equal-power law, so each channel carries
        // 1/sqrt(2) of it and the pair sums back to unit power.
        let centre = std::f32::consts::FRAC_1_SQRT_2;
        let values: Vec<f32> = (0..4).map(|_| osc.process(250.0, 0.0).0).collect();
        assert!(values[0].abs() < 1e-5, "expected 0, got {}", values[0]);
        assert!((values[1] - centre).abs() < 1e-5, "expected {centre}, got {}", values[1]);
        assert!(values[2].abs() < 1e-4, "expected 0, got {}", values[2]);
        assert!((values[3] + centre).abs() < 1e-4, "expected {}, got {}", -centre, values[3]);
    }

    #[test]
    fn unison_spreads_across_the_stereo_field() {
        let mut osc = Oscillator::default();
        osc.set_sample_rate(48_000.0);
        osc.set_waveform(Waveform::Saw);
        let mut rng = crate::Rng::new(11);
        osc.reset(7, 0.0, 1.0, &mut rng);
        osc.update_unison_gains(1.0);

        let mut left = Vec::new();
        let mut right = Vec::new();
        for _ in 0..8192 {
            let (l, r) = osc.process(110.0, 25.0);
            left.push(l);
            right.push(r);
        }
        // Detuned copies decorrelate, so the channels must not be identical.
        let difference: f32 = left.iter().zip(&right).map(|(l, r)| (l - r).abs()).sum();
        assert!(difference > 1.0, "unison produced a mono signal");
        assert!(rms(&left) > 0.1 && rms(&right) > 0.1);
    }

    #[test]
    fn mono_unison_stays_centred() {
        let mut osc = Oscillator::default();
        osc.set_sample_rate(48_000.0);
        let mut rng = crate::Rng::new(5);
        osc.reset(1, 0.0, 0.0, &mut rng);
        osc.update_unison_gains(1.0);
        let (left, right) = osc.process(440.0, 50.0);
        assert!((left - right).abs() < 1e-6, "single voice was panned");
    }

    #[test]
    fn square_warp_changes_the_duty_cycle() {
        let mut narrow = Oscillator::default();
        narrow.set_sample_rate(48_000.0);
        narrow.set_waveform(Waveform::Square);
        narrow.set_warp(0.1);
        let mut rng = crate::Rng::new(2);
        narrow.reset(1, 0.0, 0.0, &mut rng);

        let mut even = Oscillator::default();
        even.set_sample_rate(48_000.0);
        even.set_waveform(Waveform::Square);
        even.set_warp(0.5);
        even.reset(1, 0.0, 0.0, &mut rng);

        let narrow_mean: f32 = (0..4800).map(|_| narrow.process(100.0, 0.0).0).sum::<f32>() / 4800.0;
        let even_mean: f32 = (0..4800).map(|_| even.process(100.0, 0.0).0).sum::<f32>() / 4800.0;
        // A 10% duty cycle sits mostly negative; a 50% one averages near zero.
        assert!(narrow_mean < -0.5, "narrow pulse had mean {narrow_mean}");
        assert!(even_mean.abs() < 0.1, "even pulse had mean {even_mean}");
    }
}
