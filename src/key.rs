//! Key signatures: inferring one from a song, spelling pitches to suit it
//! and deciding which notes need accidentals.

use crate::music::{Accidental, Letter, MidiNote, Spelled};

/// Sharps (positive) or flats (negative) in the signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeySignature(i8);

/// Order sharps are added in; flats are added in the reverse order.
const SHARP_ORDER: [Letter; 7] = [
    Letter::F,
    Letter::C,
    Letter::G,
    Letter::D,
    Letter::A,
    Letter::E,
    Letter::B,
];
/// Keys considered when inferring (beyond this, spelling needs E#/Cb).
const MAX_ACCIDENTALS: i8 = 5;

fn natural_semitone(letter: Letter) -> u8 {
    letter.natural().semitone()
}

impl KeySignature {
    pub fn new(accidentals: i8) -> Self {
        Self(accidentals.clamp(-7, 7))
    }

    pub fn accidentals(self) -> i8 {
        self.0
    }

    /// Letters the signature alters, in the order they're written.
    pub fn altered_letters(self) -> Vec<Letter> {
        let count = usize::from(self.0.unsigned_abs());
        match self.0 {
            n if n > 0 => SHARP_ORDER[..count].to_vec(),
            n if n < 0 => SHARP_ORDER.iter().rev().take(count).copied().collect(),
            _ => Vec::new(),
        }
    }

    /// The accidental the signature applies to a letter.
    pub fn alteration(self, letter: Letter) -> Accidental {
        match (self.altered_letters().contains(&letter), self.0 > 0) {
            (false, _) => Accidental::Natural,
            (true, true) => Accidental::Sharp,
            (true, false) => Accidental::Flat,
        }
    }

    fn contains(self, pitch_class: u8) -> bool {
        Letter::ALL.iter().any(|letter| {
            let base = natural_semitone(*letter);
            let pc = match self.alteration(*letter) {
                Accidental::Natural => base,
                Accidental::Sharp => (base + 1) % 12,
                Accidental::Flat => (base + 11) % 12,
            };
            pc == pitch_class
        })
    }

    /// Spell a pitch: white keys as naturals, black keys as flats in flat
    /// keys and sharps otherwise.
    pub fn spell(self, note: MidiNote) -> Spelled {
        let pc = note.pitch_class();
        let octave = note.octave();
        match pc.accidental() {
            Accidental::Natural | Accidental::Flat => Spelled {
                letter: pc.letter(),
                accidental: Accidental::Natural,
                octave,
            },
            Accidental::Sharp if self.0 < 0 => Spelled {
                // The letter above; black keys never sit just below a C.
                letter: note
                    .offset(1)
                    .map_or(pc.letter(), |above| above.pitch_class().letter()),
                accidental: Accidental::Flat,
                octave,
            },
            Accidental::Sharp => Spelled {
                letter: pc.letter(),
                accidental: Accidental::Sharp,
                octave,
            },
        }
    }

    /// The accidental to print before a note, if the signature doesn't
    /// already imply it.
    pub fn accidental_to_show(self, spelled: Spelled) -> Option<Accidental> {
        (self.alteration(spelled.letter) != spelled.accidental).then_some(spelled.accidental)
    }

    /// The signature (up to five sharps or flats) that fits the most music,
    /// weighting notes by how long they sound. Ties go to fewer accidentals.
    pub fn infer(notes: impl IntoIterator<Item = (MidiNote, f32)>) -> Self {
        let notes: Vec<(MidiNote, f32)> = notes.into_iter().collect();
        let fit = |key: Self| -> f32 {
            notes
                .iter()
                .filter(|(n, _)| key.contains(n.pitch_class().semitone()))
                .map(|(_, w)| w)
                .sum()
        };
        (-MAX_ACCIDENTALS..=MAX_ACCIDENTALS)
            .map(Self)
            .map(|key| (key, fit(key)))
            .fold(None, |best: Option<(Self, f32)>, (key, score)| match best {
                Some((b, s)) if s > score + 1e-4 => Some((b, s)),
                Some((b, s)) if (s - score).abs() <= 1e-4 && b.0.abs() <= key.0.abs() => {
                    Some((b, s))
                }
                _ => Some((key, score)),
            })
            .map_or(Self(0), |(key, _)| key)
    }
}
