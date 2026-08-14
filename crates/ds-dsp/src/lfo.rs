//! Low-frequency oscillator.
//!
//! Shares nothing with [`crate::oscillator`] on purpose: an audio oscillator is
//! all about band-limiting, while an LFO runs far below hearing and instead
//! needs a delay, a fade-in, and a choice of how it lines up with each note.

use crate::Rng;

/// LFO shapes. `SampleHold` steps to a new random level once per cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfoShape {
    Sine,
    Triangle,
    SawUp,
    SawDown,
    Square,
    SampleHold,
}

impl LfoShape {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Sine,
            1 => Self::Triangle,
            2 => Self::SawUp,
            3 => Self::SawDown,
            4 => Self::Square,
            _ => Self::SampleHold,
        }
    }
}

/// How an LFO lines up with the notes being played.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfoTrigger {
    /// Restarts from phase zero on every note.
    Trigger,
    /// Never restarts; every voice reads one shared, continuously running phase.
    Free,
    /// Restarts per note and stops after one cycle, so it behaves as an envelope.
    Envelope,
}

impl LfoTrigger {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Trigger,
            1 => Self::Free,
            _ => Self::Envelope,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LfoSettings {
    pub shape: LfoShape,
    pub trigger: LfoTrigger,
    /// Cycles per second. Tempo sync is resolved to Hz before it reaches here so
    /// this module never needs to know about bars or the host's transport.
    pub frequency: f32,
    /// Seconds of silence before the LFO starts moving.
    pub delay: f32,
    /// Seconds spent fading from no depth to full depth, after the delay.
    pub rise: f32,
}

impl Default for LfoSettings {
    fn default() -> Self {
        Self {
            shape: LfoShape::Sine,
            trigger: LfoTrigger::Trigger,
            frequency: 2.0,
            delay: 0.0,
            rise: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Lfo {
    phase: f32,
    /// Samples since the last trigger, used by the delay and rise stages.
    ///
    /// Counted as an integer rather than accumulated in seconds: adding
    /// 1/sample_rate tens of thousands of times drifts enough that a delay can
    /// overrun its own boundary by a sample.
    elapsed_samples: u64,
    sample_rate: f32,
    rng: Rng,
    /// Current sample-and-hold level and the cycle it was drawn on.
    held: f32,
    held_cycle: i64,
    /// Completed cycles since the last trigger, which is what steps sample-and-hold.
    cycle: i64,
    /// Set once a one-shot LFO has completed its cycle.
    finished: bool,
    value: f32,
}

impl Lfo {
    pub fn new(seed: u32) -> Self {
        Self {
            phase: 0.0,
            elapsed_samples: 0,
            sample_rate: 44_100.0,
            rng: Rng::new(seed),
            held: 0.0,
            held_cycle: -1,
            cycle: 0,
            finished: false,
            value: 0.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
    }

    /// Restarts the delay, the rise and the phase.
    pub fn trigger(&mut self) {
        self.phase = 0.0;
        self.elapsed_samples = 0;
        self.held_cycle = -1;
        self.cycle = 0;
        self.finished = false;
        self.value = 0.0;
    }

    /// Clears everything, including a free-running phase.
    pub fn reset(&mut self) {
        self.trigger();
    }

    /// The most recent output, for meters and displays.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// True once a one-shot LFO has run its single cycle.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Advances one sample and returns the bipolar output in `-1.0..=1.0`.
    pub fn process(&mut self, settings: &LfoSettings) -> f32 {
        let increment = (settings.frequency.max(0.0) / self.sample_rate).clamp(0.0, 0.5);
        let elapsed = self.elapsed_samples as f32 / self.sample_rate;

        // The delay holds the output at zero without starting the phase, so a
        // delayed LFO begins at the top of its shape rather than partway in.
        if elapsed < settings.delay {
            self.elapsed_samples += 1;
            self.value = 0.0;
            return 0.0;
        }

        if self.finished {
            self.value = 0.0;
            return 0.0;
        }

        // Sample-and-hold draws once per cycle, so it needs the cycle counter
        // rather than the phase to know when to step.
        if settings.shape == LfoShape::SampleHold && self.held_cycle != self.cycle {
            self.held_cycle = self.cycle;
            self.held = self.rng.next_bipolar();
        }
        let raw = shape_at(settings.shape, self.phase, self.held);

        // Rise measured from the end of the delay, so the two stack rather than
        // the fade being eaten by the wait.
        let since_start = elapsed - settings.delay;
        let depth = if settings.rise <= f32::EPSILON {
            1.0
        } else {
            (since_start / settings.rise).clamp(0.0, 1.0)
        };

        self.phase += increment;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.cycle += 1;
            // A one-shot stops at the end of its first cycle.
            if settings.trigger == LfoTrigger::Envelope {
                self.finished = true;
            }
        }
        self.elapsed_samples += 1;

        self.value = raw * depth;
        self.value
    }

    /// Unipolar form in `0.0..=1.0`, for destinations that have no negative side.
    pub fn process_unipolar(&mut self, settings: &LfoSettings) -> f32 {
        self.process(settings) * 0.5 + 0.5
    }
}

fn shape_at(shape: LfoShape, phase: f32, held: f32) -> f32 {
    match shape {
        LfoShape::Sine => (phase * std::f32::consts::TAU).sin(),
        LfoShape::Triangle => {
            // Starts at 0, peaks at a quarter, troughs at three quarters.
            if phase < 0.25 {
                phase * 4.0
            } else if phase < 0.75 {
                2.0 - phase * 4.0
            } else {
                phase * 4.0 - 4.0
            }
        }
        LfoShape::SawUp => phase * 2.0 - 1.0,
        LfoShape::SawDown => 1.0 - phase * 2.0,
        LfoShape::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        // The level is redrawn once per cycle by the caller and simply held here.
        LfoShape::SampleHold => held,
    }
}

/// Cycles per second for a tempo-locked division.
///
/// `whole_notes` is the cycle length: a quarter note is 0.25, one bar in 4/4 is
/// 1.0. Triplets fit three in the space of two and dotted notes take half again
/// as long.
pub fn sync_frequency(whole_notes: f32, bpm: f32, triplet: bool, dotted: bool) -> f32 {
    let seconds_per_whole_note = 240.0 / bpm.max(1.0);
    let modifier = if triplet {
        2.0 / 3.0
    } else if dotted {
        1.5
    } else {
        1.0
    };
    let seconds = whole_notes.max(1e-6) * seconds_per_whole_note * modifier;
    1.0 / seconds.max(1e-6)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 48_000.0;

    fn run(lfo: &mut Lfo, settings: &LfoSettings, samples: usize) -> Vec<f32> {
        (0..samples).map(|_| lfo.process(settings)).collect()
    }

    fn settings(shape: LfoShape) -> LfoSettings {
        LfoSettings { shape, frequency: 1.0, ..LfoSettings::default() }
    }

    #[test]
    fn every_shape_stays_bipolar_and_finite() {
        for shape in [
            LfoShape::Sine, LfoShape::Triangle, LfoShape::SawUp,
            LfoShape::SawDown, LfoShape::Square, LfoShape::SampleHold,
        ] {
            let mut lfo = Lfo::new(7);
            lfo.set_sample_rate(SAMPLE_RATE);
            lfo.trigger();
            for value in run(&mut lfo, &settings(shape), SAMPLE_RATE as usize) {
                assert!(value.is_finite(), "{shape:?} produced {value}");
                assert!((-1.0..=1.0).contains(&value), "{shape:?} left range: {value}");
            }
        }
    }

    #[test]
    fn a_full_cycle_takes_one_over_the_frequency() {
        let mut lfo = Lfo::new(1);
        lfo.set_sample_rate(SAMPLE_RATE);
        lfo.trigger();
        let config = LfoSettings { shape: LfoShape::SawUp, frequency: 2.0, ..LfoSettings::default() };

        // A rising saw runs -1 to +1 across a cycle, so halfway through the first
        // cycle at 2 Hz, a quarter of a second in, it should be near zero.
        let samples = (SAMPLE_RATE / 2.0 / 2.0) as usize;
        let values = run(&mut lfo, &config, samples);
        assert!(values.last().unwrap().abs() < 0.02, "expected mid-cycle, got {}", values.last().unwrap());
    }

    #[test]
    fn delay_holds_the_output_at_zero_then_starts_from_the_top() {
        let mut lfo = Lfo::new(1);
        lfo.set_sample_rate(SAMPLE_RATE);
        lfo.trigger();
        let config = LfoSettings { shape: LfoShape::SawUp, frequency: 1.0, delay: 0.25, ..LfoSettings::default() };

        let during_delay = run(&mut lfo, &config, (SAMPLE_RATE * 0.25) as usize);
        assert!(during_delay.iter().all(|value| *value == 0.0), "output moved during the delay");

        // The phase should not have advanced while waiting, so the saw begins at -1.
        let first = lfo.process(&config);
        assert!((first + 1.0).abs() < 0.01, "saw did not start from the bottom: {first}");
    }

    #[test]
    fn rise_fades_the_depth_in_after_the_delay() {
        let mut lfo = Lfo::new(1);
        lfo.set_sample_rate(SAMPLE_RATE);
        lfo.trigger();
        // Square holds at +1, so the output is purely the rise envelope.
        let config = LfoSettings { shape: LfoShape::Square, frequency: 0.25, rise: 1.0, ..LfoSettings::default() };

        let quarter = run(&mut lfo, &config, (SAMPLE_RATE * 0.25) as usize);
        let at_quarter = *quarter.last().unwrap();
        assert!((at_quarter - 0.25).abs() < 0.02, "rise was {at_quarter} a quarter in");

        run(&mut lfo, &config, (SAMPLE_RATE * 0.75) as usize);
        let after = lfo.process(&config);
        assert!((after - 1.0).abs() < 0.02, "rise did not reach full depth: {after}");
    }

    #[test]
    fn delay_and_rise_stack_rather_than_overlapping() {
        let mut lfo = Lfo::new(1);
        lfo.set_sample_rate(SAMPLE_RATE);
        lfo.trigger();
        let config = LfoSettings { shape: LfoShape::Square, frequency: 0.1, delay: 0.5, rise: 0.5, ..LfoSettings::default() };

        // Half a second of delay, then the rise only just beginning.
        run(&mut lfo, &config, (SAMPLE_RATE * 0.5) as usize);
        let at_start_of_rise = lfo.process(&config);
        assert!(at_start_of_rise.abs() < 0.02, "rise had already progressed: {at_start_of_rise}");

        run(&mut lfo, &config, (SAMPLE_RATE * 0.5) as usize);
        let after = lfo.process(&config);
        assert!((after - 1.0).abs() < 0.02, "rise did not finish half a second later: {after}");
    }

    #[test]
    fn a_one_shot_stops_after_a_single_cycle() {
        let mut lfo = Lfo::new(1);
        lfo.set_sample_rate(SAMPLE_RATE);
        lfo.trigger();
        let config = LfoSettings {
            shape: LfoShape::Sine,
            trigger: LfoTrigger::Envelope,
            frequency: 4.0,
            ..LfoSettings::default()
        };

        // A quarter of a second is one full cycle at 4 Hz.
        run(&mut lfo, &config, (SAMPLE_RATE * 0.25) as usize + 4);
        assert!(lfo.is_finished(), "one-shot never finished");
        assert_eq!(lfo.process(&config), 0.0, "a finished one-shot should stay silent");
    }

    #[test]
    fn a_looping_lfo_keeps_going_past_its_first_cycle() {
        let mut lfo = Lfo::new(1);
        lfo.set_sample_rate(SAMPLE_RATE);
        lfo.trigger();
        let config = LfoSettings { shape: LfoShape::Sine, frequency: 4.0, ..LfoSettings::default() };
        let values = run(&mut lfo, &config, SAMPLE_RATE as usize);
        assert!(!lfo.is_finished());
        // Four cycles in a second means the tail is still swinging.
        // A whole cycle at 4 Hz, so the window is guaranteed to contain a full swing.
        let tail = &values[values.len() - 12_000..];
        let span = tail.iter().cloned().fold(f32::MIN, f32::max) - tail.iter().cloned().fold(f32::MAX, f32::min);
        assert!(span > 1.5, "LFO stopped moving, span was {span}");
    }

    #[test]
    fn trigger_restarts_the_phase() {
        let mut lfo = Lfo::new(1);
        lfo.set_sample_rate(SAMPLE_RATE);
        lfo.trigger();
        let config = settings(LfoShape::SawUp);
        run(&mut lfo, &config, (SAMPLE_RATE * 0.5) as usize);
        lfo.trigger();
        let first = lfo.process(&config);
        assert!((first + 1.0).abs() < 0.01, "phase did not reset: {first}");
    }

    #[test]
    fn sample_and_hold_holds_its_level_across_a_cycle() {
        let mut lfo = Lfo::new(99);
        lfo.set_sample_rate(SAMPLE_RATE);
        lfo.trigger();
        let config = LfoSettings { shape: LfoShape::SampleHold, frequency: 4.0, ..LfoSettings::default() };

        // Within one cycle the level must not change.
        let first = lfo.process(&config);
        let quarter_cycle = (SAMPLE_RATE / 4.0 / 4.0) as usize;
        let values = run(&mut lfo, &config, quarter_cycle);
        assert!(values.iter().all(|value| (*value - first).abs() < 1e-6), "sample and hold drifted");
    }

    #[test]
    fn sample_and_hold_steps_to_a_new_level_each_cycle() {
        let mut lfo = Lfo::new(4);
        lfo.set_sample_rate(SAMPLE_RATE);
        lfo.trigger();
        let config = LfoSettings { shape: LfoShape::SampleHold, frequency: 20.0, ..LfoSettings::default() };

        let values = run(&mut lfo, &config, SAMPLE_RATE as usize);
        let distinct = values.iter().fold(Vec::new(), |mut seen: Vec<f32>, value| {
            if !seen.iter().any(|existing| (existing - value).abs() < 1e-6) {
                seen.push(*value);
            }
            seen
        });
        // Twenty cycles in a second should produce a good spread of levels.
        assert!(distinct.len() > 5, "only {} distinct levels", distinct.len());
    }

    #[test]
    fn sync_frequency_matches_the_tempo() {
        // At 120 bpm a whole note is two seconds, so a bar-long cycle is 0.5 Hz.
        assert!((sync_frequency(1.0, 120.0, false, false) - 0.5).abs() < 1e-5);
        // A quarter note is half a second, so 2 Hz.
        assert!((sync_frequency(0.25, 120.0, false, false) - 2.0).abs() < 1e-5);
        // Triplets are faster, dotted slower.
        assert!(sync_frequency(0.25, 120.0, true, false) > 2.0);
        assert!(sync_frequency(0.25, 120.0, false, true) < 2.0);
    }

    #[test]
    fn zero_frequency_leaves_the_lfo_parked() {
        let mut lfo = Lfo::new(1);
        lfo.set_sample_rate(SAMPLE_RATE);
        lfo.trigger();
        let config = LfoSettings { shape: LfoShape::Sine, frequency: 0.0, ..LfoSettings::default() };
        let values = run(&mut lfo, &config, 1_000);
        assert!(values.iter().all(|value| value.abs() < 1e-6), "a stopped LFO should not move");
    }
}
