//! DefaultSynth synthesis core.
//!
//! Deliberately independent of any plugin framework: this crate knows about
//! samples and notes, not about VST3, CLAP or GUI toolkits. That keeps the DSP
//! unit-testable with a plain `cargo test` and reusable outside a plugin host.

pub mod curve;
pub mod engine;
pub mod envelope;
pub mod filter;
pub mod lfo;
pub mod modulation;
pub mod noise;
pub mod oscillator;
pub mod voice;

pub use curve::{CurvePoint, LfoCurve, MAX_CURVE_POINTS};
pub use engine::{SynthEngine, VoiceMode, VoicingSettings};
pub use envelope::{Envelope, EnvelopeSettings};
pub use filter::{FilterMode, StateVariableFilter};
pub use lfo::{Lfo, LfoSettings, LfoShape, LfoTrigger};
pub use modulation::{ModDest, ModInputs, ModOutputs, ModSlot, ModSource, MOD_SLOTS};
pub use noise::{Noise, NoiseColour};
pub use oscillator::{midi_note_to_hz, Oscillator, Waveform};
pub use voice::{OscSettings, Voice, VoiceSettings};

/// Hard ceiling on simultaneous voices. The UI's POLY field tops out below this.
pub const MAX_POLYPHONY: usize = 32;

/// Maximum unison copies per oscillator, matching the UI's UNISON field.
pub const MAX_UNISON: usize = 16;

/// Small xorshift PRNG.
///
/// The audio thread must not allocate or lock, and it needs randomness every
/// sample for noise and per-note phase spread. `rand` would pull in a heavier
/// dependency for what is three shifts.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        // A zero state is a fixed point for xorshift, so never allow it.
        Self { state: if seed == 0 { 0x8765_4321 } else { seed } }
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut state = self.state;
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        self.state = state;
        state
    }

    /// Uniform in `0.0..1.0`.
    pub fn next_f32(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    /// Uniform in `-1.0..1.0`.
    pub fn next_bipolar(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }
}

/// Converts decibels into a linear gain multiplier.
pub fn db_to_gain(db: f32) -> f32 {
    if db <= -100.0 {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_stays_in_range_and_never_sticks() {
        let mut rng = Rng::new(1);
        let mut seen_low = false;
        let mut seen_high = false;
        let mut previous = rng.next_f32();
        for _ in 0..10_000 {
            let value = rng.next_f32();
            assert!((0.0..=1.0).contains(&value), "rng left range: {value}");
            if value < 0.25 {
                seen_low = true;
            }
            if value > 0.75 {
                seen_high = true;
            }
            assert!(value != previous || value == 0.0, "rng got stuck");
            previous = value;
        }
        assert!(seen_low && seen_high, "rng did not cover its range");
    }

    #[test]
    fn rng_survives_a_zero_seed() {
        let mut rng = Rng::new(0);
        // A zero state would lock xorshift at zero forever.
        assert!(rng.next_u32() != 0);
        assert!(rng.next_u32() != 0);
    }

    #[test]
    fn bipolar_output_is_centred() {
        let mut rng = Rng::new(99);
        let samples: Vec<f32> = (0..50_000).map(|_| rng.next_bipolar()).collect();
        assert!(samples.iter().all(|value| (-1.0..=1.0).contains(value)));
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!(mean.abs() < 0.02, "bipolar rng had bias {mean}");
    }

    #[test]
    fn db_conversion_hits_the_reference_points() {
        assert!((db_to_gain(0.0) - 1.0).abs() < 1e-6);
        assert!((db_to_gain(-6.0) - 0.5011872).abs() < 1e-4);
        assert!((db_to_gain(6.0) - 1.9952624).abs() < 1e-4);
        assert_eq!(db_to_gain(-120.0), 0.0, "silence floor should be exactly zero");
    }
}
