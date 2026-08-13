//! A single polyphonic voice: three oscillators, noise, two filters and an amp envelope.

use crate::envelope::{Envelope, EnvelopeSettings};
use crate::filter::{FilterMode, StateVariableFilter};
use crate::noise::{Noise, NoiseColour};
use crate::oscillator::{midi_note_to_hz, Oscillator, Waveform};
use crate::Rng;

/// Per-oscillator settings, mirroring one OSC panel in the UI.
#[derive(Debug, Clone, Copy)]
pub struct OscSettings {
    pub enabled: bool,
    pub waveform: Waveform,
    pub octave: i32,
    pub fine_cents: f32,
    pub unison: usize,
    pub detune_cents: f32,
    pub blend: f32,
    pub warp: f32,
    pub phase: f32,
    pub phase_random: f32,
    pub level: f32,
    pub pan: f32,
    /// Routes this oscillator into filter A (0.0) or filter B (1.0).
    pub filter_send: f32,
    pub filter_enabled: bool,
}

impl Default for OscSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            waveform: Waveform::Saw,
            octave: 0,
            fine_cents: 0.0,
            unison: 1,
            detune_cents: 15.0,
            blend: 0.5,
            warp: 0.5,
            phase: 0.0,
            phase_random: 0.0,
            level: 0.7,
            pan: 0.0,
            filter_send: 0.0,
            filter_enabled: true,
        }
    }
}

/// Everything a voice needs to render, refreshed from plugin parameters each block.
#[derive(Debug, Clone, Copy)]
pub struct VoiceSettings {
    pub osc: [OscSettings; 3],
    pub noise_enabled: bool,
    pub noise_colour: NoiseColour,
    pub noise_level: f32,
    pub noise_pan: f32,
    pub amp_env: EnvelopeSettings,
    pub filter_env: EnvelopeSettings,
    pub filter_a_enabled: bool,
    pub filter_a_mode: FilterMode,
    pub filter_a_cutoff: f32,
    pub filter_a_resonance: f32,
    pub filter_a_env_amount: f32,
    pub filter_a_keytrack: f32,
    pub filter_b_enabled: bool,
    pub filter_b_mode: FilterMode,
    pub filter_b_cutoff: f32,
    pub filter_b_resonance: f32,
    /// The "F1" input on filter B: takes filter A's output instead of running
    /// alongside it, turning the two filters from parallel into series.
    pub filter_b_input_from_a: bool,
    /// Velocity-to-amplitude curve exponent, from the VOICING panel's VELO curve.
    pub velocity_curve: f32,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            osc: [OscSettings::default(); 3],
            noise_enabled: false,
            noise_colour: NoiseColour::White,
            noise_level: 0.0,
            noise_pan: 0.0,
            amp_env: EnvelopeSettings::default(),
            filter_env: EnvelopeSettings::default(),
            filter_a_enabled: true,
            filter_a_mode: FilterMode::Lowpass,
            filter_a_cutoff: 12_000.0,
            filter_a_resonance: 0.1,
            filter_a_env_amount: 0.0,
            filter_a_keytrack: 0.0,
            filter_b_enabled: false,
            filter_b_mode: FilterMode::Highpass,
            filter_b_cutoff: 200.0,
            filter_b_resonance: 0.1,
            filter_b_input_from_a: false,
            velocity_curve: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Voice {
    oscillators: [Oscillator; 3],
    noise: Noise,
    amp_env: Envelope,
    filter_env: Envelope,
    filter_a: StateVariableFilter,
    filter_b: StateVariableFilter,
    note: u8,
    /// The host's voice id for CLAP note expressions; -1 when unused.
    voice_id: i32,
    channel: u8,
    velocity: f32,
    /// Current pitch in MIDI note units, so portamento can glide it.
    current_note: f32,
    target_note: f32,
    glide_rate: f32,
    active: bool,
    /// Monotonic counter used to pick the oldest voice when stealing.
    age: u64,
}

impl Voice {
    pub fn new(sample_rate: f32, seed: u32) -> Self {
        let mut voice = Self {
            oscillators: [Oscillator::default(), Oscillator::default(), Oscillator::default()],
            noise: Noise::new(seed),
            amp_env: Envelope::default(),
            filter_env: Envelope::default(),
            filter_a: StateVariableFilter::new(sample_rate),
            filter_b: StateVariableFilter::new(sample_rate),
            note: 0,
            voice_id: -1,
            channel: 0,
            velocity: 0.0,
            current_note: 0.0,
            target_note: 0.0,
            glide_rate: 0.0,
            active: false,
            age: 0,
        };
        voice.set_sample_rate(sample_rate);
        voice
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        for osc in &mut self.oscillators {
            osc.set_sample_rate(sample_rate);
        }
        self.amp_env.set_sample_rate(sample_rate);
        self.filter_env.set_sample_rate(sample_rate);
        self.filter_a.set_sample_rate(sample_rate);
        self.filter_b.set_sample_rate(sample_rate);
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn note(&self) -> u8 {
        self.note
    }

    pub fn voice_id(&self) -> i32 {
        self.voice_id
    }

    pub fn channel(&self) -> u8 {
        self.channel
    }

    pub fn age(&self) -> u64 {
        self.age
    }

    /// True once the amp envelope has fully released, i.e. the voice is inaudible.
    pub fn is_releasing(&self) -> bool {
        self.amp_env.stage() == crate::envelope::Stage::Release
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        note: u8,
        velocity: f32,
        voice_id: i32,
        channel: u8,
        settings: &VoiceSettings,
        glide_from: Option<f32>,
        glide_seconds: f32,
        sample_rate: f32,
        age: u64,
        rng: &mut Rng,
    ) {
        self.note = note;
        self.voice_id = voice_id;
        self.channel = channel;
        self.velocity = velocity.clamp(0.0, 1.0);
        self.target_note = note as f32;
        self.current_note = glide_from.unwrap_or(self.target_note);
        // Convert the glide time into a per-sample step across the interval.
        let distance = (self.target_note - self.current_note).abs();
        self.glide_rate = if glide_seconds <= f32::EPSILON || distance <= f32::EPSILON {
            f32::INFINITY
        } else {
            distance / (glide_seconds * sample_rate)
        };
        self.active = true;
        self.age = age;

        for (index, osc) in self.oscillators.iter_mut().enumerate() {
            let config = &settings.osc[index];
            osc.set_waveform(config.waveform);
            osc.set_warp(config.warp);
            osc.reset(config.unison, config.phase, config.phase_random, rng);
            osc.update_unison_gains(config.blend);
        }
        self.noise.set_colour(settings.noise_colour);
        self.noise.reset();
        self.filter_a.reset();
        self.filter_b.reset();
        self.amp_env.trigger();
        self.filter_env.trigger();
    }

    /// Retargets an already-sounding voice, used for mono/legato playing.
    pub fn glide_to(&mut self, note: u8, glide_seconds: f32, sample_rate: f32, retrigger: bool) {
        self.note = note;
        self.target_note = note as f32;
        let distance = (self.target_note - self.current_note).abs();
        self.glide_rate = if glide_seconds <= f32::EPSILON || distance <= f32::EPSILON {
            f32::INFINITY
        } else {
            distance / (glide_seconds * sample_rate)
        };
        if retrigger {
            self.amp_env.retrigger_legato();
            self.filter_env.retrigger_legato();
        }
    }

    pub fn release(&mut self) {
        self.amp_env.release();
        self.filter_env.release();
    }

    /// Immediately silences and frees the voice.
    pub fn kill(&mut self) {
        self.amp_env.reset();
        self.filter_env.reset();
        self.active = false;
        self.voice_id = -1;
    }

    /// Renders one stereo sample. Returns `(left, right)`.
    pub fn process(&mut self, settings: &VoiceSettings) -> (f32, f32) {
        if !self.active {
            return (0.0, 0.0);
        }

        // Portamento: step the sounding pitch toward the target.
        if self.current_note != self.target_note {
            if self.glide_rate.is_infinite() {
                self.current_note = self.target_note;
            } else {
                let delta = self.target_note - self.current_note;
                let step = self.glide_rate.copysign(delta);
                self.current_note = if step.abs() >= delta.abs() {
                    self.target_note
                } else {
                    self.current_note + step
                };
            }
        }

        let amp = self.amp_env.process(&settings.amp_env);
        let filter_env = self.filter_env.process(&settings.filter_env);
        if self.amp_env.is_finished() {
            self.active = false;
            self.voice_id = -1;
            return (0.0, 0.0);
        }

        // Sum the oscillators into the two filter buses.
        let mut bus_a = (0.0f32, 0.0f32);
        let mut bus_b = (0.0f32, 0.0f32);
        let mut dry = (0.0f32, 0.0f32);

        for (index, osc) in self.oscillators.iter_mut().enumerate() {
            let config = &settings.osc[index];
            if !config.enabled || config.level <= 0.0 {
                continue;
            }
            let note = self.current_note + config.octave as f32 * 12.0;
            let frequency = midi_note_to_hz(note) * crate::oscillator::cents_to_ratio(config.fine_cents);
            let (mut left, mut right) = osc.process(frequency, config.detune_cents);
            let (pan_left, pan_right) = equal_power_pan(config.pan);
            left *= config.level * pan_left;
            right *= config.level * pan_right;

            if config.filter_enabled {
                // filter_send crossfades this oscillator between the two filters.
                let to_b = config.filter_send.clamp(0.0, 1.0);
                let to_a = 1.0 - to_b;
                bus_a.0 += left * to_a;
                bus_a.1 += right * to_a;
                bus_b.0 += left * to_b;
                bus_b.1 += right * to_b;
            } else {
                dry.0 += left;
                dry.1 += right;
            }
        }

        if settings.noise_enabled && settings.noise_level > 0.0 {
            let sample = self.noise.process() * settings.noise_level;
            let (pan_left, pan_right) = equal_power_pan(settings.noise_pan);
            bus_a.0 += sample * pan_left;
            bus_a.1 += sample * pan_right;
        }

        if settings.filter_a_enabled {
            // Envelope and key tracking both offset the cutoff in octaves, which is
            // how the ear hears filter movement.
            let keytrack = (self.current_note - 60.0) / 12.0 * settings.filter_a_keytrack;
            let modulation = filter_env * settings.filter_a_env_amount + keytrack;
            let cutoff = settings.filter_a_cutoff * modulation.exp2();
            self.filter_a.set_mode(settings.filter_a_mode);
            self.filter_a.set_params(cutoff, settings.filter_a_resonance);
            bus_a.0 = self.filter_a.process(bus_a.0, 0);
            bus_a.1 = self.filter_a.process(bus_a.1, 1);
        }
        // F1 input: fold filter A's output into filter B's bus and stop it reaching
        // the mix on its own, otherwise the dry-ish A path would sit alongside the
        // series path and defeat the point of chaining them.
        if settings.filter_b_input_from_a {
            bus_b.0 += bus_a.0;
            bus_b.1 += bus_a.1;
            bus_a = (0.0, 0.0);
        }

        if settings.filter_b_enabled {
            self.filter_b.set_mode(settings.filter_b_mode);
            self.filter_b.set_params(settings.filter_b_cutoff, settings.filter_b_resonance);
            bus_b.0 = self.filter_b.process(bus_b.0, 0);
            bus_b.1 = self.filter_b.process(bus_b.1, 1);
        }

        // Velocity curve: >1 makes soft playing quieter, <1 compresses the range.
        let velocity_gain = self.velocity.powf(settings.velocity_curve.max(0.01));
        let gain = amp * velocity_gain;
        (
            (bus_a.0 + bus_b.0 + dry.0) * gain,
            (bus_a.1 + bus_b.1 + dry.1) * gain,
        )
    }
}

/// Constant-power pan law: `pan` runs -1 (left) .. 0 (centre) .. +1 (right).
pub fn equal_power_pan(pan: f32) -> (f32, f32) {
    let angle = (pan.clamp(-1.0, 1.0) * 0.5 + 0.5) * std::f32::consts::FRAC_PI_2;
    (angle.cos(), angle.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 48_000.0;

    fn settings() -> VoiceSettings {
        let mut config = VoiceSettings::default();
        config.amp_env = EnvelopeSettings { attack: 0.001, hold: 0.0, decay: 0.05, sustain: 0.8, release: 0.01 };
        config
    }

    #[test]
    fn equal_power_pan_holds_constant_energy() {
        for pan in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            let (left, right) = equal_power_pan(pan);
            let power = left * left + right * right;
            assert!((power - 1.0).abs() < 1e-5, "pan {pan} had power {power}");
        }
        let (left, right) = equal_power_pan(0.0);
        assert!((left - right).abs() < 1e-6, "centre pan was lopsided");
    }

    #[test]
    fn an_untriggered_voice_is_silent_and_inactive() {
        let mut voice = Voice::new(SAMPLE_RATE, 1);
        assert!(!voice.is_active());
        let (left, right) = voice.process(&settings());
        assert_eq!((left, right), (0.0, 0.0));
    }

    #[test]
    fn a_triggered_voice_produces_sound() {
        let mut voice = Voice::new(SAMPLE_RATE, 1);
        let mut rng = Rng::new(9);
        let config = settings();
        voice.start(60, 1.0, 0, 0, &config, None, 0.0, SAMPLE_RATE, 0, &mut rng);
        assert!(voice.is_active());

        let mut peak: f32 = 0.0;
        for _ in 0..4_800 {
            let (left, right) = voice.process(&config);
            peak = peak.max(left.abs()).max(right.abs());
            assert!(left.is_finite() && right.is_finite());
        }
        assert!(peak > 0.05, "voice was inaudible, peak {peak}");
    }

    #[test]
    fn a_released_voice_frees_itself() {
        let mut voice = Voice::new(SAMPLE_RATE, 1);
        let mut rng = Rng::new(9);
        let config = settings();
        voice.start(60, 1.0, 0, 0, &config, None, 0.0, SAMPLE_RATE, 0, &mut rng);
        for _ in 0..1_000 {
            voice.process(&config);
        }
        voice.release();
        for _ in 0..SAMPLE_RATE as usize {
            voice.process(&config);
        }
        assert!(!voice.is_active(), "voice never freed itself after release");
    }

    #[test]
    fn velocity_scales_output_level() {
        let config = settings();
        let mut rng = Rng::new(4);
        let mut measure = |velocity: f32| {
            let mut voice = Voice::new(SAMPLE_RATE, 1);
            voice.start(60, velocity, 0, 0, &config, None, 0.0, SAMPLE_RATE, 0, &mut rng);
            let mut peak: f32 = 0.0;
            for _ in 0..4_800 {
                let (left, _) = voice.process(&config);
                peak = peak.max(left.abs());
            }
            peak
        };
        let soft = measure(0.25);
        let loud = measure(1.0);
        assert!(loud > soft * 2.0, "velocity barely mattered: {soft} vs {loud}");
    }

    #[test]
    fn portamento_glides_instead_of_jumping() {
        let mut voice = Voice::new(SAMPLE_RATE, 1);
        let mut rng = Rng::new(4);
        let config = settings();
        voice.start(48, 1.0, 0, 0, &config, None, 0.0, SAMPLE_RATE, 0, &mut rng);
        voice.process(&config);
        // Glide up an octave over half a second.
        voice.glide_to(60, 0.5, SAMPLE_RATE, false);
        voice.process(&config);
        assert!(voice.current_note < 49.0, "pitch jumped instead of gliding");

        for _ in 0..(SAMPLE_RATE as usize / 2) {
            voice.process(&config);
        }
        assert!((voice.current_note - 60.0).abs() < 0.1, "glide never arrived: {}", voice.current_note);
    }

    #[test]
    fn kill_silences_the_voice_immediately() {
        let mut voice = Voice::new(SAMPLE_RATE, 1);
        let mut rng = Rng::new(4);
        let config = settings();
        voice.start(60, 1.0, 0, 0, &config, None, 0.0, SAMPLE_RATE, 0, &mut rng);
        for _ in 0..500 {
            voice.process(&config);
        }
        voice.kill();
        assert!(!voice.is_active());
        assert_eq!(voice.process(&config), (0.0, 0.0));
    }

    /// Builds a voice whose whole signal sits on filter A's bus.
    fn series_test_settings() -> VoiceSettings {
        let mut config = settings();
        for osc in &mut config.osc {
            osc.enabled = false;
        }
        config.osc[0].enabled = true;
        config.osc[0].waveform = crate::oscillator::Waveform::Saw;
        config.osc[0].filter_enabled = true;
        // 0.0 routes entirely into filter A.
        config.osc[0].filter_send = 0.0;
        config.filter_a_enabled = true;
        config.filter_a_mode = FilterMode::Lowpass;
        config.filter_a_cutoff = 800.0;
        config.filter_b_enabled = true;
        config.filter_b_mode = FilterMode::Highpass;
        config.filter_b_cutoff = 600.0;
        config
    }

    fn voice_rms(config: &VoiceSettings, samples: usize) -> f32 {
        let mut voice = Voice::new(SAMPLE_RATE, 1);
        let mut rng = Rng::new(21);
        voice.start(40, 1.0, 0, 0, config, None, 0.0, SAMPLE_RATE, 0, &mut rng);
        let mut sum = 0.0;
        for _ in 0..samples {
            let (left, _) = voice.process(config);
            sum += left * left;
        }
        (sum / samples as f32).sqrt()
    }

    #[test]
    fn f1_input_chains_filter_a_into_filter_b() {
        let mut parallel = series_test_settings();
        parallel.filter_b_input_from_a = false;
        let mut series = series_test_settings();
        series.filter_b_input_from_a = true;

        let parallel_rms = voice_rms(&parallel, 24_000);
        let series_rms = voice_rms(&series, 24_000);

        // In parallel, filter B receives nothing and A's lowpass output passes through
        // untouched. In series that same output is then highpassed, so a low note
        // loses energy. The two must not be the same signal.
        assert!(parallel_rms > 0.01, "parallel path was silent");
        assert!(series_rms < parallel_rms * 0.9, "F1 did not chain the filters: {series_rms} vs {parallel_rms}");
    }

    #[test]
    fn f1_input_does_not_leave_filter_a_ringing_in_the_mix() {
        // With B bypassed and F1 on, A's output is handed to a disabled filter B,
        // which passes it through. Level should stay comparable, not double up.
        let mut config = series_test_settings();
        config.filter_b_enabled = false;
        config.filter_b_input_from_a = true;
        let chained = voice_rms(&config, 24_000);

        config.filter_b_input_from_a = false;
        let direct = voice_rms(&config, 24_000);

        assert!((chained - direct).abs() < direct * 0.05, "routing changed the level: {chained} vs {direct}");
    }

    #[test]
    fn disabled_oscillators_contribute_nothing() {
        let mut config = settings();
        for osc in &mut config.osc {
            osc.enabled = false;
        }
        let mut voice = Voice::new(SAMPLE_RATE, 1);
        let mut rng = Rng::new(4);
        voice.start(60, 1.0, 0, 0, &config, None, 0.0, SAMPLE_RATE, 0, &mut rng);
        for _ in 0..1_000 {
            let (left, right) = voice.process(&config);
            assert!(left.abs() < 1e-6 && right.abs() < 1e-6, "muted oscillators still sounded");
        }
    }
}
