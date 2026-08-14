//! Custom-drawn controls for the reference design.
//!
//! VIZIA ships sliders and buttons but nothing rotary, and the design is built
//! almost entirely out of dials, dot toggles and dark display wells, so these are
//! drawn onto the canvas directly.

mod ab_slider;
mod curve_display;
mod dropdown;
mod envelope_display;
mod field;
mod knob;
mod lfo_editor;
mod power_dot;
mod radio;
mod wave_display;

pub use ab_slider::AbSlider;
pub use curve_display::{CurveBox, FilterResponse};
pub use dropdown::ParamDropdown;
pub use envelope_display::EnvelopeDisplay;
pub use field::Field;
pub use knob::Knob;
pub use lfo_editor::LfoEditor;
pub use power_dot::PowerDot;
pub use radio::RadioDot;
pub use wave_display::WaveDisplay;
