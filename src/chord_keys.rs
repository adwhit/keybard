//! Triads: as shortcuts that "press" a button when played (in any octave
//! or inversion), and as pages of the chord book.
//!
//! The book holds only major and minor triads, so the book's own buttons
//! use sus4 and diminished chords: playing a page (or a flash card) can
//! never press one.

use crate::music::{MidiNote, PitchClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Triad {
    Major,
    Minor,
    /// A fourth in place of the third: C F G.
    Sus4,
    /// A minor third and a flattened fifth: B D F.
    Diminished,
}

impl Triad {
    /// Semitones above the root.
    const fn intervals(self) -> [u8; 3] {
        match self {
            Self::Major => [0, 4, 7],
            Self::Minor => [0, 3, 7],
            Self::Sus4 => [0, 5, 7],
            Self::Diminished => [0, 3, 6],
        }
    }
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

    pub const fn sus4(root: PitchClass) -> Self {
        Self {
            root,
            triad: Triad::Sus4,
        }
    }

    pub const fn diminished(root: PitchClass) -> Self {
        Self {
            root,
            triad: Triad::Diminished,
        }
    }

    pub fn pitch_classes(self) -> [PitchClass; 3] {
        let root = self.root.semitone();
        self.triad
            .intervals()
            .map(|interval| PitchClass::from_semitone((root + interval) % 12))
    }

    /// The chord symbol: "C", "F#m", "Csus4", "Bdim".
    pub fn label(self) -> String {
        let root = usize::from(self.root.semitone());
        match self.triad {
            Triad::Major => MAJOR_NAMES[root].to_owned(),
            Triad::Minor => MINOR_NAMES[root].to_owned(),
            Triad::Sus4 => format!("{}sus4", MAJOR_NAMES[root]),
            Triad::Diminished => format!("{}dim", MAJOR_NAMES[root]),
        }
    }

    /// The name in words: "C major", "F# minor".
    pub fn name(self) -> String {
        let label = self.label();
        match self.triad {
            Triad::Major => format!("{label} major"),
            Triad::Minor => format!("{} minor", label.trim_end_matches('m')),
            Triad::Sus4 => format!("{} suspended fourth", label.trim_end_matches("sus4")),
            Triad::Diminished => format!("{} diminished", label.trim_end_matches("dim")),
        }
    }

    /// The book's chord that the held notes are, if any.
    pub fn in_book(held: &[MidiNote]) -> Option<Self> {
        BOOK.iter().copied().find(|chord| chord.matches(held))
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
            Triad::Sus4 | Triad::Diminished => false,
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
        inversion.arrange(self.note_names())
    }

    /// The keys of this chord in `inversion`, bottom note from G3 up to F#4
    /// (so root-position chords sit around middle C).
    pub fn keys(self, inversion: Inversion) -> Vec<MidiNote> {
        let order = inversion.arrange(self.pitch_classes().map(|pc| pc.semitone()));
        // The first `pc` key above `base` (or `base` itself).
        let above = |base: u8, pc: u8| base + (pc + 12 - base % 12) % 12;
        let low = above(55, order[0]);
        let middle = above(low + 1, order[1]);
        let top = above(middle + 1, order[2]);
        [low, middle, top]
            .into_iter()
            .filter_map(|v| MidiNote::new(v).ok())
            .collect()
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

    /// Root, third and fifth reordered from the bottom up.
    pub fn arrange<T>(self, [root, third, fifth]: [T; 3]) -> [T; 3] {
        match self {
            Self::Root => [root, third, fifth],
            Self::First => [third, fifth, root],
            Self::Second => [fifth, root, third],
        }
    }

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
