//! Unlockable achievements, judged at the end of each scored run.

use std::collections::BTreeSet;
use std::path::Path;

use crate::error::Error;
use crate::performance::{Grade, Hands, PlayMode, Score};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Achievement {
    FirstClear,
    FullCombo,
    GradeS,
    Combo50,
    Combo100,
    Flawless,
    BothHandsFullCombo,
    SpeedDemon,
    Streak3,
    Streak7,
    Marathon,
    Explorer,
}

/// Everything about a finished run that achievements look at.
#[derive(Debug, Clone, PartialEq)]
pub struct RunSummary {
    pub score: Score,
    pub mode: PlayMode,
    pub hands: Hands,
    pub speed: f32,
    pub streak: u32,
    pub minutes_today: u32,
    pub songs_with_bests: usize,
}

impl Achievement {
    pub const ALL: [Self; 12] = [
        Self::FirstClear,
        Self::FullCombo,
        Self::GradeS,
        Self::Combo50,
        Self::Combo100,
        Self::Flawless,
        Self::BothHandsFullCombo,
        Self::SpeedDemon,
        Self::Streak3,
        Self::Streak7,
        Self::Marathon,
        Self::Explorer,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::FirstClear => "First Steps",
            Self::FullCombo => "Unbroken",
            Self::GradeS => "S Rank",
            Self::Combo50 => "Fifty Strong",
            Self::Combo100 => "Centurion",
            Self::Flawless => "Flawless",
            Self::BothHandsFullCombo => "Two-Handed",
            Self::SpeedDemon => "Speed Demon",
            Self::Streak3 => "Habit Forming",
            Self::Streak7 => "Week Warrior",
            Self::Marathon => "Marathon",
            Self::Explorer => "Explorer",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::FirstClear => "Finish a song in Wait or Play mode",
            Self::FullCombo => "Finish a song without a miss or wrong note",
            Self::GradeS => "Earn an S grade",
            Self::Combo50 => "Reach a 50-note combo",
            Self::Combo100 => "Reach a 100-note combo",
            Self::Flawless => "Finish with 99% accuracy or better",
            Self::BothHandsFullCombo => "Full combo with both hands",
            Self::SpeedDemon => "Finish above full speed with an A or better",
            Self::Streak3 => "Practise three days in a row",
            Self::Streak7 => "Practise seven days in a row",
            Self::Marathon => "Practise 30 minutes in a day",
            Self::Explorer => "Set personal bests on three songs",
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::FirstClear => "first-clear",
            Self::FullCombo => "full-combo",
            Self::GradeS => "grade-s",
            Self::Combo50 => "combo-50",
            Self::Combo100 => "combo-100",
            Self::Flawless => "flawless",
            Self::BothHandsFullCombo => "both-hands-full-combo",
            Self::SpeedDemon => "speed-demon",
            Self::Streak3 => "streak-3",
            Self::Streak7 => "streak-7",
            Self::Marathon => "marathon",
            Self::Explorer => "explorer",
        }
    }

    fn is_earned_by(self, run: &RunSummary) -> bool {
        let score = &run.score;
        let grade = score.grade();
        match self {
            Self::FirstClear => true,
            Self::FullCombo => score.is_full_combo(),
            Self::GradeS => grade == Some(Grade::S),
            Self::Combo50 => score.max_combo >= 50,
            Self::Combo100 => score.max_combo >= 100,
            Self::Flawless => score.accuracy().is_some_and(|a| a >= 0.99),
            Self::BothHandsFullCombo => score.is_full_combo() && run.hands == Hands::Both,
            Self::SpeedDemon => run.speed > 1.0 + 1e-3 && grade.is_some_and(|g| g <= Grade::A),
            Self::Streak3 => run.streak >= 3,
            Self::Streak7 => run.streak >= 7,
            Self::Marathon => run.minutes_today >= 30,
            Self::Explorer => run.songs_with_bests >= 3,
        }
    }

    /// Everything a run qualifies for (nothing unless something was hit).
    pub fn earned_by(run: &RunSummary) -> Vec<Self> {
        if run.score.hits() == 0 || !run.mode.is_scored() {
            return Vec::new();
        }
        Self::ALL
            .into_iter()
            .filter(|a| a.is_earned_by(run))
            .collect()
    }
}

/// The achievements unlocked so far.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Achievements {
    owned: BTreeSet<Achievement>,
}

impl Achievements {
    pub fn count(&self) -> usize {
        self.owned.len()
    }

    pub fn has(&self, achievement: Achievement) -> bool {
        self.owned.contains(&achievement)
    }

    /// Add whatever the run earned; return just the newly unlocked ones.
    pub fn unlock(&mut self, run: &RunSummary) -> Vec<Achievement> {
        Achievement::earned_by(run)
            .into_iter()
            .filter(|a| self.owned.insert(*a))
            .collect()
    }

    pub fn load(path: &Path) -> Result<Self, Error> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Self {
                owned: text
                    .lines()
                    .filter_map(|l| Achievement::ALL.into_iter().find(|a| a.slug() == l.trim()))
                    .collect(),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(Error::Io {
                path: path.to_owned(),
                source,
            }),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), Error> {
        let io = |source| Error::Io {
            path: path.to_owned(),
            source,
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let text: String = self
            .owned
            .iter()
            .map(|a| format!("{}\n", a.slug()))
            .collect();
        std::fs::write(path, text).map_err(io)
    }
}
