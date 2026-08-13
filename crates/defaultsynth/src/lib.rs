//! DefaultSynth: a polyphonic synthesiser exported as both CLAP and VST3.
//!
//! Licensing note: NIH-plug itself is ISC, but the VST3 bindings behind
//! `nih_export_vst3!` are GPLv3. Shipping the VST3 build therefore puts this
//! whole plugin under the GPLv3. The CLAP build carries no such condition.

use nih_plug::prelude::*;
use std::num::NonZeroU32;
use std::sync::Arc;

pub mod editor;
pub mod params;

use ds_dsp::{OscSettings, SynthEngine, VoiceSettings, VoicingSettings};
use nih_plug_vizia::ViziaState;
use params::{DefaultSynthParams, OscParams};

/// Longest run of samples rendered between checks for the next note event.
/// Small enough that note timing stays sample-accurate to the ear, large enough
/// that per-block parameter reads are not paid per sample.
const MAX_BLOCK_LEN: usize = 64;

pub struct DefaultSynth {
    params: Arc<DefaultSynthParams>,
    /// Window size and open/closed state, persisted with the session by the host.
    editor_state: Arc<ViziaState>,
    engine: SynthEngine,
    sample_rate: f32,
}

impl Default for DefaultSynth {
    fn default() -> Self {
        Self {
            params: Arc::new(DefaultSynthParams::default()),
            editor_state: editor::default_state(),
            engine: SynthEngine::new(44_100.0),
            sample_rate: 44_100.0,
        }
    }
}

impl DefaultSynth {
    /// Snapshots the parameter tree into the plain settings the DSP core takes.
    ///
    /// `steps` is how many samples the block covers. Smoothers are advanced by
    /// that many steps in one go: advancing them only once per block would make
    /// every parameter ramp finish `MAX_BLOCK_LEN` times too slowly.
    fn voice_settings(&self, steps: u32) -> VoiceSettings {
        let params = &self.params;
        VoiceSettings {
            osc: [
                osc_settings(&params.osc_a, steps),
                osc_settings(&params.osc_b, steps),
                osc_settings(&params.osc_c, steps),
            ],
            noise_enabled: params.noise.enabled.value(),
            noise_colour: params.noise.colour.value().to_dsp(),
            noise_level: params.noise.level.smoothed.next_step(steps),
            noise_pan: params.noise.pan.smoothed.next_step(steps),
            amp_env: params.amp_env.to_dsp(),
            filter_env: params.filter_env.to_dsp(),
            filter_a_enabled: params.filter_a.enabled.value(),
            filter_a_mode: params.filter_a.mode.value().to_dsp(),
            filter_a_cutoff: params.filter_a.cutoff.smoothed.next_step(steps),
            filter_a_resonance: params.filter_a.resonance.smoothed.next_step(steps),
            filter_a_env_amount: params.filter_a.env_amount.smoothed.next_step(steps),
            filter_a_keytrack: params.filter_a.keytrack.value(),
            filter_b_enabled: params.filter_b.enabled.value(),
            filter_b_mode: params.filter_b.mode.value().to_dsp(),
            filter_b_cutoff: params.filter_b.cutoff.smoothed.next_step(steps),
            filter_b_resonance: params.filter_b.resonance.smoothed.next_step(steps),
            filter_b_input_from_a: params.filter_b.input_from_filter_a.value(),
            velocity_curve: params.voicing.velocity_curve.value(),
        }
    }

    /// Same snapshot without advancing any smoother.
    ///
    /// Note-on needs the current oscillator configuration, but an event is not a
    /// unit of time: advancing the smoothers here would make ramp speed depend on
    /// how many notes the player happens to be triggering.
    fn voice_settings_peek(&self) -> VoiceSettings {
        self.voice_settings(0)
    }

    fn voicing_settings(&self) -> VoicingSettings {
        VoicingSettings {
            mode: self.params.voicing.mode.value().to_dsp(),
            polyphony: self.params.voicing.polyphony.value() as usize,
            portamento_seconds: self.params.voicing.portamento.value(),
            always_glide: self.params.voicing.always_glide.value(),
        }
    }
}

fn osc_settings(params: &OscParams, steps: u32) -> OscSettings {
    OscSettings {
        enabled: params.enabled.value(),
        waveform: params.waveform.value().to_dsp(),
        octave: params.octave.value(),
        fine_cents: params.fine.value(),
        unison: params.unison.value() as usize,
        detune_cents: params.detune.smoothed.next_step(steps),
        blend: params.blend.smoothed.next_step(steps),
        warp: params.warp.smoothed.next_step(steps),
        phase: params.phase.value(),
        phase_random: params.phase_random.value(),
        level: params.level.smoothed.next_step(steps),
        pan: params.pan.smoothed.next_step(steps),
        filter_send: params.filter_send.smoothed.next_step(steps),
        filter_enabled: params.filter_enabled.value(),
    }
}

impl Plugin for DefaultSynth {
    const NAME: &'static str = "DefaultSynth";
    const VENDOR: &'static str = "DefaultSynth";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "info@example.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        // An instrument has no main input, only a stereo output.
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[],
        aux_output_ports: &[],
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.editor_state.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        self.engine.set_sample_rate(buffer_config.sample_rate);
        true
    }

    fn reset(&mut self) {
        // The host calls this on transport jumps; leaving voices ringing would
        // bleed the previous playback position into the new one.
        self.engine.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let total_samples = buffer.samples();
        let mut block_start = 0;
        let mut next_event = context.next_event();

        while block_start < total_samples {
            // Render up to the next event so note timing lands on the right sample.
            let mut block_end = (block_start + MAX_BLOCK_LEN).min(total_samples);
            loop {
                match next_event {
                    Some(event) if (event.timing() as usize) <= block_start => {
                        self.handle_event(event);
                        next_event = context.next_event();
                    }
                    Some(event) if (event.timing() as usize) < block_end => {
                        block_end = event.timing() as usize;
                        break;
                    }
                    _ => break,
                }
            }

            let settings = self.voice_settings((block_end - block_start) as u32);
            let output = buffer.as_slice();
            for sample_index in block_start..block_end {
                let (left, right) = self.engine.process(&settings);
                // Master gain is the one value cheap enough to smooth per sample,
                // and the one where a stepped ramp would be most audible.
                let gain = self.params.master_gain.smoothed.next();
                if let Some(channel) = output.first_mut() {
                    channel[sample_index] = left * gain;
                }
                if let Some(channel) = output.get_mut(1) {
                    channel[sample_index] = right * gain;
                }
            }

            block_start = block_end.max(block_start + 1);
        }

        ProcessStatus::Normal
    }
}

impl DefaultSynth {
    fn handle_event(&mut self, event: NoteEvent<()>) {
        let voice_settings = self.voice_settings_peek();
        let voicing = self.voicing_settings();
        match event {
            NoteEvent::NoteOn { note, velocity, voice_id, channel, .. } => {
                self.engine.note_on(note, velocity, voice_id.unwrap_or(-1), channel, &voice_settings, &voicing);
            }
            NoteEvent::NoteOff { note, .. } => {
                self.engine.note_off(note, &voicing);
            }
            // A choke means the host wants the voice gone now, with no tail.
            NoteEvent::Choke { .. } => self.engine.reset(),
            NoteEvent::MidiCC { cc, value, .. } => {
                // CC 123 is All Notes Off; hosts send it on panic.
                if cc == 123 && value > 0.0 {
                    self.engine.all_notes_off();
                }
            }
            _ => {}
        }
    }
}

impl ClapPlugin for DefaultSynth {
    const CLAP_ID: &'static str = "com.defaultsynth.defaultsynth";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Polyphonic synthesiser");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for DefaultSynth {
    // Must be exactly 16 bytes and must never change once released, or hosts will
    // fail to reconnect the plugin to sessions that already reference it.
    const VST3_CLASS_ID: [u8; 16] = *b"DefaultSynth0001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Synth,
        Vst3SubCategory::Stereo,
    ];
}

nih_export_clap!(DefaultSynth);
nih_export_vst3!(DefaultSynth);

#[cfg(test)]
mod tests;
