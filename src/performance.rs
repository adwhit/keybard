//! Game rules: how song time advances in each play mode, and how key presses
//! are judged against the score. Pure logic, no Bevy.

use crate::music::MidiNote;
use crate::song::{NoteId, Seconds, Song};

/// How far (either side) from a note's start a press still counts as a hit.
pub const HIT_WINDOW: Seconds = Seconds(0.2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayMode {
    /// The song plays itself, with sound.
    #[default]
    Listen,
    /// Time stops at each note until you play it.
    Wait,
    /// Time keeps flowing; notes you don't play in time are missed.
    Play,
}

impl PlayMode {
    pub fn next(self) -> Self {
        match self {
            Self::Listen => Self::Wait,
            Self::Wait => Self::Play,
            Self::Play => Self::Listen,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Listen => "Listen",
            Self::Wait => "Wait",
            Self::Play => "Play",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteStatus {
    Pending,
    Hit,
    Missed,
    /// Jumped over by seeking; not counted in the score.
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Score {
    pub hit: usize,
    pub missed: usize,
    pub wrong: usize,
}

/// Result of moving the clock forward.
#[derive(Debug, Clone, PartialEq)]
pub struct Advance {
    pub time: Seconds,
    /// Notes the computer played for you (Listen mode), to be sounded.
    pub auto_played: Vec<NoteId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Performance {
    statuses: Vec<NoteStatus>,
    wrong: usize,
}

impl Performance {
    pub fn new(song: &Song) -> Self {
        Self {
            statuses: vec![NoteStatus::Pending; song.notes().len()],
            wrong: 0,
        }
    }

    pub fn status(&self, id: NoteId) -> NoteStatus {
        self.statuses[id.0]
    }

    pub fn score(&self) -> Score {
        let count = |s: NoteStatus| self.statuses.iter().filter(|x| **x == s).count();
        Score {
            hit: count(NoteStatus::Hit),
            missed: count(NoteStatus::Missed),
            wrong: self.wrong,
        }
    }

    /// Prepare to play from `time`: everything from there on is pending again,
    /// anything earlier that was never played is skipped.
    pub fn seek(&mut self, song: &Song, time: Seconds) {
        song.iter().for_each(|(id, note)| {
            let status = &mut self.statuses[id.0];
            *status = match *status {
                _ if note.start >= time => NoteStatus::Pending,
                NoteStatus::Pending => NoteStatus::Skipped,
                other => other,
            };
        });
    }

    pub fn reset(&mut self, song: &Song) {
        *self = Self::new(song);
    }

    fn pending_starting_before(
        &self,
        song: &Song,
        before: Seconds,
    ) -> impl Iterator<Item = NoteId> {
        song.iter()
            .take_while(move |(_, n)| n.start < before)
            .filter(|(id, _)| self.statuses[id.0] == NoteStatus::Pending)
            .map(|(id, _)| id)
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// Move the clock from `from` towards `to` according to the play mode.
    pub fn advance(&mut self, song: &Song, mode: PlayMode, from: Seconds, to: Seconds) -> Advance {
        match mode {
            PlayMode::Listen => {
                let auto_played: Vec<NoteId> = self.pending_starting_before(song, to).collect();
                auto_played
                    .iter()
                    .for_each(|id| self.statuses[id.0] = NoteStatus::Hit);
                Advance {
                    time: to,
                    auto_played,
                }
            }
            PlayMode::Wait => {
                let gate = self
                    .pending_starting_before(song, to)
                    .next()
                    .map(|id| song.note(id).start);
                let time = gate.map_or(to, |start| if start > from { start } else { from });
                Advance {
                    time,
                    auto_played: Vec::new(),
                }
            }
            PlayMode::Play => {
                let late = Seconds(to.0 - HIT_WINDOW.0);
                let missed: Vec<NoteId> = self.pending_starting_before(song, late).collect();
                missed
                    .iter()
                    .for_each(|id| self.statuses[id.0] = NoteStatus::Missed);
                Advance {
                    time: to,
                    auto_played: Vec::new(),
                }
            }
        }
    }

    /// Judge a key press at `time`. Returns the note it hit, if any.
    pub fn press(&mut self, song: &Song, pitch: MidiNote, time: Seconds) -> Option<NoteId> {
        let hit = song
            .iter()
            .filter(|(id, n)| {
                n.pitch == pitch
                    && self.statuses[id.0] == NoteStatus::Pending
                    && (n.start.0 - time.0).abs() <= HIT_WINDOW.0
            })
            .min_by(|(_, a), (_, b)| {
                (a.start.0 - time.0)
                    .abs()
                    .total_cmp(&(b.start.0 - time.0).abs())
            })
            .map(|(id, _)| id);
        match hit {
            Some(id) => self.statuses[id.0] = NoteStatus::Hit,
            None => self.wrong += 1,
        }
        hit
    }
}
