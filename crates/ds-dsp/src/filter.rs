//! Zero-delay-feedback state variable filter (Andrew Simper / Cytomic topology).
//!
//! A naive digital biquad detunes badly as the cutoff approaches Nyquist and goes
//! unstable when modulated quickly. The ZDF SVF pre-warps the cutoff and solves
//! the feedback path analytically, so it stays stable under audio-rate modulation
//! and gives lowpass, highpass, bandpass and notch from the same state.

/// Filter response, matching the UI's model selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
}

impl FilterMode {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Lowpass,
            1 => Self::Highpass,
            2 => Self::Bandpass,
            _ => Self::Notch,
        }
    }
}

/// One stereo-independent SVF channel pair.
#[derive(Debug, Clone, Default)]
pub struct StateVariableFilter {
    ic1: [f32; 2],
    ic2: [f32; 2],
    sample_rate: f32,
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    mode: FilterMode,
}

impl Default for FilterMode {
    fn default() -> Self {
        Self::Lowpass
    }
}

impl StateVariableFilter {
    pub fn new(sample_rate: f32) -> Self {
        let mut filter = Self { sample_rate: sample_rate.max(1.0), ..Default::default() };
        filter.set_params(1_000.0, 0.5);
        filter
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
    }

    pub fn set_mode(&mut self, mode: FilterMode) {
        self.mode = mode;
    }

    /// `cutoff_hz` is clamped below Nyquist; `resonance` is normalised `0.0..=1.0`.
    pub fn set_params(&mut self, cutoff_hz: f32, resonance: f32) {
        let nyquist = self.sample_rate * 0.5;
        // Stop just short of Nyquist: tan() blows up at exactly half the rate.
        let cutoff = cutoff_hz.clamp(20.0, nyquist * 0.98);
        // Bilinear-transform frequency pre-warp keeps the analogue cutoff accurate.
        self.g = (std::f32::consts::PI * cutoff / self.sample_rate).tan();
        // Map 0..1 onto a useful Q range; k is the inverse of Q.
        let q = 0.5 + resonance.clamp(0.0, 1.0) * 9.5;
        self.k = 1.0 / q;
        self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        self.a2 = self.g * self.a1;
        self.a3 = self.g * self.a2;
    }

    pub fn reset(&mut self) {
        self.ic1 = [0.0; 2];
        self.ic2 = [0.0; 2];
    }

    /// Filters one sample on `channel` (0 = left, 1 = right).
    pub fn process(&mut self, input: f32, channel: usize) -> f32 {
        let channel = channel.min(1);
        let ic1 = self.ic1[channel];
        let ic2 = self.ic2[channel];

        let v3 = input - ic2;
        let v1 = self.a1 * ic1 + self.a2 * v3;
        let v2 = ic2 + self.a2 * ic1 + self.a3 * v3;

        self.ic1[channel] = 2.0 * v1 - ic1;
        self.ic2[channel] = 2.0 * v2 - ic2;

        match self.mode {
            FilterMode::Lowpass => v2,
            FilterMode::Highpass => input - self.k * v1 - v2,
            FilterMode::Bandpass => v1,
            FilterMode::Notch => input - self.k * v1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 48_000.0;

    /// Measures steady-state output level for a sine at `frequency`.
    fn response_at(filter: &mut StateVariableFilter, frequency: f32) -> f32 {
        filter.reset();
        let mut peak: f32 = 0.0;
        let total = 8_192;
        for index in 0..total {
            let phase = index as f32 / SAMPLE_RATE * frequency * std::f32::consts::TAU;
            let output = filter.process(phase.sin(), 0);
            // Ignore the settling transient.
            if index > total / 2 {
                peak = peak.max(output.abs());
            }
        }
        peak
    }

    #[test]
    fn lowpass_passes_lows_and_rejects_highs() {
        let mut filter = StateVariableFilter::new(SAMPLE_RATE);
        filter.set_mode(FilterMode::Lowpass);
        filter.set_params(1_000.0, 0.0);

        let low = response_at(&mut filter, 100.0);
        let high = response_at(&mut filter, 12_000.0);
        assert!(low > 0.9, "lowpass attenuated the passband: {low}");
        assert!(high < 0.1, "lowpass let a high through: {high}");
    }

    #[test]
    fn highpass_is_the_mirror_of_lowpass() {
        let mut filter = StateVariableFilter::new(SAMPLE_RATE);
        filter.set_mode(FilterMode::Highpass);
        filter.set_params(1_000.0, 0.0);

        let low = response_at(&mut filter, 100.0);
        let high = response_at(&mut filter, 12_000.0);
        assert!(low < 0.1, "highpass let a low through: {low}");
        assert!(high > 0.9, "highpass attenuated the passband: {high}");
    }

    #[test]
    fn bandpass_peaks_around_the_cutoff() {
        let mut filter = StateVariableFilter::new(SAMPLE_RATE);
        filter.set_mode(FilterMode::Bandpass);
        filter.set_params(1_000.0, 0.7);

        let below = response_at(&mut filter, 100.0);
        let centre = response_at(&mut filter, 1_000.0);
        let above = response_at(&mut filter, 10_000.0);
        assert!(centre > below * 3.0, "bandpass did not favour the centre");
        assert!(centre > above * 3.0, "bandpass did not reject the top");
    }

    #[test]
    fn resonance_lifts_the_cutoff_region() {
        let mut flat = StateVariableFilter::new(SAMPLE_RATE);
        flat.set_mode(FilterMode::Lowpass);
        flat.set_params(1_000.0, 0.0);
        let without = response_at(&mut flat, 1_000.0);

        let mut resonant = StateVariableFilter::new(SAMPLE_RATE);
        resonant.set_mode(FilterMode::Lowpass);
        resonant.set_params(1_000.0, 0.9);
        let with = response_at(&mut resonant, 1_000.0);

        assert!(with > without * 1.5, "resonance did not emphasise the cutoff");
    }

    #[test]
    fn stays_finite_at_extreme_settings() {
        // Sweeping cutoff to the rails under full resonance is exactly the case a
        // naive biquad blows up on.
        let mut filter = StateVariableFilter::new(SAMPLE_RATE);
        filter.set_mode(FilterMode::Lowpass);
        for index in 0..48_000 {
            let sweep = 20.0 + (index as f32 / 48_000.0) * 23_000.0;
            filter.set_params(sweep, 1.0);
            let output = filter.process(if index % 2 == 0 { 1.0 } else { -1.0 }, 0);
            assert!(output.is_finite(), "filter diverged at {sweep} Hz");
            assert!(output.abs() < 100.0, "filter rang out of control: {output}");
        }
    }

    #[test]
    fn channels_hold_independent_state() {
        let mut filter = StateVariableFilter::new(SAMPLE_RATE);
        filter.set_mode(FilterMode::Lowpass);
        filter.set_params(500.0, 0.0);
        // Drive only the left channel; the right must stay silent.
        for _ in 0..1_000 {
            filter.process(1.0, 0);
        }
        let right = filter.process(0.0, 1);
        assert!(right.abs() < 1e-6, "channel state bled across: {right}");
    }
}
