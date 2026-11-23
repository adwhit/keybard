//! Chord naming for lead-sheet style symbols above the staff.

use crate::key::KeySignature;
use crate::music::{Accidental, MidiNote};
use crate::song::{Hand, Seconds, Song};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quality {
    Major,
    Minor,
    Diminished,
    Augmented,
    Sus4,
    Dominant7,
    Major7,
    Minor7,
    Power,
}

impl Quality {
    /// In order of preference when scores tie.
    const ALL: [Self; 9] = [
        Self::Major,
        Self::Minor,
        Self::Dominant7,
        Self::Minor7,
        Self::Major7,
        Self::Sus4,
        Self::Diminished,
        Self::Augmented,
        Self::Power,
    ];

    fn intervals(self) -> &'static [u8] {
        match self {
            Self::Major => &[0, 4, 7],
            Self::Minor => &[0, 3, 7],
            Self::Diminished => &[0, 3, 6],
            Self::Augmented => &[0, 4, 8],
            Self::Sus4 => &[0, 5, 7],
            Self::Dominant7 => &[0, 4, 7, 10],
            Self::Major7 => &[0, 4, 7, 11],
            Self::Minor7 => &[0, 3, 7, 10],
            Self::Power => &[0, 7],
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::Major => "",
            Self::Minor => "m",
            Self::Diminished => "dim",
            Self::Augmented => "aug",
            Self::Sus4 => "sus4",
            Self::Dominant7 => "7",
            Self::Major7 => "maj7",
            Self::Minor7 => "m7",
            Self::Power => "5",
        }
    }

    /// Penalty for chords that are rarer than passing-tone lookalikes.
    fn rarity(self) -> f32 {
        match self {
            Self::Sus4 => 2.0,
            Self::Diminished | Self::Augmented => 0.5,
            _ => 0.0,
        }
    }

    /// Third above the root, for triads whose third may be inferred.
    fn third(self) -> Option<u8> {
        match self {
            Self::Major => Some(4),
            Self::Minor => Some(3),
            _ => None,
        }
    }
}

fn root_name(root: u8, key: KeySignature) -> String {
    // Spell via a concrete pitch; the octave doesn't matter for the name.
    let spelled = MidiNote::new(60 + root).map(|n| key.spell(n));
    spelled.map_or_else(
        |_| String::from("?"),
        |s| {
            let accidental = match s.accidental {
                Accidental::Natural => "",
                Accidental::Sharp => "#",
                Accidental::Flat => "b",
            };
            format!("{:?}{accidental}", s.letter)
        },
    )
}

/// Cost of filling in a missing third from the key.
const INFERRED_THIRD: f32 = 0.3;

/// Weight (sounding time) of each pitch class.
type Weights = [f32; 12];

fn has(weights: &Weights, pc: u8) -> bool {
    weights[usize::from(pc % 12)] > 1e-4
}

/// The exact chord formed by these notes, if they spell one completely
/// with nothing extra.
pub fn name(notes: &[MidiNote], key: KeySignature) -> Option<String> {
    let mut weights: Weights = [0.0; 12];
    notes
        .iter()
        .for_each(|n| weights[usize::from(n.pitch_class().semitone())] = 1.0);
    let present = weights.iter().filter(|w| **w > 0.0).count();
    (0..12u8).find_map(|root| {
        Quality::ALL.into_iter().find_map(|q| {
            let tones: Vec<u8> = q.intervals().iter().map(|i| (root + i) % 12).collect();
            let exact = tones.len() == present && tones.iter().all(|pc| has(&weights, *pc));
            exact.then(|| format!("{}{}", root_name(root, key), q.suffix()))
        })
    })
}

/// Best-fitting chord for duration-weighted pitch classes: chord tones
/// count for it, other tones against it, and a missing third may be filled
/// in from the key (so a bare G-D in C major reads as G).
fn fit(weights: &Weights, key: KeySignature) -> Option<String> {
    let distinct = weights.iter().filter(|w| **w > 1e-4).count();
    if distinct < 2 {
        return None;
    }
    let in_key = |pc: u8| key.contains_pitch_class(pc % 12);
    let total: f32 = weights.iter().sum();
    (0..12u8)
        .filter(|root| has(weights, *root))
        .flat_map(|root| {
            Quality::ALL
                .into_iter()
                .filter(|q| *q != Quality::Power)
                .filter_map(move |q| {
                    let tones: Vec<u8> = q.intervals().iter().map(|i| (root + i) % 12).collect();
                    let missing: Vec<u8> = tones
                        .iter()
                        .copied()
                        .filter(|pc| !has(weights, *pc))
                        .collect();
                    let allowed = match (missing.as_slice(), q.third()) {
                        ([], _) => true,
                        // Only a triad's third may be missing, and only when
                        // the key supplies that third.
                        ([pc], Some(third)) => *pc == (root + third) % 12 && in_key(*pc),
                        _ => false,
                    };
                    allowed.then(|| {
                        let inside: f32 = tones.iter().map(|pc| weights[usize::from(*pc)]).sum();
                        let outside = total - inside;
                        let extra = tones.len().saturating_sub(3) as f32;
                        let score = inside
                            - 0.6 * outside
                            - INFERRED_THIRD * missing.len() as f32
                            - 0.3 * extra
                            - q.rarity();
                        (score, root, q)
                    })
                })
        })
        .fold(
            None,
            |best: Option<(f32, u8, Quality)>, candidate| match best {
                Some(b) if b.0 >= candidate.0 => Some(b),
                _ => Some(candidate),
            },
        )
        .filter(|(score, ..)| *score > 0.0)
        .map(|(_, root, q)| format!("{}{}", root_name(root, key), q.suffix()))
}

/// Chord symbols where the harmony changes. Windows run between bar lines
/// and the left hand's on-beat chord changes (a single-note bass line is
/// harmonised per bar), so passing notes don't flip chords.
pub fn progression(song: &Song) -> Vec<(Seconds, String)> {
    let tempo = song.tempo;
    let beat = tempo.seconds_per_beat.0;
    let end = song.duration().0;
    let bar = tempo.bar_length().0;
    let on_beat = |t: f32| ((t / beat) - (t / beat).round()).abs() < 1e-3;
    let mut bounds: Vec<f32> = (0..)
        .map(|i| i as f32 * bar)
        .take_while(|t| *t < end)
        .chain(
            song.notes()
                .iter()
                .filter(|n| n.hand == Hand::Left && on_beat(n.start.0))
                .filter(|n| {
                    // Only chords in the left hand mark a change.
                    song.notes()
                        .iter()
                        .filter(|m| m.hand == Hand::Left && (m.start.0 - n.start.0).abs() < 1e-4)
                        .count()
                        >= 2
                })
                .map(|n| n.start.0),
        )
        .collect();
    bounds.push(end);
    bounds.sort_by(f32::total_cmp);
    bounds.dedup_by(|a, b| (*a - *b).abs() < 1e-3);

    let key = song.key();
    let named = bounds.windows(2).filter_map(|w| {
        let (from, to) = (w[0], w[1]);
        let mut weights: Weights = [0.0; 12];
        song.notes().iter().for_each(|n| {
            let overlap = n.end().0.min(to) - n.start.0.max(from);
            if overlap > 0.0 {
                weights[usize::from(n.pitch.pitch_class().semitone())] += overlap;
            }
        });
        fit(&weights, key).map(|symbol| (Seconds(from), symbol))
    });
    named.fold(
        Vec::new(),
        |mut marks: Vec<(Seconds, String)>, (at, symbol)| {
            if marks.last().is_none_or(|(_, last)| *last != symbol) {
                marks.push((at, symbol));
            }
            marks
        },
    )
}
