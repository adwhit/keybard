//! Timing tiers, combo/points bookkeeping and end-of-song grades.

use crate::song::Seconds;

/// How far (either side) from a note's start a press still counts as a hit.
pub const HIT_WINDOW: Seconds = Seconds(0.2);
const PERFECT_WINDOW: Seconds = Seconds(0.05);
const GREAT_WINDOW: Seconds = Seconds(0.12);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Timing {
    Perfect,
    Great,
    Good,
}

impl Timing {
    pub const ALL: [Self; 3] = [Self::Perfect, Self::Great, Self::Good];

    /// Grade a press `offset` seconds from the note start (negative = early).
    pub fn from_offset(offset: Seconds) -> Option<Self> {
        // A hair of slack so values exactly on a boundary aren't lost to
        // float rounding.
        const EPSILON: f32 = 1e-4;
        match offset.0.abs() - EPSILON {
            d if d <= PERFECT_WINDOW.0 => Some(Self::Perfect),
            d if d <= GREAT_WINDOW.0 => Some(Self::Great),
            d if d <= HIT_WINDOW.0 => Some(Self::Good),
            _ => None,
        }
    }

    pub fn points(self) -> u32 {
        match self {
            Self::Perfect => 300,
            Self::Great => 200,
            Self::Good => 100,
        }
    }

    /// Contribution to accuracy, 0..=1.
    fn weight(self) -> f32 {
        match self {
            Self::Perfect => 1.0,
            Self::Great => 0.7,
            Self::Good => 0.4,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Perfect => "PERFECT",
            Self::Great => "GREAT",
            Self::Good => "GOOD",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grade {
    S,
    A,
    B,
    C,
    D,
}

impl Grade {
    pub fn label(self) -> &'static str {
        match self {
            Self::S => "S",
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
        }
    }

    fn from_accuracy(accuracy: f32) -> Self {
        match accuracy {
            a if a >= 0.95 => Self::S,
            a if a >= 0.85 => Self::A,
            a if a >= 0.65 => Self::B,
            a if a >= 0.45 => Self::C,
            _ => Self::D,
        }
    }
}

/// Every this many combo, the multiplier goes up by one.
const COMBO_PER_MULTIPLIER: u32 = 10;
const MAX_MULTIPLIER: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Score {
    pub perfect: usize,
    pub great: usize,
    pub good: usize,
    pub missed: usize,
    /// Presses that matched no note.
    pub wrong: usize,
    pub points: u32,
    pub combo: u32,
    pub max_combo: u32,
}

impl Score {
    pub fn multiplier(&self) -> u32 {
        (1 + self.combo / COMBO_PER_MULTIPLIER).min(MAX_MULTIPLIER)
    }

    pub fn hits(&self) -> usize {
        self.perfect + self.great + self.good
    }

    pub fn count(&self, timing: Timing) -> usize {
        match timing {
            Timing::Perfect => self.perfect,
            Timing::Great => self.great,
            Timing::Good => self.good,
        }
    }

    /// Notes that have been decided either way.
    pub fn judged(&self) -> usize {
        self.hits() + self.missed
    }

    pub fn accuracy(&self) -> Option<f32> {
        let judged = self.judged();
        (judged > 0).then(|| {
            let earned: f32 = Timing::ALL
                .iter()
                .map(|t| t.weight() * self.count(*t) as f32)
                .sum();
            earned / judged as f32
        })
    }

    pub fn grade(&self) -> Option<Grade> {
        self.accuracy().map(Grade::from_accuracy)
    }

    pub fn is_full_combo(&self) -> bool {
        self.judged() > 0 && self.missed == 0 && self.wrong == 0
    }

    pub(super) fn record_hit(&mut self, timing: Timing) {
        match timing {
            Timing::Perfect => self.perfect += 1,
            Timing::Great => self.great += 1,
            Timing::Good => self.good += 1,
        }
        self.combo += 1;
        self.max_combo = self.max_combo.max(self.combo);
        self.points += timing.points() * self.multiplier();
    }

    pub(super) fn record_miss(&mut self) {
        self.missed += 1;
        self.combo = 0;
    }

    pub(super) fn record_wrong(&mut self) {
        self.wrong += 1;
        self.combo = 0;
    }
}
