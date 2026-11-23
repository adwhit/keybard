//! Game rules: how song time advances in each play mode, and how key presses
//! are judged against the score. Pure logic, no Bevy.

mod score;

pub use score::{Grade, HIT_WINDOW, Score, Timing};

use crate::music::MidiNote;
use crate::song::{NoteId, Seconds, Song};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
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
    pub const ALL: [Self; 3] = [Self::Listen, Self::Wait, Self::Play];

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

    /// Whether the player's presses are judged and scored.
    pub fn is_scored(self) -> bool {
        self != Self::Listen
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteStatus {
    Pending,
    Hit(Timing),
    /// Played by the computer in Listen mode; not scored.
    AutoPlayed,
    Missed,
    /// Jumped over by seeking; not scored.
    Skipped,
}

/// A successful press.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Judgement {
    pub note: NoteId,
    pub timing: Timing,
    /// How far from the note's start the press was (negative = early).
    pub offset: Seconds,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PressOutcome {
    Hit(Judgement),
    /// Matched no pending note.
    Wrong,
}

/// Result of moving the clock forward.
#[derive(Debug, Clone, PartialEq)]
pub struct Advance {
    pub time: Seconds,
    /// Notes the computer played for you (Listen mode), to be sounded.
    pub auto_played: Vec<NoteId>,
    /// Notes that slipped past unplayed (Play mode).
    pub missed: Vec<NoteId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Performance {
    statuses: Vec<NoteStatus>,
    score: Score,
    /// In Wait mode, how long the clock has been held at the current note.
    stall: Seconds,
}

impl Performance {
    pub fn new(song: &Song) -> Self {
        Self {
            statuses: vec![NoteStatus::Pending; song.notes().len()],
            score: Score::default(),
            stall: Seconds(0.0),
        }
    }

    pub fn status(&self, id: NoteId) -> NoteStatus {
        self.statuses[id.0]
    }

    pub fn score(&self) -> &Score {
        &self.score
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
        self.stall = Seconds(0.0);
    }

    pub fn reset(&mut self, song: &Song) {
        *self = Self::new(song);
    }

    fn pending_starting_before(&self, song: &Song, before: Seconds) -> Vec<NoteId> {
        song.iter()
            .take_while(|(_, n)| n.start < before)
            .filter(|(id, _)| self.statuses[id.0] == NoteStatus::Pending)
            .map(|(id, _)| id)
            .collect()
    }

    /// Move the clock from `from` towards `to` according to the play mode.
    pub fn advance(&mut self, song: &Song, mode: PlayMode, from: Seconds, to: Seconds) -> Advance {
        match mode {
            PlayMode::Listen => {
                let auto_played = self.pending_starting_before(song, to);
                auto_played
                    .iter()
                    .for_each(|id| self.statuses[id.0] = NoteStatus::AutoPlayed);
                Advance {
                    time: to,
                    auto_played,
                    missed: Vec::new(),
                }
            }
            PlayMode::Wait => {
                let gate = self
                    .pending_starting_before(song, to)
                    .first()
                    .map(|id| song.note(*id).start);
                let time = match gate {
                    Some(start) if start > from => {
                        // Arriving at a new note: any overshoot is already waiting.
                        self.stall = to - start;
                        start
                    }
                    Some(_) => {
                        self.stall = self.stall + (to - from);
                        from
                    }
                    None => {
                        self.stall = Seconds(0.0);
                        to
                    }
                };
                Advance {
                    time,
                    auto_played: Vec::new(),
                    missed: Vec::new(),
                }
            }
            PlayMode::Play => {
                let missed = self.pending_starting_before(song, to - HIT_WINDOW);
                missed.iter().for_each(|id| {
                    self.statuses[id.0] = NoteStatus::Missed;
                    self.score.record_miss();
                });
                Advance {
                    time: to,
                    auto_played: Vec::new(),
                    missed,
                }
            }
        }
    }

    /// Judge a key press at `time`.
    pub fn press(&mut self, song: &Song, pitch: MidiNote, time: Seconds) -> PressOutcome {
        let candidate = song
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
            });
        match candidate {
            Some((id, note)) => {
                // A note the clock is waiting on counts the wait as lateness.
                let waited = if time >= note.start {
                    self.stall
                } else {
                    Seconds(0.0)
                };
                let offset = (time - note.start) + waited;
                let timing = Timing::from_offset(offset).unwrap_or(Timing::Good);
                self.statuses[id.0] = NoteStatus::Hit(timing);
                self.score.record_hit(timing);
                PressOutcome::Hit(Judgement {
                    note: id,
                    timing,
                    offset,
                })
            }
            None => {
                self.score.record_wrong();
                PressOutcome::Wrong
            }
        }
    }
}
