//! Noise generator with the colours offered by the UI's NOISE selector.

use crate::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseColour {
    White,
    Pink,
    Brown,
}

impl NoiseColour {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::White,
            1 => Self::Pink,
            _ => Self::Brown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Noise {
    rng: Rng,
    /// Paul Kellet's pink-noise filter bank state.
    pink: [f32; 7],
    brown: f32,
    colour: NoiseColour,
}

impl Noise {
    pub fn new(seed: u32) -> Self {
        Self { rng: Rng::new(seed), pink: [0.0; 7], brown: 0.0, colour: NoiseColour::White }
    }

    pub fn set_colour(&mut self, colour: NoiseColour) {
        self.colour = colour;
    }

    pub fn reset(&mut self) {
        self.pink = [0.0; 7];
        self.brown = 0.0;
    }

    pub fn process(&mut self) -> f32 {
        let white = self.rng.next_bipolar();
        match self.colour {
            NoiseColour::White => white,
            NoiseColour::Pink => {
                // Kellet's economical -3 dB/octave approximation.
                self.pink[0] = 0.99886 * self.pink[0] + white * 0.0555179;
                self.pink[1] = 0.99332 * self.pink[1] + white * 0.0750759;
                self.pink[2] = 0.96900 * self.pink[2] + white * 0.1538520;
                self.pink[3] = 0.86650 * self.pink[3] + white * 0.3104856;
                self.pink[4] = 0.55000 * self.pink[4] + white * 0.5329522;
                self.pink[5] = -0.7616 * self.pink[5] - white * 0.0168980;
                let output = self.pink[0] + self.pink[1] + self.pink[2] + self.pink[3]
                    + self.pink[4] + self.pink[5] + self.pink[6] + white * 0.5362;
                self.pink[6] = white * 0.115926;
                output * 0.11
            }
            NoiseColour::Brown => {
                // Integrated white noise, leaked back toward zero so it cannot
                // wander off into DC over a long note.
                self.brown = (self.brown + white * 0.02).clamp(-1.0, 1.0) * 0.998;
                self.brown * 3.5
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(colour: NoiseColour, count: usize) -> Vec<f32> {
        let mut noise = Noise::new(42);
        noise.set_colour(colour);
        (0..count).map(|_| noise.process()).collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|value| value * value).sum::<f32>() / samples.len() as f32).sqrt()
    }

    /// Counts sign changes, a cheap proxy for how much high-frequency energy there is.
    fn zero_crossings(samples: &[f32]) -> usize {
        samples.windows(2).filter(|pair| (pair[0] < 0.0) != (pair[1] < 0.0)).count()
    }

    #[test]
    fn every_colour_is_audible_and_bounded() {
        for colour in [NoiseColour::White, NoiseColour::Pink, NoiseColour::Brown] {
            let samples = collect(colour, 48_000);
            assert!(rms(&samples) > 0.01, "{colour:?} noise was silent");
            assert!(samples.iter().all(|value| value.is_finite()), "{colour:?} produced non-finite output");
            assert!(samples.iter().all(|value| value.abs() < 8.0), "{colour:?} ran away in level");
        }
    }

    #[test]
    fn darker_colours_cross_zero_less_often() {
        let white = zero_crossings(&collect(NoiseColour::White, 48_000));
        let pink = zero_crossings(&collect(NoiseColour::Pink, 48_000));
        let brown = zero_crossings(&collect(NoiseColour::Brown, 48_000));
        assert!(pink < white, "pink was not darker than white ({pink} vs {white})");
        assert!(brown < pink, "brown was not darker than pink ({brown} vs {pink})");
    }

    #[test]
    fn white_noise_is_roughly_centred() {
        let samples = collect(NoiseColour::White, 96_000);
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!(mean.abs() < 0.02, "white noise had DC offset {mean}");
    }

    #[test]
    fn brown_noise_does_not_drift_to_dc() {
        // The leak term is what stops a random walk from parking at a rail.
        let samples = collect(NoiseColour::Brown, 480_000);
        let tail = &samples[samples.len() - 48_000..];
        let mean = tail.iter().sum::<f32>() / tail.len() as f32;
        assert!(mean.abs() < 1.0, "brown noise drifted to {mean}");
    }

    #[test]
    fn is_deterministic_for_a_given_seed() {
        let first = collect(NoiseColour::White, 512);
        let second = collect(NoiseColour::White, 512);
        assert_eq!(first, second, "same seed produced different noise");
    }
}
