//! Custom-drawn controls for the reference design.
//!
//! VIZIA ships sliders and buttons but nothing rotary, and the design is built
//! almost entirely out of dials and dark display wells, so these are drawn onto
//! the canvas directly.

mod envelope_display;
mod knob;
mod wave_display;

pub use envelope_display::EnvelopeDisplay;
pub use knob::Knob;
pub use wave_display::WaveDisplay;
