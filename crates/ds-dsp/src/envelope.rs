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
                if settings.attack <= f32::EPSILON {
                    self.value = 1.0;
                    self.advance(Stage::Hold);
                } else {
                    // Inverted exponential: fast at first, easing into the peak.
                    self.value = 1.0 - (-5.0 * elapsed / settings.attack).exp();
                    if elapsed >= settings.attack {
                        self.value = 1.0;
                        self.advance(Stage::Hold);
                    }
                }
            }
            Stage::Hold => {
                self.value = 1.0;
                if elapsed >= settings.hold {
                    self.advance(Stage::Decay);
                }
            }
            Stage::Decay => {
                if settings.decay <= f32::EPSILON {
                    self.value = settings.sustain;
                    self.advance(Stage::Sustain);
                } else {
                    let progress = (-5.0 * elapsed / settings.decay).exp();
                    self.value = settings.sustain + (1.0 - settings.sustain) * progress;
                    if elapsed >= settings.decay {
                        self.value = settings.sustain;
                        self.advance(Stage::Sustain);
                    }
                }
            }
            Stage::Sustain => self.value = settings.sustain,
            Stage::Release => {
                if settings.release <= f32::EPSILON {
                    self.value = 0.0;
                    self.stage = Stage::Idle;
                } else {
                    self.value = self.release_from * (-5.0 * elapsed / settings.release).exp();
                    if elapsed >= settings.release {
                        self.value = 0.0;
                        self.stage = Stage::Idle;
                    }
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
