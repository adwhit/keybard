//! Chords as shortcuts: a triad that "presses" a button when played, in any
//! octave or inversion.

use crate::music::{MidiNote, PitchClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Triad {
    Major,
    Minor,
}

/// A major or minor triad on a root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChordKey {
    root: PitchClass,
    triad: Triad,
}

/// Names as players write them: flats for the black-key majors, sharps for
/// the black-key minors (as their keys are usually spelled).
const MAJOR_NAMES: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];
const MINOR_NAMES: [&str; 12] = [
    "Cm", "C#m", "Dm", "Ebm", "Em", "Fm", "F#m", "Gm", "G#m", "Am", "Bbm", "Bm",
];

impl ChordKey {
    pub const fn major(root: PitchClass) -> Self {
        Self {
            root,
            triad: Triad::Major,
        }
    }

    pub const fn minor(root: PitchClass) -> Self {
        Self {
            root,
            triad: Triad::Minor,
        }
    }

    pub fn pitch_classes(self) -> [PitchClass; 3] {
        let third = match self.triad {
            Triad::Major => 4,
            Triad::Minor => 3,
        };
        let root = self.root.semitone();
        [0, third, 7].map(|interval| PitchClass::from_semitone((root + interval) % 12))
    }

    pub fn label(self) -> String {
        let names = match self.triad {
            Triad::Major => MAJOR_NAMES,
            Triad::Minor => MINOR_NAMES,
        };
        names[usize::from(self.root.semitone())].to_owned()
    }

    /// Whether the held notes are exactly this chord: each of its pitch
    /// classes, in any octave, and nothing else.
    pub fn matches(self, held: &[MidiNote]) -> bool {
        let wanted = self.pitch_classes();
        let played = |pc: &PitchClass| held.iter().any(|n| n.pitch_class() == *pc);
        wanted.iter().all(played) && held.iter().all(|n| wanted.contains(&n.pitch_class()))
    }
}
