//! Automatic piano fingering.
//!
//! Each hand's notes are grouped into chords (notes starting together), and a
//! dynamic programme picks the finger assignment with the lowest total cost.
//! The costs are a simplified piano-ergonomics model:
//!
//! * each finger naturally covers one white key, so stretching or squeezing
//!   between consecutive notes costs the mismatch;
//! * passing the thumb under (going up) or a finger over the thumb (going
//!   down) is allowed at a moderate cost, cheapest with the middle finger;
//!   any other crossing is heavily penalised;
//! * moving to a new note with the same finger costs a lot, repeating a note
//!   with the same finger costs nothing;
//! * thumbs (and to a lesser degree pinkies) prefer white keys.
//!
//! The left hand is handled by mirroring pitch, so finger 1 is always the
//! finger nearest the middle of the keyboard.

use crate::music::{KeyColor, MidiNote};
use crate::song::{Hand, Note};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Finger {
    Thumb,
    Index,
    Middle,
    Ring,
    Pinky,
}

impl Finger {
    pub const ALL: [Self; 5] = [
        Self::Thumb,
        Self::Index,
        Self::Middle,
        Self::Ring,
        Self::Pinky,
    ];

    pub fn from_number(number: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.number() == number)
    }

    /// Conventional piano finger number, 1 (thumb) to 5 (pinky).
    pub fn number(self) -> u8 {
        match self {
            Self::Thumb => 1,
            Self::Index => 2,
            Self::Middle => 3,
            Self::Ring => 4,
            Self::Pinky => 5,
        }
    }

    fn index(self) -> i32 {
        i32::from(self.number())
    }
}

/// A note as the fingering model sees it.
#[derive(Debug, Clone, Copy)]
struct Key {
    /// Diatonic position, mirrored for the left hand so it always increases
    /// away from the thumb.
    x: i32,
    pitch: MidiNote,
}

impl Key {
    fn new(note: &Note) -> Self {
        let step = note.pitch.diatonic_step();
        Self {
            x: match note.hand {
                Hand::Right => step,
                Hand::Left => -step,
            },
            pitch: note.pitch,
        }
    }
}

const SAME_FINGER_MOVE: f32 = 8.0;
/// Changing finger on a repeated note is a deliberate technique, not a free
/// way to reposition: it costs more than a one-key stretch.
const FINGER_SWAP_ON_REPEAT: f32 = 2.0;
const FORBIDDEN_CROSSING: f32 = 20.0;
const THUMB_ON_BLACK: f32 = 1.0;
const PINKY_ON_BLACK: f32 = 0.5;

/// Cost of passing the thumb under (or a finger over it), by the finger
/// involved: the middle finger is the classic pivot.
fn crossing_cost(finger: Finger) -> f32 {
    match finger {
        Finger::Thumb => FORBIDDEN_CROSSING,
        Finger::Index => 2.5,
        Finger::Middle => 2.0,
        Finger::Ring => 2.8,
        Finger::Pinky => 8.0,
    }
}

fn key_cost(key: Key, finger: Finger) -> f32 {
    match (key.pitch.key_color(), finger) {
        (KeyColor::Black, Finger::Thumb) => THUMB_ON_BLACK,
        (KeyColor::Black, Finger::Pinky) => PINKY_ON_BLACK,
        _ => 0.0,
    }
}

/// Cost of playing `to` with `g` right after `from` with `f`.
fn move_cost(from: Key, f: Finger, to: Key, g: Finger) -> f32 {
    if from.pitch == to.pitch {
        return if f == g { 0.0 } else { FINGER_SWAP_ON_REPEAT };
    }
    let dx = to.x - from.x;
    let df = g.index() - f.index();
    let stretch = |want: i32| (dx.abs() - want).abs() as f32;
    match (dx.signum(), df.signum()) {
        (_, 0) => SAME_FINGER_MOVE + dx.abs() as f32,
        // A sharp and its natural share a diatonic step.
        (0, _) => df.abs() as f32 * 0.5,
        (1, 1) | (-1, -1) => stretch(df.abs()),
        // Going up with a lower finger: only the thumb can pass under.
        (1, -1) if g == Finger::Thumb => crossing_cost(f) + stretch(1),
        // Going down with a higher finger: only over the thumb.
        (-1, 1) if f == Finger::Thumb => crossing_cost(g) + stretch(1),
        _ => FORBIDDEN_CROSSING,
    }
}

/// Every way to give `k` notes (sorted by `x`) strictly increasing fingers.
fn assignments(k: usize) -> Vec<Vec<Finger>> {
    fn extend(from: usize, left: usize, current: &mut Vec<Finger>, out: &mut Vec<Vec<Finger>>) {
        if left == 0 {
            out.push(current.clone());
            return;
        }
        (from..Finger::ALL.len()).for_each(|i| {
            current.push(Finger::ALL[i]);
            extend(i + 1, left - 1, current, out);
            current.pop();
        });
    }
    let mut out = Vec::new();
    extend(0, k.min(Finger::ALL.len()), &mut Vec::new(), &mut out);
    out
}

fn chord_cost(keys: &[Key], fingers: &[Finger]) -> f32 {
    let inner: f32 = keys
        .windows(2)
        .zip(fingers.windows(2))
        .map(|(k, f)| {
            let dx = k[1].x - k[0].x;
            let df = f[1].index() - f[0].index();
            (dx - df).abs() as f32
        })
        .sum();
    inner
        + keys
            .iter()
            .zip(fingers)
            .map(|(k, f)| key_cost(*k, *f))
            .sum::<f32>()
}

fn transition_cost(prev: &[Key], prev_f: &[Finger], next: &[Key], next_f: &[Finger]) -> f32 {
    next.iter()
        .zip(next_f)
        .map(|(to, g)| {
            // Measure each new note from the nearest note just played.
            prev.iter()
                .zip(prev_f)
                .min_by_key(|(from, _)| (from.x - to.x).abs())
                .map_or(0.0, |(from, f)| move_cost(*from, *f, *to, *g))
        })
        .sum()
}

/// One chord (or single note): note indices sorted by mirrored position.
struct Group {
    notes: Vec<usize>,
    keys: Vec<Key>,
    options: Vec<Vec<Finger>>,
}

fn groups_for(notes: &[Note], hand: Hand) -> Vec<Group> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    notes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.hand == hand)
        .for_each(|(i, n)| match groups.last_mut() {
            Some(g) if (notes[g[0]].start.0 - n.start.0).abs() < 1e-4 => g.push(i),
            _ => groups.push(vec![i]),
        });
    groups
        .into_iter()
        .map(|mut indices| {
            indices.sort_by_key(|i| Key::new(&notes[*i]).x);
            // Chords wider than a hand: only the outer five are fingered;
            // the rest reuse the nearest finger.
            indices.truncate(Finger::ALL.len());
            let keys: Vec<Key> = indices.iter().map(|i| Key::new(&notes[*i])).collect();
            // Fingers written into the score rule out every other option.
            let pinned: Vec<Option<Finger>> = indices.iter().map(|i| notes[*i].finger).collect();
            let all = assignments(keys.len());
            let respecting: Vec<Vec<Finger>> = all
                .iter()
                .filter(|option| {
                    option
                        .iter()
                        .zip(&pinned)
                        .all(|(f, pin)| pin.is_none_or(|p| p == *f))
                })
                .cloned()
                .collect();
            Group {
                // Contradictory pins (e.g. out of order) fall back to free choice.
                options: if respecting.is_empty() {
                    all
                } else {
                    respecting
                },
                notes: indices,
                keys,
            }
        })
        .collect()
}

/// Minimum-cost fingering for one hand, by dynamic programming over groups.
fn solve(groups: &[Group]) -> Vec<Vec<Finger>> {
    let Some(first) = groups.first() else {
        return Vec::new();
    };
    // best[g][o] = (cost to reach option o of group g, back-pointer)
    let mut best: Vec<Vec<(f32, usize)>> = vec![
        first
            .options
            .iter()
            .map(|o| (chord_cost(&first.keys, o), 0))
            .collect(),
    ];
    groups.windows(2).for_each(|pair| {
        let (prev, next) = (&pair[0], &pair[1]);
        let previous = best.last().cloned().unwrap_or_default();
        let row = next
            .options
            .iter()
            .map(|o| {
                let own = chord_cost(&next.keys, o);
                previous
                    .iter()
                    .enumerate()
                    .map(|(p, (cost, _))| {
                        (
                            cost + own
                                + transition_cost(&prev.keys, &prev.options[p], &next.keys, o),
                            p,
                        )
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .unwrap_or((f32::INFINITY, 0))
            })
            .collect();
        best.push(row);
    });
    // Walk back from the cheapest final option.
    let mut choice = best
        .last()
        .and_then(|row| {
            row.iter()
                .enumerate()
                .min_by(|a, b| a.1.0.total_cmp(&b.1.0))
                .map(|(i, _)| i)
        })
        .unwrap_or(0);
    let mut picks = vec![0; groups.len()];
    (0..groups.len()).rev().for_each(|g| {
        picks[g] = choice;
        choice = best[g][choice].1;
    });
    picks
        .iter()
        .zip(groups)
        .map(|(pick, group)| group.options[*pick].clone())
        .collect()
}

/// A finger for every note, in the same order as `notes`.
pub fn assign(notes: &[Note]) -> Vec<Finger> {
    let mut fingers = vec![Finger::Middle; notes.len()];
    [Hand::Right, Hand::Left].into_iter().for_each(|hand| {
        let groups = groups_for(notes, hand);
        solve(&groups)
            .into_iter()
            .zip(&groups)
            .for_each(|(chosen, group)| {
                group
                    .notes
                    .iter()
                    .zip(chosen)
                    .for_each(|(i, f)| fingers[*i] = f);
            });
    });
    fingers
}
