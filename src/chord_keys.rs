//! Triads: as shortcuts that "press" a button when played (in any octave
//! or inversion), and as pages of the chord book.

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

    /// Whether this chord is written with flats (Bb, Eb, ...) rather than
    /// sharps, as its key would be.
    fn uses_flats(self) -> bool {
        use PitchClass as P;
        match self.triad {
            Triad::Major => matches!(
                self.root,
                P::F | P::ASharp | P::DSharp | P::GSharp | P::CSharp
            ),
            Triad::Minor => matches!(self.root, P::D | P::G | P::C | P::F | P::ASharp | P::DSharp),
        }
    }

    /// The chord's notes, root first, spelled for its key.
    pub fn note_names(self) -> [&'static str; 3] {
        const SHARPS: [&str; 12] = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        const FLATS: [&str; 12] = [
            "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
        ];
        let names = if self.uses_flats() { FLATS } else { SHARPS };
        self.pitch_classes()
            .map(|pc| names[usize::from(pc.semitone())])
    }

    /// The notes from the bottom up in `inversion`.
    pub fn voicing(self, inversion: Inversion) -> [&'static str; 3] {
        let [root, third, fifth] = self.note_names();
        match inversion {
            Inversion::Root => [root, third, fifth],
            Inversion::First => [third, fifth, root],
            Inversion::Second => [fifth, root, third],
        }
    }

    /// Which inversion the held notes are, if they're this chord: told by
    /// the lowest note.
    pub fn inversion(self, held: &[MidiNote]) -> Option<Inversion> {
        if !self.matches(held) {
            return None;
        }
        let lowest = held.iter().min()?.pitch_class();
        let [root, third, _] = self.pitch_classes();
        Some(match lowest {
            pc if pc == root => Inversion::Root,
            pc if pc == third => Inversion::First,
            _ => Inversion::Second,
        })
    }

    /// Whether the held notes are exactly this chord: each of its pitch
    /// classes, in any octave, and nothing else.
    pub fn matches(self, held: &[MidiNote]) -> bool {
        let wanted = self.pitch_classes();
        let played = |pc: &PitchClass| held.iter().any(|n| n.pitch_class() == *pc);
        wanted.iter().all(played) && held.iter().all(|n| wanted.contains(&n.pitch_class()))
    }
}

/// Which chord note is at the bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Inversion {
    Root,
    First,
    Second,
}

impl Inversion {
    pub const ALL: [Self; 3] = [Self::Root, Self::First, Self::Second];

    pub fn label(self) -> &'static str {
        match self {
            Self::Root => "root position",
            Self::First => "first inversion",
            Self::Second => "second inversion",
        }
    }
}

/// Every major and minor triad, easiest first: round the circle of fifths,
/// each major followed by its relative minor.
pub const BOOK: [ChordKey; 24] = {
    use PitchClass as P;
    [
        ChordKey::major(P::C),
        ChordKey::minor(P::A),
        ChordKey::major(P::G),
        ChordKey::minor(P::E),
        ChordKey::major(P::F),
        ChordKey::minor(P::D),
        ChordKey::major(P::D),
        ChordKey::minor(P::B),
        ChordKey::major(P::A),
        ChordKey::minor(P::FSharp),
        ChordKey::major(P::E),
        ChordKey::minor(P::CSharp),
        ChordKey::major(P::ASharp),
        ChordKey::minor(P::G),
        ChordKey::major(P::DSharp),
        ChordKey::minor(P::C),
        ChordKey::major(P::GSharp),
        ChordKey::minor(P::F),
        ChordKey::major(P::CSharp),
        ChordKey::minor(P::ASharp),
        ChordKey::major(P::FSharp),
        ChordKey::minor(P::DSharp),
        ChordKey::major(P::B),
        ChordKey::minor(P::GSharp),
    ]
};

/// Three neighbouring keys held together (C, C#, D, say): the gesture that
/// pauses and resumes a song from the piano. Nothing in a song asks for it,
/// and other held keys don't get in the way. Returns the three keys.
pub fn pause_cluster(held: &[MidiNote]) -> Option<[MidiNote; 3]> {
    let neighbour = |low: MidiNote, step: u8| {
        MidiNote::new(low.value() + step)
            .ok()
            .filter(|n| held.contains(n))
    };
    held.iter()
        .copied()
        .find_map(|low| Some([low, neighbour(low, 1)?, neighbour(low, 2)?]))
}
