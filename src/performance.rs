//! Game rules: how song time advances in each play mode, and how key presses
//! are judged against the score. Pure logic, no Bevy.

mod score;

pub use score::{Grade, HIT_WINDOW, Heat, Score, Timing};

use crate::music::MidiNote;
use crate::song::{Hand, NoteId, Seconds, Song};

/// Whose notes to consider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Anyone,
    Player,
    Computer,
}

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

/// Which hands the player is practising; the rest is played automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Hands {
    #[default]
    Both,
    Right,
    Left,
}

impl Hands {
    pub const ALL: [Self; 3] = [Self::Both, Self::Right, Self::Left];

    pub fn includes(self, hand: Hand) -> bool {
        match (self, hand) {
            (Self::Both, _) | (Self::Right, Hand::Right) | (Self::Left, Hand::Left) => true,
            (Self::Right, Hand::Left) | (Self::Left, Hand::Right) => false,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Both => Self::Right,
            Self::Right => Self::Left,
            Self::Left => Self::Both,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Both => "both hands",
            Self::Right => "right hand",
            Self::Left => "left hand",
        }
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
    hands: Hands,
}

impl Performance {
    pub fn new(song: &Song) -> Self {
        Self {
            statuses: vec![NoteStatus::Pending; song.notes().len()],
            score: Score::default(),
            stall: Seconds(0.0),
            hands: Hands::Both,
        }
    }

    pub fn hands(&self) -> Hands {
        self.hands
    }

    /// Choose which hands the player is responsible for.
    pub fn set_hands(&mut self, hands: Hands) {
        self.hands = hands;
    }

    /// Whether the player (rather than the computer) plays this note.
    pub fn is_players(&self, song: &Song, id: NoteId) -> bool {
        self.hands.includes(song.note(id).hand)
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
        let hands = self.hands;
        *self = Self::new(song);
        self.hands = hands;
    }

    /// Pending notes starting before `before`, optionally only one owner's.
    fn pending_starting_before(&self, song: &Song, before: Seconds, owner: Owner) -> Vec<NoteId> {
        song.iter()
            .take_while(|(_, n)| n.start < before)
            .filter(|(id, _)| self.statuses[id.0] == NoteStatus::Pending)
            .filter(|(_, n)| match owner {
                Owner::Anyone => true,
                Owner::Player => self.hands.includes(n.hand),
                Owner::Computer => !self.hands.includes(n.hand),
            })
            .map(|(id, _)| id)
            .collect()
    }

    fn auto_play(&mut self, ids: &[NoteId]) {
        ids.iter()
            .for_each(|id| self.statuses[id.0] = NoteStatus::AutoPlayed);
    }

    /// Move the clock from `from` towards `to` according to the play mode.
    pub fn advance(&mut self, song: &Song, mode: PlayMode, from: Seconds, to: Seconds) -> Advance {
        match mode {
            PlayMode::Listen => {
                let auto_played = self.pending_starting_before(song, to, Owner::Anyone);
                self.auto_play(&auto_played);
                Advance {
                    time: to,
                    auto_played,
                    missed: Vec::new(),
                }
            }
            PlayMode::Wait => {
                let gate = self
                    .pending_starting_before(song, to, Owner::Player)
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
                // The computer's part sounds as soon as the clock reaches it,
                // even while it waits for the player at the same instant.
                let auto_played =
                    self.pending_starting_before(song, Seconds(time.0.next_up()), Owner::Computer);
                self.auto_play(&auto_played);
                Advance {
                    time,
                    auto_played,
                    missed: Vec::new(),
                }
            }
            PlayMode::Play => {
                let auto_played = self.pending_starting_before(song, to, Owner::Computer);
                self.auto_play(&auto_played);
                let missed = self.pending_starting_before(song, to - HIT_WINDOW, Owner::Player);
                missed.iter().for_each(|id| {
                    self.statuses[id.0] = NoteStatus::Missed;
                    self.score.record_miss();
                });
                Advance {
                    time: to,
                    auto_played,
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
                    && self.hands.includes(n.hand)
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
