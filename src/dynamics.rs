//! How hard notes are played: MIDI velocity, dynamic markings and loudness.

use crate::song::{Seconds, Song};

/// MIDI velocity, 0..=127.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Velocity(u8);

impl Velocity {
    /// Mezzo-forte: what notes get when the source doesn't say.
    pub const DEFAULT: Self = Self(80);

    pub fn new(value: u8) -> Self {
        Self(value.min(127))
    }

    pub fn value(self) -> u8 {
        self.0
    }

    /// Amplitude multiplier for the synth, 0..=1 (perceptually curved).
    pub fn gain(self) -> f32 {
        (f32::from(self.0) / 127.0).powf(1.6).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dynamic {
    Pianissimo,
    Piano,
    MezzoPiano,
    MezzoForte,
    Forte,
    Fortissimo,
}

impl Dynamic {
    pub fn from_velocity(velocity: Velocity) -> Self {
        match velocity.0 {
            0..=31 => Self::Pianissimo,
            32..=55 => Self::Piano,
            56..=71 => Self::MezzoPiano,
            72..=91 => Self::MezzoForte,
            92..=111 => Self::Forte,
            _ => Self::Fortissimo,
        }
    }

    pub const ALL: [Self; 6] = [
        Self::Pianissimo,
        Self::Piano,
        Self::MezzoPiano,
        Self::MezzoForte,
        Self::Forte,
        Self::Fortissimo,
    ];

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.label() == label)
    }

    /// A typical velocity for playing at this level.
    pub fn velocity(self) -> Velocity {
        Velocity(match self {
            Self::Pianissimo => 24,
            Self::Piano => 45,
            Self::MezzoPiano => 64,
            Self::MezzoForte => 80,
            Self::Forte => 100,
            Self::Fortissimo => 118,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pianissimo => "pp",
            Self::Piano => "p",
            Self::MezzoPiano => "mp",
            Self::MezzoForte => "mf",
            Self::Forte => "f",
            Self::Fortissimo => "ff",
        }
    }
}

/// Dynamic markings at the start of each bar whose (duration-weighted)
/// average level differs from the last marking.
pub fn markings(song: &Song) -> Vec<(Seconds, Dynamic)> {
    let bar = song.tempo.bar_length().0;
    let bars = (song.duration().0 / bar).ceil() as usize;
    (0..bars)
        .filter_map(|i| {
            let (from, to) = (i as f32 * bar, (i + 1) as f32 * bar);
            let (weighted, total) = song
                .notes()
                .iter()
                .filter(|n| n.start.0 >= from - 1e-4 && n.start.0 < to - 1e-4)
                .fold((0.0_f32, 0.0_f32), |(w, t), n| {
                    let d = n.duration.0.max(1e-3);
                    (w + f32::from(n.velocity.value()) * d, t + d)
                });
            (total > 0.0).then(|| {
                let mean = (weighted / total).round() as u8;
                (Seconds(from), Dynamic::from_velocity(Velocity::new(mean)))
            })
        })
        .fold(
            Vec::new(),
            |mut marks: Vec<(Seconds, Dynamic)>, (at, level)| {
                if marks.last().is_none_or(|(_, last)| *last != level) {
                    marks.push((at, level));
                }
                marks
            },
        )
}
