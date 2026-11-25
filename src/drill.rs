//! The flash-card drill: chord-book challenges you've cleared come back as
//! cards, one at a time, each to be played before the clock runs out.
//!
//! Pure model: the app feeds it elapsed time and the keys held, and shows
//! what it says. Cards you miss come back a few cards later and are dealt
//! more often afterwards; cards you get right fade back to normal.

use std::collections::BTreeMap;

use tracing::{debug, info};

use crate::chord_keys::{BOOK, ChordKey, Inversion};
use crate::music::MidiNote;
use crate::song::Seconds;

/// Cards in a round.
pub const ROUND: usize = 10;
/// How long you have to play each card.
pub const TIME_LIMIT: Seconds = Seconds(5.0);
/// How long a right answer shows before the next card.
pub const PAUSE_RIGHT: Seconds = Seconds(0.7);
/// How long a miss shows (with the right keys lit) before the next card.
pub const PAUSE_MISS: Seconds = Seconds(1.8);
/// Cards dealt between missing a card and seeing it again.
pub const RETRY_AFTER: usize = 2;
/// How much more likely a card is per outstanding miss.
const MISS_WEIGHT: u64 = 3;

/// One cleared challenge: a chord in a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Card {
    page: usize,
    inversion: Inversion,
}

impl Card {
    /// The card for a chord-book page, if there is such a page.
    pub fn new(page: usize, inversion: Inversion) -> Option<Self> {
        (page < BOOK.len()).then_some(Self { page, inversion })
    }

    pub fn page(self) -> usize {
        self.page
    }

    pub fn chord(self) -> ChordKey {
        BOOK[self.page]
    }

    pub fn inversion(self) -> Inversion {
        self.inversion
    }

    /// The keys to play, around middle C.
    pub fn keys(self) -> Vec<MidiNote> {
        self.chord().keys(self.inversion)
    }

    /// "C major, first inversion".
    pub fn label(self) -> String {
        format!("{}, {}", self.chord().name(), self.inversion.label())
    }
}

/// What was played instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mistake {
    /// The right chord, in this other position.
    Inversion(Inversion),
    /// Another chord from the book.
    Chord(ChordKey),
    /// Notes that make no major or minor triad.
    NotAChord,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Answer {
    /// Played in this long.
    Right(Seconds),
    Wrong(Mistake),
    TimedOut,
}

impl Answer {
    pub fn is_right(self) -> bool {
        matches!(self, Self::Right(_))
    }
}

/// Where a round is up to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    /// Waiting for `card`, `elapsed` into its time.
    Asking { card: Card, elapsed: Seconds },
    /// `card` was just answered; the next comes after a pause.
    Marked { card: Card, answer: Answer },
    /// Every card has been dealt.
    Over,
}

/// How a round went.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Summary {
    pub right: usize,
    pub wrong: usize,
    pub timed_out: usize,
    /// Mean time of the right answers.
    pub average: Option<Seconds>,
    /// Cards missed at least once, in the order first missed.
    pub to_work_on: Vec<Card>,
}

#[derive(Debug, Clone)]
pub struct Round {
    stage: Stage,
    /// Answers so far, in order.
    results: Vec<(Card, Answer)>,
    /// Every key has been let go since the card was dealt, so what's held
    /// now was played for it.
    armed: bool,
    /// Time left of the pause after an answer.
    pause: Seconds,
    /// Missed cards to deal again, with the deal number they're due at.
    retries: Vec<(Card, usize)>,
}

impl Round {
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// Which card this is, from 1 to [`ROUND`].
    pub fn number(&self) -> usize {
        match self.stage {
            Stage::Asking { .. } => self.results.len() + 1,
            Stage::Marked { .. } | Stage::Over => self.results.len(),
        }
    }

    pub fn results(&self) -> &[(Card, Answer)] {
        &self.results
    }

    /// Time left to play the current card.
    pub fn left(&self) -> Seconds {
        match self.stage {
            Stage::Asking { elapsed, .. } => Seconds((TIME_LIMIT.0 - elapsed.0).max(0.0)),
            Stage::Marked { .. } | Stage::Over => Seconds(0.0),
        }
    }

    pub fn summary(&self) -> Summary {
        let count = |f: fn(&Answer) -> bool| self.results.iter().filter(|(_, a)| f(a)).count();
        let times: Vec<f32> = self
            .results
            .iter()
            .filter_map(|(_, a)| match a {
                Answer::Right(t) => Some(t.0),
                _ => None,
            })
            .collect();
        let to_work_on = self.results.iter().filter(|(_, a)| !a.is_right()).fold(
            Vec::new(),
            |mut cards, (card, _)| {
                if !cards.contains(card) {
                    cards.push(*card);
                }
                cards
            },
        );
        Summary {
            right: times.len(),
            wrong: count(|a| matches!(a, Answer::Wrong(_))),
            timed_out: count(|a| *a == Answer::TimedOut),
            average: (!times.is_empty())
                .then(|| Seconds(times.iter().sum::<f32>() / times.len() as f32)),
            to_work_on,
        }
    }
}

/// Nothing in the chord book is cleared yet, so there are no cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("no chord-book challenges cleared yet")]
pub struct NothingCleared;

/// A small deterministic random source (splitmix64), so drills can be
/// replayed from a seed.
#[derive(Debug, Clone)]
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

/// The deck, what you tend to miss, and the round in play.
#[derive(Debug, Clone)]
pub struct Drill {
    deck: Vec<Card>,
    /// Outstanding misses per card: up one for each miss, down one for each
    /// right answer.
    misses: BTreeMap<Card, u32>,
    rng: Rng,
    round: Option<Round>,
}

impl Drill {
    pub fn new(seed: u64) -> Self {
        Self {
            deck: Vec::new(),
            misses: BTreeMap::new(),
            rng: Rng(seed),
            round: None,
        }
    }

    /// Deal a new round from these cleared challenges (page, position).
    pub fn start(
        &mut self,
        cleared: impl IntoIterator<Item = (usize, Inversion)>,
    ) -> Result<(), NothingCleared> {
        self.deck = cleared
            .into_iter()
            .filter_map(|(page, inversion)| Card::new(page, inversion))
            .collect();
        self.deck.sort();
        self.deck.dedup();
        if self.deck.is_empty() {
            self.round = None;
            return Err(NothingCleared);
        }
        let mut round = Round {
            stage: Stage::Over,
            results: Vec::new(),
            armed: false,
            pause: Seconds(0.0),
            retries: Vec::new(),
        };
        let first = self.pick(&mut round, None);
        round.stage = Stage::Asking {
            card: first,
            elapsed: Seconds(0.0),
        };
        info!(cards = self.deck.len(), "flash-card round started");
        self.round = Some(round);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.round = None;
    }

    pub fn round(&self) -> Option<&Round> {
        self.round.as_ref()
    }

    pub fn deck(&self) -> &[Card] {
        &self.deck
    }

    /// Let `dt` pass: the clock runs down, and after a pause the next card
    /// is dealt. Returns the answer if time just ran out.
    pub fn tick(&mut self, dt: Seconds) -> Option<Answer> {
        let round = self.round.as_mut()?;
        match round.stage {
            Stage::Asking { card, elapsed } => {
                let elapsed = elapsed + dt;
                if elapsed.0 >= TIME_LIMIT.0 {
                    return Some(self.mark(card, Answer::TimedOut));
                }
                round.stage = Stage::Asking { card, elapsed };
            }
            Stage::Marked { card, .. } => {
                round.pause = round.pause - dt;
                if round.pause.0 <= 0.0 {
                    self.deal(card);
                }
            }
            Stage::Over => {}
        }
        None
    }

    /// The keys held now. Once every key has been let go since the card
    /// was dealt, three or more different notes answer it.
    pub fn play(&mut self, held: &[MidiNote]) -> Option<Answer> {
        let round = self.round.as_mut()?;
        let Stage::Asking { card, elapsed } = round.stage else {
            return None;
        };
        if !round.armed {
            round.armed = held.is_empty();
            return None;
        }
        let mut classes: Vec<_> = held.iter().map(|n| n.pitch_class().semitone()).collect();
        classes.sort_unstable();
        classes.dedup();
        if classes.len() < 3 {
            return None;
        }
        let chord = card.chord();
        let answer = match chord.inversion(held) {
            Some(inversion) if inversion == card.inversion => Answer::Right(elapsed),
            Some(inversion) => Answer::Wrong(Mistake::Inversion(inversion)),
            None => {
                Answer::Wrong(ChordKey::in_book(held).map_or(Mistake::NotAChord, Mistake::Chord))
            }
        };
        Some(self.mark(card, answer))
    }

    fn mark(&mut self, card: Card, answer: Answer) -> Answer {
        let misses = self.misses.entry(card).or_default();
        *misses = if answer.is_right() {
            misses.saturating_sub(1)
        } else {
            *misses + 1
        };
        if let Some(round) = self.round.as_mut() {
            round.results.push((card, answer));
            if !answer.is_right() {
                round
                    .retries
                    .push((card, round.results.len() + RETRY_AFTER));
            }
            round.pause = if answer.is_right() {
                PAUSE_RIGHT
            } else {
                PAUSE_MISS
            };
            round.stage = Stage::Marked { card, answer };
        }
        debug!(card = card.label(), ?answer, "flash card answered");
        answer
    }

    /// The next card, or the end of the round.
    fn deal(&mut self, previous: Card) {
        let Some(mut round) = self.round.take() else {
            return;
        };
        round.stage = if round.results.len() >= ROUND {
            Stage::Over
        } else {
            Stage::Asking {
                card: self.pick(&mut round, Some(previous)),
                elapsed: Seconds(0.0),
            }
        };
        round.armed = false;
        self.round = Some(round);
    }

    /// A missed card that's due, or else a random card (not the last one
    /// again), weighted towards the ones you miss.
    fn pick(&mut self, round: &mut Round, previous: Option<Card>) -> Card {
        let dealt = round.results.len();
        let due = round
            .retries
            .iter()
            .position(|(card, at)| *at <= dealt && Some(*card) != previous);
        let card = match due {
            Some(index) => round.retries[index].0,
            None => {
                let candidates: Vec<(Card, u64)> = self
                    .deck
                    .iter()
                    .filter(|card| self.deck.len() == 1 || Some(**card) != previous)
                    .map(|card| {
                        let misses = u64::from(self.misses.get(card).copied().unwrap_or(0));
                        (*card, 1 + MISS_WEIGHT * misses)
                    })
                    .collect();
                let total = candidates.iter().map(|(_, w)| w).sum();
                let mut roll = self.rng.below(total);
                candidates
                    .iter()
                    .find(|(_, weight)| {
                        let here = roll < *weight;
                        roll = roll.saturating_sub(*weight);
                        here
                    })
                    .map_or(self.deck[0], |(card, _)| *card)
            }
        };
        round.retries.retain(|(retry, _)| *retry != card);
        card
    }
}
