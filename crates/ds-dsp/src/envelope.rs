//! AHDSR envelope generator matching the UI's ATTACK / HOLD / DECAY / SUSTAIN / RELEASE.
//!
//! Stages use exponential curves rather than straight lines because linear
//! decays sound unnaturally abrupt; analogue envelopes are RC curves.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Idle,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
}

/// Envelope times in seconds, plus a normalised sustain level.
#[derive(Debug, Clone, Copy)]
pub struct EnvelopeSettings {
    pub attack: f32,
    pub hold: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl Default for EnvelopeSettings {
    fn default() -> Self {
        Self { attack: 0.005, hold: 0.0, decay: 0.15, sustain: 0.7, release: 0.25 }
    }
}

/// Exponential curvature shared by every stage.
///
/// After this many time constants a stage is within ~0.7% of its target, which
/// is close enough to call it arrived.
const CURVE: f32 = 5.0;

/// Normalised exponential fall from 1 to ~0 across a stage.
///
/// Both the decay and the release are this shape, just scaled to different
/// start and end levels.
pub fn exp_fall(progress: f32) -> f32 {
    (-CURVE * progress.clamp(0.0, 1.0)).exp()
}

impl EnvelopeSettings {
    /// Level partway through a stage, where `progress` runs 0..1 across it.
    ///
    /// [`Envelope::process`] is driven by this, and so is the editor's curve
    /// display. Keeping one definition is what stops the drawn envelope and the
    /// audible one from drifting apart as the curves are tuned.
    pub fn stage_level(&self, stage: Stage, progress: f32) -> f32 {
        let t = progress.clamp(0.0, 1.0);
        match stage {
            Stage::Idle => 0.0,
            // A zero-length stage is already at its destination.
            Stage::Attack if self.attack <= f32::EPSILON => 1.0,
            Stage::Attack => 1.0 - exp_fall(t),
            Stage::Hold => 1.0,
            Stage::Decay if self.decay <= f32::EPSILON => self.sustain,
            Stage::Decay => self.sustain + (1.0 - self.sustain) * exp_fall(t),
            Stage::Sustain => self.sustain,
            Stage::Release if self.release <= f32::EPSILON => 0.0,
            // Releasing from the sustain level, which is where a held note sits.
            Stage::Release => self.sustain * exp_fall(t),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Envelope {
    stage: Stage,
    value: f32,
    /// Level the release stage started from, so an early release does not jump.
    release_from: f32,
    stage_samples: u32,
    sample_rate: f32,
}

impl Default for Envelope {
    fn default() -> Self {
        Self { stage: Stage::Idle, value: 0.0, release_from: 0.0, stage_samples: 0, sample_rate: 44_100.0 }
    }
}

impl Envelope {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    /// True once the envelope has fully released and the voice can be reclaimed.
    pub fn is_finished(&self) -> bool {
        self.stage == Stage::Idle
    }

    pub fn is_active(&self) -> bool {
        self.stage != Stage::Idle
    }

    pub fn trigger(&mut self) {
        self.stage = Stage::Attack;
        self.stage_samples = 0;
    }

    /// Re-triggers without resetting the current level, for legato voice stealing.
    pub fn retrigger_legato(&mut self) {
        self.stage = Stage::Attack;
        self.stage_samples = 0;
    }

    pub fn release(&mut self) {
        if self.stage != Stage::Idle {
            self.release_from = self.value;
            self.stage = Stage::Release;
            self.stage_samples = 0;
        }
    }

    /// Silences the envelope immediately, used for panic / all-notes-off.
    pub fn reset(&mut self) {
        self.stage = Stage::Idle;
        self.value = 0.0;
        self.release_from = 0.0;
        self.stage_samples = 0;
    }

    /// Advances one sample and returns the new level in `0.0..=1.0`.
    pub fn process(&mut self, settings: &EnvelopeSettings) -> f32 {
        let elapsed = self.stage_samples as f32 / self.sample_rate;
        self.stage_samples = self.stage_samples.saturating_add(1);

        match self.stage {
            Stage::Idle => self.value = 0.0,
            Stage::Attack => {
                // Inverted exponential: fast at first, easing into the peak.
                self.value = settings.stage_level(Stage::Attack, elapsed / settings.attack);
                if elapsed >= settings.attack {
                    self.value = 1.0;
                    self.advance(Stage::Hold);
                }
            }
            Stage::Hold => {
                self.value = 1.0;
                if elapsed >= settings.hold {
                    self.advance(Stage::Decay);
                }
            }
            Stage::Decay => {
                self.value = settings.stage_level(Stage::Decay, elapsed / settings.decay);
                if elapsed >= settings.decay {
                    self.value = settings.sustain;
                    self.advance(Stage::Sustain);
                }
            }
            Stage::Sustain => self.value = settings.sustain,
            Stage::Release => {
                // Falls from wherever the note actually was, so releasing mid-attack
                // does not jump up to the sustain level first. Same curve the
                // display draws, only anchored to a different starting level.
                self.value = if settings.release <= f32::EPSILON {
                    0.0
                } else {
                    self.release_from * exp_fall(elapsed / settings.release)
                };
                if elapsed >= settings.release {
                    self.value = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.value
    }

    fn advance(&mut self, next: Stage) {
        self.stage = next;
        self.stage_samples = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> EnvelopeSettings {
        EnvelopeSettings { attack: 0.01, hold: 0.0, decay: 0.05, sustain: 0.5, release: 0.02 }
    }

    #[test]
    fn starts_idle_and_silent() {
        let env = Envelope::default();
        assert_eq!(env.stage(), Stage::Idle);
        assert!(env.is_finished());
        assert_eq!(env.value(), 0.0);
    }

    #[test]
    fn rises_to_peak_then_settles_on_sustain() {
        let mut env = Envelope::default();
        env.set_sample_rate(48_000.0);
        env.trigger();
        let config = settings();

        // Run through attack, and the level must be climbing.
        let early = env.process(&config);
        for _ in 0..100 {
            env.process(&config);
        }
        let later = env.process(&config);
        assert!(later > early, "attack did not rise");

        // Well past attack + decay it must hold the sustain level.
        for _ in 0..48_000 {
            env.process(&config);
        }
        assert_eq!(env.stage(), Stage::Sustain);
        assert!((env.value() - config.sustain).abs() < 1e-4);
    }

    #[test]
    fn release_falls_to_silence_and_finishes() {
        let mut env = Envelope::default();
        env.set_sample_rate(48_000.0);
        env.trigger();
        let config = settings();
        for _ in 0..48_000 {
            env.process(&config);
        }
        env.release();
        for _ in 0..48_000 {
            env.process(&config);
        }
        assert!(env.is_finished(), "envelope never finished releasing");
        assert_eq!(env.value(), 0.0);
    }

    #[test]
    fn release_starts_from_the_current_level_without_jumping() {
        let mut env = Envelope::default();
        env.set_sample_rate(48_000.0);
        env.trigger();
        let config = EnvelopeSettings { attack: 1.0, ..settings() };

        // Release mid-attack, while the level is still low.
        for _ in 0..1_000 {
            env.process(&config);
        }
        let level_before = env.value();
        assert!(level_before < 0.5, "test setup should release during attack");
        env.release();
        let first = env.process(&config);
        assert!(first <= level_before + 1e-3, "release jumped up from {level_before} to {first}");
    }

    #[test]
    fn zero_attack_reaches_peak_immediately() {
        let mut env = Envelope::default();
        env.set_sample_rate(48_000.0);
        env.trigger();
        let config = EnvelopeSettings { attack: 0.0, ..settings() };
        let value = env.process(&config);
        assert!((value - 1.0).abs() < 1e-6, "instant attack produced {value}");
    }

    #[test]
    fn never_leaves_the_normalised_range() {
        let mut env = Envelope::default();
        env.set_sample_rate(48_000.0);
        env.trigger();
        let config = settings();
        for index in 0..96_000 {
            if index == 20_000 {
                env.release();
            }
            let value = env.process(&config);
            assert!((0.0..=1.0).contains(&value), "envelope left range: {value}");
        }
    }

    #[test]
    fn stage_level_matches_what_the_running_envelope_produces() {
        // The editor draws from stage_level while the audio comes from process().
        // If these ever disagree the displayed envelope becomes a lie.
        let config = EnvelopeSettings { attack: 0.2, hold: 0.0, decay: 0.4, sustain: 0.35, release: 0.3 };
        let sample_rate = 48_000.0;
        let mut env = Envelope::default();
        env.set_sample_rate(sample_rate);
        env.trigger();

        // Sample the attack a quarter of the way in.
        let attack_samples = (config.attack * sample_rate * 0.25) as usize;
        for _ in 0..attack_samples {
            env.process(&config);
        }
        let expected = config.stage_level(Stage::Attack, 0.25);
        assert!((env.value() - expected).abs() < 1e-3, "attack drifted: {} vs {expected}", env.value());

        // And the decay, halfway through.
        while env.stage() == Stage::Attack || env.stage() == Stage::Hold {
            env.process(&config);
        }
        let decay_samples = (config.decay * sample_rate * 0.5) as usize;
        for _ in 0..decay_samples {
            env.process(&config);
        }
        let expected = config.stage_level(Stage::Decay, 0.5);
        assert!((env.value() - expected).abs() < 1e-3, "decay drifted: {} vs {expected}", env.value());
    }

    #[test]
    fn release_still_rings_out_when_sustain_is_zero() {
        // A percussive patch sustains at zero but must still release from whatever
        // level the decay left behind rather than cutting off instantly.
        let config = EnvelopeSettings { attack: 0.0, hold: 0.0, decay: 4.0, sustain: 0.0, release: 0.5 };
        let mut env = Envelope::default();
        env.set_sample_rate(48_000.0);
        env.trigger();
        for _ in 0..2_000 {
            env.process(&config);
        }
        let before = env.value();
        assert!(before > 0.5, "test setup should release from a high level, got {before}");

        env.release();
        let first = env.process(&config);
        assert!(first > before * 0.9, "release cut off instantly: {first} from {before}");

        for _ in 0..24_000 {
            env.process(&config);
        }
        assert!(env.is_finished(), "release never completed");
    }

    #[test]
    fn stage_level_stays_normalised_everywhere() {
        let config = EnvelopeSettings { attack: 1.0, hold: 0.5, decay: 2.0, sustain: 0.4, release: 1.5 };
        for stage in [Stage::Idle, Stage::Attack, Stage::Hold, Stage::Decay, Stage::Sustain, Stage::Release] {
            for step in 0..=20 {
                let value = config.stage_level(stage, step as f32 / 20.0);
                assert!((0.0..=1.0).contains(&value), "{stage:?} produced {value}");
            }
        }
    }

    #[test]
    fn reset_silences_an_active_envelope() {
        let mut env = Envelope::default();
        env.set_sample_rate(48_000.0);
        env.trigger();
        for _ in 0..500 {
            env.process(&settings());
        }
        env.reset();
        assert!(env.is_finished());
        assert_eq!(env.value(), 0.0);
    }
}
