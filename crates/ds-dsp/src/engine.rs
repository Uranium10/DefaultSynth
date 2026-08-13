//! Voice allocation and the top-level render loop.

use crate::voice::{Voice, VoiceSettings};
use crate::{Rng, MAX_POLYPHONY};

/// How incoming notes map onto voices, matching the UI's VOICING panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceMode {
    /// Every note gets its own voice.
    Poly,
    /// One voice; each new note restarts the envelopes.
    Mono,
    /// One voice; overlapping notes glide without restarting the envelopes.
    Legato,
}

impl VoiceMode {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Poly,
            1 => Self::Mono,
            _ => Self::Legato,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VoicingSettings {
    pub mode: VoiceMode,
    pub polyphony: usize,
    pub portamento_seconds: f32,
    /// When true, portamento applies even to non-overlapping notes.
    pub always_glide: bool,
}

impl Default for VoicingSettings {
    fn default() -> Self {
        Self { mode: VoiceMode::Poly, polyphony: 8, portamento_seconds: 0.0, always_glide: false }
    }
}

pub struct SynthEngine {
    voices: Vec<Voice>,
    /// Notes currently held down, in arrival order, for mono/legato note priority.
    held_notes: Vec<u8>,
    rng: Rng,
    sample_rate: f32,
    /// Increments per note-on so voice stealing can find the oldest voice.
    age_counter: u64,
    last_note: Option<f32>,
}

impl SynthEngine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            voices: (0..MAX_POLYPHONY).map(|index| Voice::new(sample_rate, 0x9E37_79B9 ^ index as u32)).collect(),
            held_notes: Vec::with_capacity(128),
            rng: Rng::new(0x1234_5678),
            sample_rate: sample_rate.max(1.0),
            age_counter: 0,
            last_note: None,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
        for voice in &mut self.voices {
            voice.set_sample_rate(self.sample_rate);
        }
    }

    pub fn active_voice_count(&self) -> usize {
        self.voices.iter().filter(|voice| voice.is_active()).count()
    }

    /// Drops every sounding voice without a release tail.
    pub fn reset(&mut self) {
        for voice in &mut self.voices {
            voice.kill();
        }
        self.held_notes.clear();
        self.last_note = None;
    }

    pub fn note_on(
        &mut self,
        note: u8,
        velocity: f32,
        voice_id: i32,
        channel: u8,
        settings: &VoiceSettings,
        voicing: &VoicingSettings,
    ) {
        self.held_notes.retain(|held| *held != note);
        self.held_notes.push(note);
        self.age_counter = self.age_counter.wrapping_add(1);

        if voicing.mode != VoiceMode::Poly {
            let overlapping = self.held_notes.len() > 1;
            // Legato only re-triggers when the previous note has been let go.
            let retrigger = voicing.mode == VoiceMode::Mono || !overlapping;
            let glide = if voicing.always_glide || overlapping { voicing.portamento_seconds } else { 0.0 };

            if let Some(voice) = self.voices.iter_mut().find(|voice| voice.is_active()) {
                voice.glide_to(note, glide, self.sample_rate, retrigger);
                self.last_note = Some(note as f32);
                return;
            }
            let glide_from = if voicing.always_glide { self.last_note } else { None };
            let sample_rate = self.sample_rate;
            let age = self.age_counter;
            self.voices[0].start(note, velocity, voice_id, channel, settings, glide_from, glide, sample_rate, age, &mut self.rng);
            self.last_note = Some(note as f32);
            return;
        }

        let limit = voicing.polyphony.clamp(1, MAX_POLYPHONY);
        let index = self.allocate(limit);
        let glide_from = if voicing.always_glide { self.last_note } else { None };
        let glide = if voicing.always_glide { voicing.portamento_seconds } else { 0.0 };
        let sample_rate = self.sample_rate;
        let age = self.age_counter;
        self.voices[index].start(note, velocity, voice_id, channel, settings, glide_from, glide, sample_rate, age, &mut self.rng);
        self.last_note = Some(note as f32);
    }

    pub fn note_off(&mut self, note: u8, voicing: &VoicingSettings) {
        self.held_notes.retain(|held| *held != note);

        if voicing.mode != VoiceMode::Poly {
            // Mono/legato fall back to the most recent note still held down.
            if let Some(&previous) = self.held_notes.last() {
                if let Some(voice) = self.voices.iter_mut().find(|voice| voice.is_active()) {
                    voice.glide_to(previous, voicing.portamento_seconds, self.sample_rate, voicing.mode == VoiceMode::Mono);
                }
            } else {
                for voice in &mut self.voices {
                    voice.release();
                }
            }
            return;
        }

        for voice in &mut self.voices {
            if voice.is_active() && voice.note() == note && !voice.is_releasing() {
                voice.release();
            }
        }
    }

    /// Releases everything, letting the tails ring out.
    pub fn all_notes_off(&mut self) {
        self.held_notes.clear();
        for voice in &mut self.voices {
            if voice.is_active() {
                voice.release();
            }
        }
    }

    /// Renders one stereo sample by summing every active voice.
    pub fn process(&mut self, settings: &VoiceSettings) -> (f32, f32) {
        let mut left = 0.0;
        let mut right = 0.0;
        for voice in &mut self.voices {
            if !voice.is_active() {
                continue;
            }
            let (voice_left, voice_right) = voice.process(settings);
            left += voice_left;
            right += voice_right;
        }
        (left, right)
    }

    /// Picks a voice slot: a free one, else the oldest releasing one, else the oldest.
    fn allocate(&mut self, limit: usize) -> usize {
        // Voices past the polyphony limit must be silenced or they keep ringing
        // after the user lowers the setting.
        for index in limit..self.voices.len() {
            if self.voices[index].is_active() {
                self.voices[index].kill();
            }
        }
        if let Some(index) = (0..limit).find(|index| !self.voices[*index].is_active()) {
            return index;
        }
        let releasing = (0..limit)
            .filter(|index| self.voices[*index].is_releasing())
            .min_by_key(|index| self.voices[*index].age());
        if let Some(index) = releasing {
            return index;
        }
        (0..limit)
            .min_by_key(|index| self.voices[*index].age())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::EnvelopeSettings;

    const SAMPLE_RATE: f32 = 48_000.0;

    fn settings() -> VoiceSettings {
        let mut config = VoiceSettings::default();
        config.amp_env = EnvelopeSettings { attack: 0.001, hold: 0.0, decay: 0.05, sustain: 0.8, release: 0.01 };
        config
    }

    fn peak_over(engine: &mut SynthEngine, config: &VoiceSettings, samples: usize) -> f32 {
        let mut peak: f32 = 0.0;
        for _ in 0..samples {
            let (left, right) = engine.process(config);
            assert!(left.is_finite() && right.is_finite(), "engine produced non-finite output");
            peak = peak.max(left.abs()).max(right.abs());
        }
        peak
    }

    #[test]
    fn silent_until_a_note_arrives() {
        let mut engine = SynthEngine::new(SAMPLE_RATE);
        let config = settings();
        assert_eq!(engine.active_voice_count(), 0);
        assert_eq!(peak_over(&mut engine, &config, 512), 0.0);
    }

    #[test]
    fn a_note_on_makes_sound_and_note_off_stops_it() {
        let mut engine = SynthEngine::new(SAMPLE_RATE);
        let config = settings();
        let voicing = VoicingSettings::default();

        engine.note_on(60, 1.0, -1, 0, &config, &voicing);
        assert_eq!(engine.active_voice_count(), 1);
        assert!(peak_over(&mut engine, &config, 4_800) > 0.05, "note produced no sound");

        engine.note_off(60, &voicing);
        peak_over(&mut engine, &config, SAMPLE_RATE as usize);
        assert_eq!(engine.active_voice_count(), 0, "voice was not reclaimed");
    }

    #[test]
    fn polyphony_limit_caps_simultaneous_voices() {
        let mut engine = SynthEngine::new(SAMPLE_RATE);
        let config = settings();
        let voicing = VoicingSettings { polyphony: 4, ..Default::default() };
        for note in 60..70 {
            engine.note_on(note, 1.0, -1, 0, &config, &voicing);
            engine.process(&config);
        }
        assert!(engine.active_voice_count() <= 4, "polyphony limit was exceeded: {}", engine.active_voice_count());
    }

    #[test]
    fn voice_stealing_prefers_the_oldest_voice() {
        let mut engine = SynthEngine::new(SAMPLE_RATE);
        let config = settings();
        let voicing = VoicingSettings { polyphony: 2, ..Default::default() };
        engine.note_on(60, 1.0, -1, 0, &config, &voicing);
        engine.note_on(64, 1.0, -1, 0, &config, &voicing);
        engine.process(&config);
        // The third note has to steal, and note 60 is the oldest.
        engine.note_on(67, 1.0, -1, 0, &config, &voicing);
        engine.process(&config);

        let sounding: Vec<u8> = engine.voices.iter().filter(|voice| voice.is_active()).map(|voice| voice.note()).collect();
        assert!(!sounding.contains(&60), "stole the wrong voice, still holding {sounding:?}");
        assert!(sounding.contains(&67), "new note never sounded");
    }

    #[test]
    fn mono_mode_uses_a_single_voice() {
        let mut engine = SynthEngine::new(SAMPLE_RATE);
        let config = settings();
        let voicing = VoicingSettings { mode: VoiceMode::Mono, ..Default::default() };
        for note in [60, 64, 67] {
            engine.note_on(note, 1.0, -1, 0, &config, &voicing);
            engine.process(&config);
        }
        assert_eq!(engine.active_voice_count(), 1, "mono mode stacked voices");
    }

    #[test]
    fn mono_note_off_falls_back_to_the_still_held_note() {
        let mut engine = SynthEngine::new(SAMPLE_RATE);
        let config = settings();
        let voicing = VoicingSettings { mode: VoiceMode::Mono, ..Default::default() };
        engine.note_on(60, 1.0, -1, 0, &config, &voicing);
        engine.note_on(67, 1.0, -1, 0, &config, &voicing);
        engine.process(&config);
        // Releasing the top note should drop back to the one still held.
        engine.note_off(67, &voicing);
        engine.process(&config);
        assert_eq!(engine.active_voice_count(), 1);
        let sounding = engine.voices.iter().find(|voice| voice.is_active()).map(|voice| voice.note());
        assert_eq!(sounding, Some(60), "mono did not fall back to the held note");
    }

    #[test]
    fn all_notes_off_clears_everything() {
        let mut engine = SynthEngine::new(SAMPLE_RATE);
        let config = settings();
        let voicing = VoicingSettings::default();
        for note in 60..66 {
            engine.note_on(note, 1.0, -1, 0, &config, &voicing);
        }
        engine.all_notes_off();
        peak_over(&mut engine, &config, SAMPLE_RATE as usize);
        assert_eq!(engine.active_voice_count(), 0);
    }

    #[test]
    fn reset_silences_instantly() {
        let mut engine = SynthEngine::new(SAMPLE_RATE);
        let config = settings();
        let voicing = VoicingSettings::default();
        engine.note_on(60, 1.0, -1, 0, &config, &voicing);
        engine.process(&config);
        engine.reset();
        assert_eq!(engine.active_voice_count(), 0);
        assert_eq!(peak_over(&mut engine, &config, 256), 0.0);
    }

    #[test]
    fn lowering_polyphony_silences_out_of_range_voices() {
        let mut engine = SynthEngine::new(SAMPLE_RATE);
        let config = settings();
        let wide = VoicingSettings { polyphony: 8, ..Default::default() };
        for note in 60..68 {
            engine.note_on(note, 1.0, -1, 0, &config, &wide);
        }
        engine.process(&config);
        let narrow = VoicingSettings { polyphony: 2, ..Default::default() };
        engine.note_on(72, 1.0, -1, 0, &config, &narrow);
        engine.process(&config);
        assert!(engine.active_voice_count() <= 2, "stale voices survived a polyphony drop");
    }

    #[test]
    fn a_dense_chord_stays_within_a_sane_level() {
        let mut engine = SynthEngine::new(SAMPLE_RATE);
        let config = settings();
        let voicing = VoicingSettings { polyphony: 16, ..Default::default() };
        for note in [48, 52, 55, 59, 62, 65, 69, 72] {
            engine.note_on(note, 1.0, -1, 0, &config, &voicing);
        }
        // Voices sum, so this checks for runaway rather than clipping.
        let peak = peak_over(&mut engine, &config, 9_600);
        assert!(peak < 40.0, "summed voices ran away: {peak}");
    }
}
