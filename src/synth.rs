//! A small additive "struck string" synthesiser: a handful of slightly
//! stretched harmonics that decay at different rates, with a soft attack and a
//! release tail so notes never click. Pure sample generation, no audio I/O.

use std::f32::consts::TAU;

use crate::song::Seconds;

pub const SAMPLE_RATE: u32 = 48_000;
/// How long a note rings on after it's released.
pub const RELEASE: Seconds = Seconds(0.12);
const ATTACK: Seconds = Seconds(0.004);
/// Relative strengths of harmonics 1, 2, 3...
const HARMONICS: [f32; 6] = [1.0, 0.5, 0.3, 0.18, 0.1, 0.06];
/// Real strings are slightly stiff, which stretches upper partials sharp.
const INHARMONICITY: f32 = 0.0004;
/// Overall level, leaving headroom for chords.
const GAIN: f32 = 0.9;

#[derive(Debug, Clone, Copy)]
struct Partial {
    /// Radians advanced per sample.
    step: f32,
    amplitude: f32,
    /// Exponential decay rate per second.
    decay: f32,
}

/// One note, rendered as an iterator of mono samples at [`SAMPLE_RATE`].
#[derive(Debug, Clone)]
pub struct PianoVoice {
    partials: Vec<Partial>,
    sample: u32,
    held: u32,
    total: u32,
}

fn samples_in(duration: Seconds) -> u32 {
    (duration.0.max(0.0) * SAMPLE_RATE as f32).round() as u32
}

impl PianoVoice {
    pub fn new(frequency: f32, held: Seconds) -> Self {
        let nyquist = SAMPLE_RATE as f32 / 2.0;
        // Higher notes lose energy faster, as on a real piano.
        let base_decay = 0.8 + frequency / 400.0;
        let norm: f32 = HARMONICS.iter().sum();
        let partials = HARMONICS
            .iter()
            .enumerate()
            .map(|(i, amplitude)| {
                let k = (i + 1) as f32;
                let hz = k * frequency * (1.0 + INHARMONICITY * k * k).sqrt();
                (hz, amplitude, k)
            })
            .filter(|(hz, ..)| *hz < nyquist)
            .map(|(hz, amplitude, k)| Partial {
                step: TAU * hz / SAMPLE_RATE as f32,
                amplitude: amplitude / norm,
                decay: base_decay * (1.0 + 0.6 * (k - 1.0)),
            })
            .collect();
        let held = samples_in(held);
        Self {
            partials,
            sample: 0,
            held,
            total: held + samples_in(RELEASE),
        }
    }

    fn envelope(&self, t: f32) -> f32 {
        let attack = (t / ATTACK.0).min(1.0);
        let release = match self.sample.checked_sub(self.held) {
            None => 1.0,
            Some(after) => {
                let release_len = (self.total - self.held).max(1) as f32;
                1.0 - after as f32 / release_len
            }
        };
        attack * release
    }
}

impl Iterator for PianoVoice {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.sample >= self.total {
            return None;
        }
        let n = self.sample as f32;
        let t = n / SAMPLE_RATE as f32;
        let tone: f32 = self
            .partials
            .iter()
            .map(|p| p.amplitude * (-p.decay * t).exp() * (p.step * n).sin())
            .sum();
        let value = GAIN * self.envelope(t) * tone;
        self.sample += 1;
        Some(value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = (self.total - self.sample) as usize;
        (left, Some(left))
    }
}

impl ExactSizeIterator for PianoVoice {}
