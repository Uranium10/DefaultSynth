//! Host-visible parameters, grouped to mirror the panels in the UI design.

use crossbeam::atomic::AtomicCell;
use ds_dsp::LfoCurve;
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

impl LfoTriggerParam {
    pub fn to_dsp(self) -> ds_dsp::LfoTrigger {
        match self {
            Self::Trigger => ds_dsp::LfoTrigger::Trigger,
            Self::Free => ds_dsp::LfoTrigger::Free,
            Self::Envelope => ds_dsp::LfoTrigger::Envelope,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum LfoShapeParam {
    Sine,
    Triangle,
    #[name = "Saw Up"]
    SawUp,
    #[name = "Saw Down"]
    SawDown,
    Square,
    #[name = "S & H"]
    SampleHold,
    /// The shape drawn in the LFO well. Selecting it on its own changes nothing;
    /// any edit in the well switches to it and seeds the curve from whichever
    /// shape was showing.
    Custom,
}

impl LfoShapeParam {
    pub fn to_dsp(self) -> ds_dsp::LfoShape {
        match self {
            Self::Sine => ds_dsp::LfoShape::Sine,
            Self::Triangle => ds_dsp::LfoShape::Triangle,
            Self::SawUp => ds_dsp::LfoShape::SawUp,
            Self::SawDown => ds_dsp::LfoShape::SawDown,
            Self::Square => ds_dsp::LfoShape::Square,
            Self::SampleHold => ds_dsp::LfoShape::SampleHold,
            Self::Custom => ds_dsp::LfoShape::Custom,
        }
    }
}

/// Modulation matrix source, mirroring [`ds_dsp::ModSource`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum ModSourceSlotParam {
    #[name = "—"]
    None,
    #[name = "LFO 1"]
    Lfo1,
    #[name = "LFO 2"]
    Lfo2,
    #[name = "LFO 3"]
    Lfo3,
    #[name = "LFO 4"]
    Lfo4,
    #[name = "Amp Env"]
    AmpEnv,
    #[name = "Filter Env"]
    FilterEnv,
    #[name = "Mod Env"]
    ModEnv,
    Velocity,
    #[name = "Key Track"]
    KeyTrack,
    #[name = "Mod Wheel"]
    ModWheel,
}

impl ModSourceSlotParam {
    pub fn to_dsp(self) -> ds_dsp::ModSource {
        // The index order is shared with the DSP, so a new variant has to be
        // added in the same position on both sides.
        ds_dsp::ModSource::from_index(self as usize)
    }
}

/// Modulation matrix destination, mirroring [`ds_dsp::ModDest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum ModDestSlotParam {
    #[name = "—"]
    None,
    Pitch,
    #[name = "Filter A Cut"]
    FilterACutoff,
    #[name = "Filter B Cut"]
    FilterBCutoff,
    #[name = "Filter A Res"]
    FilterAResonance,
    #[name = "Amplitude"]
    Amplitude,
    Pan,
    #[name = "OSC A Warp"]
    OscAWarp,
    #[name = "OSC B Warp"]
    OscBWarp,
    #[name = "OSC C Warp"]
    OscCWarp,
    #[name = "OSC A Level"]
    OscALevel,
    #[name = "OSC B Level"]
    OscBLevel,
    #[name = "OSC C Level"]
    OscCLevel,
    Detune,
    #[name = "OSC A Detune"]
    OscADetune,
    #[name = "OSC B Detune"]
    OscBDetune,
    #[name = "OSC C Detune"]
    OscCDetune,
    #[name = "OSC A Pan"]
    OscAPan,
    #[name = "OSC B Pan"]
    OscBPan,
    #[name = "OSC C Pan"]
    OscCPan,
}

impl ModDestSlotParam {
    pub fn to_dsp(self) -> ds_dsp::ModDest {
        ds_dsp::ModDest::from_index(self as usize)
    }
}

/// One row of the modulation matrix.
#[derive(Params)]
pub struct ModSlotParams {
    #[id = "src"]
    pub source: EnumParam<ModSourceSlotParam>,
    #[id = "dst"]
    pub destination: EnumParam<ModDestSlotParam>,
    #[id = "amt"]
    pub amount: FloatParam,
}

impl Default for ModSlotParams {
    fn default() -> Self {
        Self {
            source: EnumParam::new("Source", ModSourceSlotParam::None),
            destination: EnumParam::new("Destination", ModDestSlotParam::None),
            amount: FloatParam::new("Amount", 0.0, FloatRange::Linear { min: -1.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
        }
    }
}

impl ModSlotParams {
    pub fn to_dsp(&self) -> ds_dsp::ModSlot {
        ds_dsp::ModSlot {
            source: self.source.value().to_dsp(),
            destination: self.destination.value().to_dsp(),
            amount: self.amount.value(),
        }
    }
}

/// Tempo-locked LFO rate, used instead of the free-running Hz rate while BPM
/// sync is on. The TRIP and DOT toggles scale whichever division is picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum SyncRateParam {
    #[name = "1/128"]
    OneOver128,
    #[name = "1/64"]
    OneOver64,
    #[name = "1/32"]
    OneOver32,
    #[name = "1/16"]
    OneOver16,
    #[name = "1/8"]
    OneOver8,
    #[name = "1/4"]
    OneOver4,
    #[name = "1/2"]
    OneOver2,
    #[name = "1 Bar"]
    OneBar,
    #[name = "2 Bar"]
    TwoBar,
    #[name = "4 Bar"]
    FourBar,
}

impl SyncRateParam {
    /// Length of one LFO cycle in whole notes, before TRIP or DOT is applied.
    ///
    /// A bar is one whole note in 4/4, which is what the design's "1 Bar" means.
    pub fn cycle_in_whole_notes(self) -> f32 {
        match self {
            Self::OneOver128 => 1.0 / 128.0,
            Self::OneOver64 => 1.0 / 64.0,
            Self::OneOver32 => 1.0 / 32.0,
            Self::OneOver16 => 1.0 / 16.0,
            Self::OneOver8 => 1.0 / 8.0,
            Self::OneOver4 => 1.0 / 4.0,
            Self::OneOver2 => 1.0 / 2.0,
            Self::OneBar => 1.0,
            Self::TwoBar => 2.0,
            Self::FourBar => 4.0,
        }
    }

    /// Cycle length in seconds at `bpm`, with the triplet and dotted modifiers.
    ///
    /// Triplets fit three in the space of two, dotted notes take half again as
    /// long; the two are mutually exclusive in every host that offers them, and
    /// triplet wins here if both are somehow set.
    pub fn cycle_seconds(self, bpm: f32, triplet: bool, dotted: bool) -> f32 {
        let seconds_per_whole_note = 240.0 / bpm.max(1.0);
        let modifier = if triplet {
            2.0 / 3.0
        } else if dotted {
            1.5
        } else {
            1.0
        };
        self.cycle_in_whole_notes() * seconds_per_whole_note * modifier
    }
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
    /// How much of `mod_source` is applied. Not yet wired to the DSP.
    #[id = "modamt"]
    pub mod_amount: FloatParam,
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
            // Routing ratio between the two filters. Centre is an even 50:50
            // split, which is why it defaults to the middle rather than to A.
            filter_send: FloatParam::new("Filter A/B", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(Arc::new(|value| {
                    let to_b = (value * 100.0).round();
                    format!("{:.0}:{to_b:.0}", 100.0 - to_b)
                }))
                .with_string_to_value(Arc::new(|string| {
                    // Accepts "40:60" as well as a plain percentage toward B.
                    let text = string.trim();
                    if let Some((_, b)) = text.split_once(':') {
                        return b.trim().parse::<f32>().ok().map(|value| (value / 100.0).clamp(0.0, 1.0));
                    }
                    text.trim_end_matches('%').trim().parse::<f32>().ok().map(|value| (value / 100.0).clamp(0.0, 1.0))
                })),
            direct_out: BoolParam::new("Direct Out", false),
            mod_source: EnumParam::new("Mod Source", ModSourceParam::None),
            mod_amount: FloatParam::new("Mod Amount", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
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
    #[id = "shape"]
    pub shape: EnumParam<LfoShapeParam>,
    /// Free-running rate, used while BPM sync is off.
    #[id = "rate"]
    pub rate: FloatParam,
    /// Tempo-locked rate, used while BPM sync is on. Two parameters rather than
    /// one because the free rate is a continuous sweep and the synced one is a
    /// short list of musical divisions; a host automating either should see the
    /// range it actually expects.
    #[id = "srate"]
    pub sync_rate: EnumParam<SyncRateParam>,
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
            // Custom, so a new LFO opens on the drawable curve rather than on a
            // fixed shape the well would refuse to let you touch.
            shape: EnumParam::new("Shape", LfoShapeParam::Custom),
            rate: FloatParam::new(
                "Rate",
                2.0,
                FloatRange::Skewed { min: 0.01, max: 40.0, factor: FloatRange::skew_factor(-2.0) },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            sync_rate: EnumParam::new("Sync Rate", SyncRateParam::OneOver4),
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

    /// The four drawn LFO shapes.
    ///
    /// Persisted rather than exposed as parameters: a curve is a variable-length
    /// structure, and there is no honest way to present that as a list of floats
    /// a host could automate. `AtomicCell` because the audio thread reads one per
    /// block and must not take a lock to do it.
    ///
    /// Four separate keys rather than one per `LfoParams`: persisted keys are
    /// collected into a single flat map, so a shared name would have the four
    /// LFOs quietly overwriting each other's shapes.
    #[persist = "lfo1curve"]
    pub lfo1_curve: Arc<AtomicCell<LfoCurve>>,
    #[persist = "lfo2curve"]
    pub lfo2_curve: Arc<AtomicCell<LfoCurve>>,
    #[persist = "lfo3curve"]
    pub lfo3_curve: Arc<AtomicCell<LfoCurve>>,
    #[persist = "lfo4curve"]
    pub lfo4_curve: Arc<AtomicCell<LfoCurve>>,

    #[nested(array, group = "Mod Matrix")]
    pub matrix: [ModSlotParams; ds_dsp::MOD_SLOTS],

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
            lfo1_curve: Arc::new(AtomicCell::new(LfoCurve::peak())),
            lfo2_curve: Arc::new(AtomicCell::new(LfoCurve::peak())),
            lfo3_curve: Arc::new(AtomicCell::new(LfoCurve::peak())),
            lfo4_curve: Arc::new(AtomicCell::new(LfoCurve::peak())),
            matrix: Default::default(),
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
