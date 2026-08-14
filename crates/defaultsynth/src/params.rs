//! Host-visible parameters, grouped to mirror the panels in the UI design.

use nih_plug::prelude::*;
use std::sync::Arc;

/// Oscillator waveform choices. Wavetables replace this later; these are the base shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum WaveformParam {
    Sine,
    Triangle,
    Saw,
    Square,
}

impl WaveformParam {
    pub fn to_dsp(self) -> ds_dsp::Waveform {
        match self {
            Self::Sine => ds_dsp::Waveform::Sine,
            Self::Triangle => ds_dsp::Waveform::Triangle,
            Self::Saw => ds_dsp::Waveform::Saw,
            Self::Square => ds_dsp::Waveform::Square,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum FilterModeParam {
    #[name = "LP"]
    Lowpass,
    #[name = "HP"]
    Highpass,
    #[name = "BP"]
    Bandpass,
    #[name = "Notch"]
    Notch,
}

impl FilterModeParam {
    pub fn to_dsp(self) -> ds_dsp::FilterMode {
        match self {
            Self::Lowpass => ds_dsp::FilterMode::Lowpass,
            Self::Highpass => ds_dsp::FilterMode::Highpass,
            Self::Bandpass => ds_dsp::FilterMode::Bandpass,
            Self::Notch => ds_dsp::FilterMode::Notch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum NoiseColourParam {
    White,
    Pink,
    Brown,
}

impl NoiseColourParam {
    pub fn to_dsp(self) -> ds_dsp::NoiseColour {
        match self {
            Self::White => ds_dsp::NoiseColour::White,
            Self::Pink => ds_dsp::NoiseColour::Pink,
            Self::Brown => ds_dsp::NoiseColour::Brown,
        }
    }
}

/// What OSC B / OSC C do to the oscillator in front of them.
///
/// Only `None` is wired to the DSP so far; the rest exist so the routing
/// selector in the editor is the real control it will stay once the modulation
/// paths land, rather than a placeholder that has to be replaced later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum ModSourceParam {
    #[name = "NONE"]
    None,
    #[name = "FM (B)"]
    FmB,
    #[name = "FM (C)"]
    FmC,
    #[name = "RING"]
    Ring,
    #[name = "SYNC"]
    Sync,
}

/// Retrigger behaviour for an LFO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum LfoTriggerParam {
    #[name = "TRIG"]
    Trigger,
    #[name = "FREE"]
    Free,
    #[name = "ENV"]
    Envelope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum VoiceModeParam {
    Poly,
    Mono,
    Legato,
}

impl VoiceModeParam {
    pub fn to_dsp(self) -> ds_dsp::VoiceMode {
        match self {
            Self::Poly => ds_dsp::VoiceMode::Poly,
            Self::Mono => ds_dsp::VoiceMode::Mono,
            Self::Legato => ds_dsp::VoiceMode::Legato,
        }
    }
}

/// Pan control with a matching pair of string conversions.
///
/// A custom `value_to_string` without a matching `string_to_value` makes the
/// parameter unparseable, which hosts (and clap-validator) treat as a fault.
fn pan_param(name: &str) -> FloatParam {
    FloatParam::new(name, 0.0, FloatRange::Linear { min: -1.0, max: 1.0 })
        .with_smoother(SmoothingStyle::Linear(20.0))
        .with_value_to_string(Arc::new(|value| {
            if value.abs() < 0.005 {
                "C".to_string()
            } else if value < 0.0 {
                format!("{:.0}L", value.abs() * 100.0)
            } else {
                format!("{:.0}R", value * 100.0)
            }
        }))
        .with_string_to_value(Arc::new(|string| {
            let text = string.trim().to_ascii_uppercase();
            if text == "C" {
                return Some(0.0);
            }
            let (digits, sign) = match text.strip_suffix('L') {
                Some(rest) => (rest, -1.0),
                None => (text.strip_suffix('R').unwrap_or(&text), 1.0),
            };
            digits
                .trim()
                .parse::<f32>()
                .ok()
                .map(|value| (value / 100.0 * sign).clamp(-1.0, 1.0))
        }))
}

/// One OSC panel's worth of parameters.
#[derive(Params)]
pub struct OscParams {
    #[id = "on"]
    pub enabled: BoolParam,
    #[id = "wave"]
    pub waveform: EnumParam<WaveformParam>,
    #[id = "oct"]
    pub octave: IntParam,
    #[id = "fine"]
    pub fine: FloatParam,
    #[id = "uni"]
    pub unison: IntParam,
    #[id = "det"]
    pub detune: FloatParam,
    #[id = "blend"]
    pub blend: FloatParam,
    #[id = "warp"]
    pub warp: FloatParam,
    #[id = "phase"]
    pub phase: FloatParam,
    #[id = "rand"]
    pub phase_random: FloatParam,
    #[id = "lvl"]
    pub level: FloatParam,
    #[id = "pan"]
    pub pan: FloatParam,
    #[id = "flten"]
    pub filter_enabled: BoolParam,
    /// 0 routes into filter A, 1 into filter B, in between feeds both.
    #[id = "fltab"]
    pub filter_send: FloatParam,
    /// DIR: sends this oscillator straight to the output, past both filters.
    #[id = "dir"]
    pub direct_out: BoolParam,
    /// What this oscillator does to its neighbour. Not yet wired to the DSP.
    #[id = "mod"]
    pub mod_source: EnumParam<ModSourceParam>,
}

impl OscParams {
    /// `enabled_by_default` matches the design, where OSC A and B are lit and C is not.
    fn new(enabled_by_default: bool, octave: i32) -> Self {
        Self {
            enabled: BoolParam::new("Enabled", enabled_by_default),
            waveform: EnumParam::new("Waveform", WaveformParam::Saw),
            octave: IntParam::new("Octave", octave, IntRange::Linear { min: -4, max: 4 }),
            fine: FloatParam::new("Fine", 0.0, FloatRange::Linear { min: -100.0, max: 100.0 })
                .with_unit(" ct")
                .with_step_size(0.1),
            unison: IntParam::new("Unison", 1, IntRange::Linear { min: 1, max: ds_dsp::MAX_UNISON as i32 }),
            detune: FloatParam::new("Detune", 15.0, FloatRange::Linear { min: 0.0, max: 100.0 })
                .with_unit(" ct")
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_step_size(0.1),
            blend: FloatParam::new("Blend", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            warp: FloatParam::new("Warp", 0.5, FloatRange::Linear { min: 0.01, max: 0.99 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            phase: FloatParam::new("Phase", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            phase_random: FloatParam::new("Rand", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            level: FloatParam::new("Level", 0.7, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            pan: pan_param("Pan"),
            filter_enabled: BoolParam::new("To Filter", true),
            filter_send: FloatParam::new("Filter A/B", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0)),
            direct_out: BoolParam::new("Direct Out", false),
            mod_source: EnumParam::new("Mod Source", ModSourceParam::None),
        }
    }
}

/// One AHDSR envelope panel.
#[derive(Params)]
pub struct EnvParams {
    #[id = "att"]
    pub attack: FloatParam,
    #[id = "hold"]
    pub hold: FloatParam,
    #[id = "dec"]
    pub decay: FloatParam,
    #[id = "sus"]
    pub sustain: FloatParam,
    #[id = "rel"]
    pub release: FloatParam,
}

impl EnvParams {
    fn new(attack: f32, decay: f32, sustain: f32, release: f32) -> Self {
        // Skewed ranges: envelope times are perceived logarithmically, so a linear
        // knob would spend most of its travel on times nobody uses.
        let time_range = |max: f32| FloatRange::Skewed { min: 0.0, max, factor: FloatRange::skew_factor(-2.0) };
        Self {
            attack: FloatParam::new("Attack", attack, time_range(10.0))
                .with_unit(" s")
                .with_value_to_string(formatters::v2s_f32_rounded(3)),
            hold: FloatParam::new("Hold", 0.0, time_range(5.0))
                .with_unit(" s")
                .with_value_to_string(formatters::v2s_f32_rounded(3)),
            decay: FloatParam::new("Decay", decay, time_range(20.0))
                .with_unit(" s")
                .with_value_to_string(formatters::v2s_f32_rounded(3)),
            sustain: FloatParam::new("Sustain", sustain, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            release: FloatParam::new("Release", release, time_range(20.0))
                .with_unit(" s")
                .with_value_to_string(formatters::v2s_f32_rounded(3)),
        }
    }

    pub fn to_dsp(&self) -> ds_dsp::EnvelopeSettings {
        ds_dsp::EnvelopeSettings {
            attack: self.attack.value(),
            hold: self.hold.value(),
            decay: self.decay.value(),
            sustain: self.sustain.value(),
            release: self.release.value(),
        }
    }
}

/// One FILTER panel.
#[derive(Params)]
pub struct FilterParams {
    #[id = "on"]
    pub enabled: BoolParam,
    #[id = "mode"]
    pub mode: EnumParam<FilterModeParam>,
    #[id = "cut"]
    pub cutoff: FloatParam,
    #[id = "res"]
    pub resonance: FloatParam,
    #[id = "env"]
    pub env_amount: FloatParam,
    #[id = "key"]
    pub keytrack: FloatParam,
    /// Filter B only: take filter A's output as this filter's input.
    /// Unused on filter A, where feeding a filter its own output would be a loop.
    #[id = "inf1"]
    pub input_from_filter_a: BoolParam,

    // Per-source input enables, matching the A / B / C / N dots on each filter.
    // The oscillators' own A/B send still decides how much reaches each filter;
    // these gate it per source. Not yet read by the DSP.
    #[id = "ina"]
    pub input_a: BoolParam,
    #[id = "inb"]
    pub input_b: BoolParam,
    #[id = "inc"]
    pub input_c: BoolParam,
    #[id = "inn"]
    pub input_noise: BoolParam,

    #[id = "pan"]
    pub pan: FloatParam,
    /// Saturation into the filter. Not yet read by the DSP.
    #[id = "drive"]
    pub drive: FloatParam,
    /// Drive tilt frequency. Not yet read by the DSP.
    #[id = "freq"]
    pub freq: FloatParam,
    /// Dry/wet for the whole filter stage. Not yet read by the DSP.
    #[id = "mix"]
    pub mix: FloatParam,
}

impl FilterParams {
    fn new(enabled: bool, mode: FilterModeParam, cutoff: f32) -> Self {
        Self {
            enabled: BoolParam::new("Enabled", enabled),
            input_from_filter_a: BoolParam::new("F1 Input", false),
            mode: EnumParam::new("Mode", mode),
            // Cutoff is heard logarithmically, so the knob has to be skewed too.
            // No `with_unit` here: v2s_f32_hz_then_khz already emits the unit, and
            // adding another produced "1.1 kHz Hz", which then failed to parse back.
            cutoff: FloatParam::new(
                "Cutoff",
                cutoff,
                FloatRange::Skewed { min: 20.0, max: 20_000.0, factor: FloatRange::skew_factor(-2.0) },
            )
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            resonance: FloatParam::new("Resonance", 0.1, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            env_amount: FloatParam::new("Env Amount", 0.0, FloatRange::Linear { min: -8.0, max: 8.0 })
                .with_unit(" oct")
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_step_size(0.01),
            keytrack: FloatParam::new("Key Track", 0.0, FloatRange::Linear { min: -1.0, max: 1.0 })
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            input_a: BoolParam::new("Input A", enabled),
            input_b: BoolParam::new("Input B", false),
            input_c: BoolParam::new("Input C", false),
            input_noise: BoolParam::new("Input Noise", false),
            pan: pan_param("Filter Pan"),
            drive: FloatParam::new("Drive", 0.0, FloatRange::Linear { min: 0.0, max: 24.0 })
                .with_unit(" dB")
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_step_size(0.1),
            freq: FloatParam::new(
                "Drive Freq",
                1_000.0,
                FloatRange::Skewed { min: 20.0, max: 20_000.0, factor: FloatRange::skew_factor(-2.0) },
            )
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            mix: FloatParam::new("Mix", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
        }
    }
}

/// One LFO panel. None of these reach the DSP yet.
#[derive(Params)]
pub struct LfoParams {
    #[id = "trig"]
    pub trigger: EnumParam<LfoTriggerParam>,
    #[id = "rate"]
    pub rate: FloatParam,
    #[id = "rise"]
    pub rise: FloatParam,
    #[id = "delay"]
    pub delay: FloatParam,
    #[id = "bpm"]
    pub sync_bpm: BoolParam,
    #[id = "trip"]
    pub triplet: BoolParam,
    #[id = "anch"]
    pub anchor: BoolParam,
    #[id = "dot"]
    pub dotted: BoolParam,
}

impl Default for LfoParams {
    fn default() -> Self {
        Self {
            trigger: EnumParam::new("Trigger", LfoTriggerParam::Trigger),
            rate: FloatParam::new(
                "Rate",
                2.0,
                FloatRange::Skewed { min: 0.01, max: 40.0, factor: FloatRange::skew_factor(-2.0) },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            rise: FloatParam::new(
                "Rise",
                0.0,
                FloatRange::Skewed { min: 0.0, max: 10.0, factor: FloatRange::skew_factor(-2.0) },
            )
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(3)),
            delay: FloatParam::new(
                "Delay",
                0.0,
                FloatRange::Skewed { min: 0.0, max: 10.0, factor: FloatRange::skew_factor(-2.0) },
            )
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(3)),
            sync_bpm: BoolParam::new("Sync to BPM", true),
            triplet: BoolParam::new("Triplet", false),
            anchor: BoolParam::new("Anchor", false),
            dotted: BoolParam::new("Dotted", false),
        }
    }
}

#[derive(Params)]
pub struct NoiseParams {
    #[id = "on"]
    pub enabled: BoolParam,
    #[id = "col"]
    pub colour: EnumParam<NoiseColourParam>,
    #[id = "lvl"]
    pub level: FloatParam,
    #[id = "pan"]
    pub pan: FloatParam,
    /// Playback pitch for sampled noise. Not yet read by the DSP.
    #[id = "pitch"]
    pub pitch: FloatParam,
    /// Whether the noise pitch follows the played note. Not yet read by the DSP.
    #[id = "key"]
    pub keytrack: BoolParam,
}

impl Default for NoiseParams {
    fn default() -> Self {
        Self {
            enabled: BoolParam::new("Enabled", false),
            colour: EnumParam::new("Colour", NoiseColourParam::White),
            level: FloatParam::new("Level", 0.2, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            pan: pan_param("Pan"),
            pitch: FloatParam::new("Noise Pitch", 0.0, FloatRange::Linear { min: -24.0, max: 24.0 })
                .with_unit(" st")
                .with_step_size(0.1),
            keytrack: BoolParam::new("Key Track", true),
        }
    }
}

#[derive(Params)]
pub struct VoicingParams {
    #[id = "mode"]
    pub mode: EnumParam<VoiceModeParam>,
    #[id = "poly"]
    pub polyphony: IntParam,
    #[id = "porta"]
    pub portamento: FloatParam,
    #[id = "always"]
    pub always_glide: BoolParam,
    #[id = "velo"]
    pub velocity_curve: FloatParam,
    /// Key-tracking curve for the NOTE box. Not yet read by the DSP.
    #[id = "note"]
    pub note_curve: FloatParam,
}

impl Default for VoicingParams {
    fn default() -> Self {
        Self {
            mode: EnumParam::new("Voice Mode", VoiceModeParam::Poly),
            polyphony: IntParam::new("Polyphony", 8, IntRange::Linear { min: 1, max: ds_dsp::MAX_POLYPHONY as i32 }),
            portamento: FloatParam::new(
                "Portamento",
                0.0,
                FloatRange::Skewed { min: 0.0, max: 5.0, factor: FloatRange::skew_factor(-2.0) },
            )
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(3)),
            always_glide: BoolParam::new("Always Glide", false),
            // 1.0 is linear; above that soft playing thins out faster.
            velocity_curve: FloatParam::new("Velocity Curve", 1.0, FloatRange::Skewed { min: 0.25, max: 4.0, factor: FloatRange::skew_factor(-1.0) })
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            note_curve: FloatParam::new("Note Curve", 1.0, FloatRange::Skewed { min: 0.25, max: 4.0, factor: FloatRange::skew_factor(-1.0) })
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
        }
    }
}

/// The complete parameter tree the host sees.
#[derive(Params)]
pub struct DefaultSynthParams {
    #[nested(id_prefix = "oa", group = "OSC A")]
    pub osc_a: OscParams,
    #[nested(id_prefix = "ob", group = "OSC B")]
    pub osc_b: OscParams,
    #[nested(id_prefix = "oc", group = "OSC C")]
    pub osc_c: OscParams,

    #[nested(id_prefix = "noise", group = "Noise")]
    pub noise: NoiseParams,

    #[nested(id_prefix = "fa", group = "Filter A")]
    pub filter_a: FilterParams,
    #[nested(id_prefix = "fb", group = "Filter B")]
    pub filter_b: FilterParams,

    #[nested(id_prefix = "aeg", group = "Amp Envelope")]
    pub amp_env: EnvParams,
    #[nested(id_prefix = "feg", group = "Filter Envelope")]
    pub filter_env: EnvParams,
    #[nested(id_prefix = "meg", group = "Mod Envelope")]
    pub mod_env: EnvParams,

    #[nested(id_prefix = "lfo1", group = "LFO 1")]
    pub lfo1: LfoParams,
    #[nested(id_prefix = "lfo2", group = "LFO 2")]
    pub lfo2: LfoParams,
    #[nested(id_prefix = "lfo3", group = "LFO 3")]
    pub lfo3: LfoParams,
    #[nested(id_prefix = "lfo4", group = "LFO 4")]
    pub lfo4: LfoParams,

    #[nested(id_prefix = "voc", group = "Voicing")]
    pub voicing: VoicingParams,

    #[id = "master"]
    pub master_gain: FloatParam,
}

impl Default for DefaultSynthParams {
    fn default() -> Self {
        Self {
            // The design shows OSC A and B lit with C dimmed, so match that.
            osc_a: OscParams::new(true, -1),
            osc_b: OscParams::new(true, 0),
            osc_c: OscParams::new(false, -1),
            noise: NoiseParams::default(),
            filter_a: FilterParams::new(true, FilterModeParam::Lowpass, 12_000.0),
            filter_b: FilterParams::new(false, FilterModeParam::Highpass, 200.0),
            amp_env: EnvParams::new(0.005, 0.4, 0.7, 0.25),
            filter_env: EnvParams::new(0.002, 0.3, 0.0, 0.2),
            mod_env: EnvParams::new(0.01, 0.5, 0.5, 0.4),
            lfo1: LfoParams::default(),
            lfo2: LfoParams::default(),
            lfo3: LfoParams::default(),
            lfo4: LfoParams::default(),
            voicing: VoicingParams::default(),
            // The minimum must stay above zero: logarithmic smoothing interpolates
            // in the log domain, so a target of exactly 0.0 yields NaN samples.
            // -60 dB is inaudible, which is what the bottom of the fader means here.
            master_gain: FloatParam::new(
                "Master",
                util::db_to_gain(-6.0),
                FloatRange::Skewed { min: util::db_to_gain(-60.0), max: util::db_to_gain(6.0), factor: FloatRange::gain_skew_factor(-60.0, 6.0) },
            )
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
        }
    }
}
