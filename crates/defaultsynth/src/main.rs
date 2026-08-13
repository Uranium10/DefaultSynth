//! Standalone desktop build.
//!
//! Opens the editor and talks to the system audio and MIDI devices directly, so
//! the synth can be played without a plugin host installed. The plugin builds
//! are the real deliverable; this exists for development and for trying it out.
//!
//! Run `defaultsynth --help` for device, sample-rate and buffer-size options.

use nih_plug::prelude::*;

use defaultsynth::DefaultSynth;

fn main() {
    nih_export_standalone::<DefaultSynth>();
}
