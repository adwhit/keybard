//! The daily challenge: one song from the playlist per day, with a target.

use crate::performance::{Grade, PlayMode};
use crate::streak::Day;

/// What the challenge asks for.
pub const TARGET_GRADE: Grade = Grade::B;
pub const TARGET_MODE: PlayMode = PlayMode::Play;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Challenge {
    /// Index into the playlist.
    pub song: usize,
}

/// Today's challenge for a playlist of `songs` songs (one new song a day).
pub fn for_day(day: Day, songs: usize) -> Challenge {
    Challenge {
        song: day.0 as usize % songs.max(1),
    }
}

impl Challenge {
    pub fn is_met_by(self, song: usize, mode: PlayMode, grade: Option<Grade>) -> bool {
        song == self.song && mode == TARGET_MODE && grade.is_some_and(|g| g <= TARGET_GRADE)
    }

    pub fn describe(self, title: &str) -> String {
        format!(
            "TODAY'S CHALLENGE: {title} - {} or better in {} mode",
            TARGET_GRADE.label(),
            TARGET_MODE.label()
        )
    }
}
