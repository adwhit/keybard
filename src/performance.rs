//! Game rules: how song time advances in each play mode, and how key presses
//! are judged against the score. Pure logic, no Bevy.

mod insights;
mod score;

pub use insights::{Expression, Insights, LOUDNESS_LEAN, Loudness, MOST_MISSED, Tendency};
pub use score::{Grade, HIT_WINDOW, Heat, Score, Timing};

use crate::dynamics::{Dynamic, Velocity};
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
    /// Timing of each hit note (negative = early), for insights.
    offsets: Vec<Seconds>,
    /// How hard each hit note was played, when the keyboard can tell.
    touches: Vec<Option<Velocity>>,
}

impl Performance {
    pub fn new(song: &Song) -> Self {
        Self {
            statuses: vec![NoteStatus::Pending; song.notes().len()],
            score: Score::default(),
            stall: Seconds(0.0),
            hands: Hands::Both,
            offsets: vec![Seconds(0.0); song.notes().len()],
            touches: vec![None; song.notes().len()],
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
        song.iter()
            .filter(|(_, note)| note.start >= time)
            .for_each(|(id, _)| self.touches[id.0] = None);
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

    /// Accuracy of the player's judged notes in each bar (`None` where
    /// nothing has been judged yet), for spotting weak sections.
    pub fn bar_accuracy(&self, song: &Song) -> Vec<Option<f32>> {
        let bar = song.tempo.bar_length().0;
        let bars = (song.duration().0 / bar).ceil().max(1.0) as usize;
        let mut sums = vec![(0.0_f32, 0_u32); bars];
        song.iter()
            .filter(|(_, n)| self.hands.includes(n.hand))
            .filter_map(|(id, n)| {
                let earned = match self.statuses[id.0] {
                    NoteStatus::Hit(timing) => timing.weight(),
                    NoteStatus::Missed => 0.0,
                    NoteStatus::Pending | NoteStatus::AutoPlayed | NoteStatus::Skipped => {
                        return None;
                    }
                };
                let index = ((n.start.0 / bar) as usize).min(bars - 1);
                Some((index, earned))
            })
            .for_each(|(index, earned)| {
                sums[index].0 += earned;
                sums[index].1 += 1;
            });
        sums.into_iter()
            .map(|(earned, count)| (count > 0).then(|| earned / count as f32))
            .collect()
    }

    /// Whether every one of the player's notes in a (0-based) bar was hit
    /// Perfect. Bars with none of the player's notes never count.
    pub fn bar_is_perfect(&self, song: &Song, bar: usize) -> bool {
        let length = song.tempo.bar_length().0;
        let mut notes = song
            .iter()
            .filter(|(_, n)| self.hands.includes(n.hand))
            .filter(|(_, n)| ((n.start.0 / length + 1e-4) as usize) == bar)
            .peekable();
        notes.peek().is_some()
            && notes.all(|(id, _)| self.statuses[id.0] == NoteStatus::Hit(Timing::Perfect))
    }

    /// What to work on, from the player's judged notes.
    pub fn insights(&self, song: &Song) -> Insights {
        let judged = |hand: Hand| {
            let weights: Vec<f32> = song
                .iter()
                .filter(|(_, n)| n.hand == hand && self.hands.includes(hand))
                .filter_map(|(id, _)| match self.statuses[id.0] {
                    NoteStatus::Hit(t) => Some(t.weight()),
                    NoteStatus::Missed => Some(0.0),
                    NoteStatus::Pending | NoteStatus::AutoPlayed | NoteStatus::Skipped => None,
                })
                .collect();
            (!weights.is_empty()).then(|| weights.iter().sum::<f32>() / weights.len() as f32)
        };
        let hit_offsets: Vec<f32> = song
            .iter()
            .filter(|(id, _)| matches!(self.statuses[id.0], NoteStatus::Hit(_)))
            .map(|(id, _)| self.offsets[id.0].0)
            .collect();
        let mean_offset = (!hit_offsets.is_empty())
            .then(|| Seconds(hit_offsets.iter().sum::<f32>() / hit_offsets.len() as f32));
        let mut misses: Vec<(MidiNote, usize)> = Vec::new();
        song.iter()
            .filter(|(id, _)| self.statuses[id.0] == NoteStatus::Missed)
            .for_each(
                |(_, n)| match misses.iter_mut().find(|(p, _)| *p == n.pitch) {
                    Some((_, count)) => *count += 1,
                    None => misses.push((n.pitch, 1)),
                },
            );
        misses.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        misses.truncate(MOST_MISSED);
        Insights {
            right_accuracy: judged(Hand::Right),
            left_accuracy: judged(Hand::Left),
            mean_offset,
            most_missed: misses,
            dynamics: self.expression(song),
        }
    }

    /// Record how hard a hit note was played (touch-sensitive input only).
    pub fn touch(&mut self, id: NoteId, velocity: Velocity) {
        self.touches[id.0] = Some(velocity);
    }

    /// How the touch of hit notes compares with their marked dynamics.
    fn expression(&self, song: &Song) -> Option<Expression> {
        let level = |v: Velocity| {
            Dynamic::ALL
                .iter()
                .position(|d| *d == Dynamic::from_velocity(v))
                .unwrap_or(0) as i32
        };
        let steps: Vec<i32> = song
            .iter()
            .filter(|(id, _)| matches!(self.statuses[id.0], NoteStatus::Hit(_)))
            .filter_map(|(id, n)| self.touches[id.0].map(|v| level(v) - level(n.velocity)))
            .collect();
        (!steps.is_empty()).then(|| {
            let count = steps.len() as f32;
            let as_marked = steps.iter().filter(|s| s.abs() <= 1).count() as f32 / count;
            let mean = steps.iter().sum::<i32>() as f32 / count;
            Expression {
                as_marked,
                lean: match mean {
                    m if m > LOUDNESS_LEAN => Loudness::Louder,
                    m if m < -LOUDNESS_LEAN => Loudness::Softer,
                    _ => Loudness::AsMarked,
                },
            }
        })
    }

    /// The player's pending note due near `time` whose pitch is closest to
    /// `pitch`, to suggest after a wrong press.
    pub fn expected_near(&self, song: &Song, pitch: MidiNote, time: Seconds) -> Option<NoteId> {
        song.iter()
            .filter(|(id, n)| {
                self.statuses[id.0] == NoteStatus::Pending
                    && self.hands.includes(n.hand)
                    && (n.start.0 - time.0).abs() <= HIT_WINDOW.0
            })
            .min_by_key(|(_, n)| (i16::from(n.pitch.value()) - i16::from(pitch.value())).abs())
            .map(|(id, _)| id)
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
                self.offsets[id.0] = offset;
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
