//! Modulation routing.
//!
//! An LFO that reaches nothing is inaudible, so the sources and destinations are
//! defined here rather than being hard-wired into the voice. The design puts the
//! routing UI on its own MATRIX page; this is the engine side of it, and it works
//! from host automation in the meantime.

/// Number of routing slots. Enough to be useful without making the parameter
/// list unwieldy before the MATRIX page exists to manage them.
pub const MOD_SLOTS: usize = 8;

/// Where a modulation signal comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSource {
    None,
    Lfo1,
    Lfo2,
    Lfo3,
    Lfo4,
    AmpEnv,
    FilterEnv,
    ModEnv,
    Velocity,
    /// Note number mapped around middle C, so playing higher pushes it positive.
    KeyTrack,
    ModWheel,
}

impl ModSource {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Lfo1,
            2 => Self::Lfo2,
            3 => Self::Lfo3,
            4 => Self::Lfo4,
            5 => Self::AmpEnv,
            6 => Self::FilterEnv,
            7 => Self::ModEnv,
            8 => Self::Velocity,
            9 => Self::KeyTrack,
            10 => Self::ModWheel,
            _ => Self::None,
        }
    }

    /// Index of the LFO this source reads, if it is one.
    pub fn lfo_index(self) -> Option<usize> {
        match self {
            Self::Lfo1 => Some(0),
            Self::Lfo2 => Some(1),
            Self::Lfo3 => Some(2),
            Self::Lfo4 => Some(3),
            _ => None,
        }
    }
}

/// What a modulation signal changes.
///
/// Each destination carries its own natural unit, listed with it, so a matrix
/// amount of 1.0 means something musically sensible everywhere rather than
/// needing a different scaling per row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModDest {
    None,
    /// Semitones, +/- 24 at full amount.
    Pitch,
    /// Octaves of filter cutoff, +/- 4 at full amount.
    FilterACutoff,
    FilterBCutoff,
    /// Normalised resonance, +/- 1.
    FilterAResonance,
    /// Linear gain applied to the whole voice, +/- 1.
    Amplitude,
    /// Voice pan, +/- 1.
    Pan,
    /// Oscillator shape warp, +/- 1.
    OscAWarp,
    OscBWarp,
    OscCWarp,
    /// Oscillator level, +/- 1.
    OscALevel,
    OscBLevel,
    OscCLevel,
    /// Unison detune spread in cents, +/- 100.
    Detune,
}

impl ModDest {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Pitch,
            2 => Self::FilterACutoff,
            3 => Self::FilterBCutoff,
            4 => Self::FilterAResonance,
            5 => Self::Amplitude,
            6 => Self::Pan,
            7 => Self::OscAWarp,
            8 => Self::OscBWarp,
            9 => Self::OscCWarp,
            10 => Self::OscALevel,
            11 => Self::OscBLevel,
            12 => Self::OscCLevel,
            13 => Self::Detune,
            _ => Self::None,
        }
    }

    /// Full-scale range of this destination in its own unit.
    pub fn full_scale(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Pitch => 24.0,
            Self::FilterACutoff | Self::FilterBCutoff => 4.0,
            Self::Detune => 100.0,
            _ => 1.0,
        }
    }
}

/// One row of the matrix.
#[derive(Debug, Clone, Copy)]
pub struct ModSlot {
    pub source: ModSource,
    pub destination: ModDest,
    /// Bipolar depth in `-1.0..=1.0`, scaled by the destination's full scale.
    pub amount: f32,
}

impl Default for ModSlot {
    fn default() -> Self {
        Self { source: ModSource::None, destination: ModDest::None, amount: 0.0 }
    }
}

/// Per-voice modulation source values, sampled once per block.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModInputs {
    pub lfo: [f32; 4],
    pub amp_env: f32,
    pub filter_env: f32,
    pub mod_env: f32,
    pub velocity: f32,
    /// Note number relative to middle C, divided by four octaves.
    pub key_track: f32,
    pub mod_wheel: f32,
}

impl ModInputs {
    pub fn value(&self, source: ModSource) -> f32 {
        match source {
            ModSource::None => 0.0,
            ModSource::Lfo1 => self.lfo[0],
            ModSource::Lfo2 => self.lfo[1],
            ModSource::Lfo3 => self.lfo[2],
            ModSource::Lfo4 => self.lfo[3],
            ModSource::AmpEnv => self.amp_env,
            ModSource::FilterEnv => self.filter_env,
            ModSource::ModEnv => self.mod_env,
            ModSource::Velocity => self.velocity,
            ModSource::KeyTrack => self.key_track,
            ModSource::ModWheel => self.mod_wheel,
        }
    }
}

/// Summed modulation for one voice, in each destination's own unit.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModOutputs {
    pub pitch_semitones: f32,
    pub filter_a_octaves: f32,
    pub filter_b_octaves: f32,
    pub filter_a_resonance: f32,
    pub amplitude: f32,
    pub pan: f32,
    pub osc_warp: [f32; 3],
    pub osc_level: [f32; 3],
    pub detune_cents: f32,
}

/// Adds up every slot into a single set of offsets.
///
/// Several rows may target the same destination; they sum, which is what lets an
/// LFO and an envelope share control of one parameter.
pub fn apply(slots: &[ModSlot; MOD_SLOTS], inputs: &ModInputs) -> ModOutputs {
    let mut out = ModOutputs::default();
    for slot in slots {
        if slot.source == ModSource::None || slot.destination == ModDest::None || slot.amount == 0.0 {
            continue;
        }
        let offset = inputs.value(slot.source) * slot.amount * slot.destination.full_scale();
        match slot.destination {
            ModDest::None => {}
            ModDest::Pitch => out.pitch_semitones += offset,
            ModDest::FilterACutoff => out.filter_a_octaves += offset,
            ModDest::FilterBCutoff => out.filter_b_octaves += offset,
            ModDest::FilterAResonance => out.filter_a_resonance += offset,
            ModDest::Amplitude => out.amplitude += offset,
            ModDest::Pan => out.pan += offset,
            ModDest::OscAWarp => out.osc_warp[0] += offset,
            ModDest::OscBWarp => out.osc_warp[1] += offset,
            ModDest::OscCWarp => out.osc_warp[2] += offset,
            ModDest::OscALevel => out.osc_level[0] += offset,
            ModDest::OscBLevel => out.osc_level[1] += offset,
            ModDest::OscCLevel => out.osc_level[2] += offset,
            ModDest::Detune => out.detune_cents += offset,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> ModInputs {
        ModInputs { lfo: [1.0, -1.0, 0.5, 0.0], velocity: 0.8, ..ModInputs::default() }
    }

    #[test]
    fn an_empty_matrix_changes_nothing() {
        let slots = [ModSlot::default(); MOD_SLOTS];
        let out = apply(&slots, &inputs());
        assert_eq!(out.pitch_semitones, 0.0);
        assert_eq!(out.filter_a_octaves, 0.0);
        assert_eq!(out.amplitude, 0.0);
    }

    #[test]
    fn a_slot_scales_by_its_destination_range() {
        let mut slots = [ModSlot::default(); MOD_SLOTS];
        // LFO 1 sits at +1, so full amount should reach the destination's limit.
        slots[0] = ModSlot { source: ModSource::Lfo1, destination: ModDest::Pitch, amount: 1.0 };
        let out = apply(&slots, &inputs());
        assert!((out.pitch_semitones - 24.0).abs() < 1e-5, "got {}", out.pitch_semitones);

        slots[0].destination = ModDest::FilterACutoff;
        let out = apply(&slots, &inputs());
        assert!((out.filter_a_octaves - 4.0).abs() < 1e-5, "got {}", out.filter_a_octaves);
    }

    #[test]
    fn a_negative_amount_inverts_the_source() {
        let mut slots = [ModSlot::default(); MOD_SLOTS];
        slots[0] = ModSlot { source: ModSource::Lfo1, destination: ModDest::Pitch, amount: -0.5 };
        let out = apply(&slots, &inputs());
        assert!((out.pitch_semitones + 12.0).abs() < 1e-5, "got {}", out.pitch_semitones);
    }

    #[test]
    fn slots_sharing_a_destination_sum() {
        let mut slots = [ModSlot::default(); MOD_SLOTS];
        slots[0] = ModSlot { source: ModSource::Lfo1, destination: ModDest::Pitch, amount: 0.5 };
        // LFO 2 is at -1, so this cancels the first exactly.
        slots[1] = ModSlot { source: ModSource::Lfo2, destination: ModDest::Pitch, amount: 0.5 };
        let out = apply(&slots, &inputs());
        assert!(out.pitch_semitones.abs() < 1e-5, "rows did not cancel: {}", out.pitch_semitones);
    }

    #[test]
    fn a_zero_amount_row_is_skipped() {
        let mut slots = [ModSlot::default(); MOD_SLOTS];
        slots[0] = ModSlot { source: ModSource::Lfo1, destination: ModDest::Pitch, amount: 0.0 };
        assert_eq!(apply(&slots, &inputs()).pitch_semitones, 0.0);
    }

    #[test]
    fn every_destination_index_round_trips() {
        // The editor and the host talk to these by index, so a gap would silently
        // move every row below it to a different destination.
        for index in 0..=13 {
            let dest = ModDest::from_index(index);
            if index == 0 {
                assert_eq!(dest, ModDest::None);
            } else {
                assert_ne!(dest, ModDest::None, "index {index} fell through to None");
            }
        }
        for index in 0..=10 {
            let source = ModSource::from_index(index);
            if index == 0 {
                assert_eq!(source, ModSource::None);
            } else {
                assert_ne!(source, ModSource::None, "index {index} fell through to None");
            }
        }
    }

    #[test]
    fn lfo_sources_report_their_index() {
        assert_eq!(ModSource::Lfo1.lfo_index(), Some(0));
        assert_eq!(ModSource::Lfo4.lfo_index(), Some(3));
        assert_eq!(ModSource::AmpEnv.lfo_index(), None);
    }
}
