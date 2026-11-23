//! Pure music-theory primitives: pitches, letters, clefs and staff positions.

use std::fmt;

use crate::error::Error;

/// A MIDI note number in the range `0..=127`. Middle C is 60.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MidiNote(u8);

impl MidiNote {
    pub const MIDDLE_C: Self = Self(60);
    pub const MAX: u8 = 127;

    pub fn new(value: u8) -> Result<Self, Error> {
        (value <= Self::MAX)
            .then_some(Self(value))
            .ok_or(Error::InvalidMidiNote(value))
    }

    /// Build a note from a pitch class and a scientific-pitch octave (C4 = 60).
    pub fn from_parts(pitch_class: PitchClass, octave: i8) -> Result<Self, Error> {
        let value = (i16::from(octave) + 1) * 12 + i16::from(pitch_class.semitone());
        u8::try_from(value)
            .map_err(|_| Error::InvalidOctave(octave))
            .and_then(Self::new)
    }

    pub fn value(self) -> u8 {
        self.0
    }

    pub fn pitch_class(self) -> PitchClass {
        PitchClass::from_semitone(self.0 % 12)
    }

    /// Scientific-pitch octave number (middle C is in octave 4).
    pub fn octave(self) -> i8 {
        // 127 / 12 - 1 == 9, so this always fits.
        (self.0 / 12) as i8 - 1
    }

    pub fn key_color(self) -> KeyColor {
        match self.pitch_class().accidental() {
            Accidental::Natural => KeyColor::White,
            Accidental::Sharp | Accidental::Flat => KeyColor::Black,
        }
    }

    /// Equal-temperament frequency with A4 = 440Hz.
    pub fn frequency(self) -> f32 {
        440.0 * 2.0_f32.powf((f32::from(self.0) - 69.0) / 12.0)
    }

    /// Number of diatonic (letter-name) steps from C-1. Sharps share the step
    /// of their natural, e.g. C#4 and C4 sit on the same staff position.
    pub fn diatonic_step(self) -> i32 {
        i32::from(self.octave() + 1) * 7 + self.pitch_class().letter().index()
    }

    /// Step to the semitone above or below, if it is still a valid MIDI note.
    pub fn offset(self, semitones: i16) -> Option<Self> {
        u8::try_from(i16::from(self.0) + semitones)
            .ok()
            .and_then(|v| Self::new(v).ok())
    }

    /// All notes from `self` to `high` inclusive.
    pub fn range_inclusive(self, high: Self) -> impl Iterator<Item = Self> {
        (self.0..=high.0).map(Self)
    }
}

impl fmt::Display for MidiNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.pitch_class(), self.octave())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyColor {
    White,
    Black,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Letter {
    C,
    D,
    E,
    F,
    G,
    A,
    B,
}

impl Letter {
    pub const ALL: [Self; 7] = [
        Self::C,
        Self::D,
        Self::E,
        Self::F,
        Self::G,
        Self::A,
        Self::B,
    ];

    pub fn index(self) -> i32 {
        match self {
            Self::C => 0,
            Self::D => 1,
            Self::E => 2,
            Self::F => 3,
            Self::G => 4,
            Self::A => 5,
            Self::B => 6,
        }
    }

    pub fn natural(self) -> PitchClass {
        match self {
            Self::C => PitchClass::C,
            Self::D => PitchClass::D,
            Self::E => PitchClass::E,
            Self::F => PitchClass::F,
            Self::G => PitchClass::G,
            Self::A => PitchClass::A,
            Self::B => PitchClass::B,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accidental {
    Natural,
    Sharp,
    Flat,
}

/// How a pitch is written: a letter, an accidental and the octave of the
/// letter (which can differ from the pitch's octave for e.g. Cb).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spelled {
    pub letter: Letter,
    pub accidental: Accidental,
    pub octave: i8,
}

impl Spelled {
    pub fn diatonic_step(self) -> i32 {
        i32::from(self.octave + 1) * 7 + self.letter.index()
    }
}

/// The twelve pitch classes, spelled with sharps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PitchClass {
    C,
    CSharp,
    D,
    DSharp,
    E,
    F,
    FSharp,
    G,
    GSharp,
    A,
    ASharp,
    B,
}

impl PitchClass {
    const ALL: [Self; 12] = [
        Self::C,
        Self::CSharp,
        Self::D,
        Self::DSharp,
        Self::E,
        Self::F,
        Self::FSharp,
        Self::G,
        Self::GSharp,
        Self::A,
        Self::ASharp,
        Self::B,
    ];

    pub fn from_semitone(semitone: u8) -> Self {
        Self::ALL[usize::from(semitone % 12)]
    }

    pub fn semitone(self) -> u8 {
        match self {
            Self::C => 0,
            Self::CSharp => 1,
            Self::D => 2,
            Self::DSharp => 3,
            Self::E => 4,
            Self::F => 5,
            Self::FSharp => 6,
            Self::G => 7,
            Self::GSharp => 8,
            Self::A => 9,
            Self::ASharp => 10,
            Self::B => 11,
        }
    }

    pub fn letter(self) -> Letter {
        match self {
            Self::C | Self::CSharp => Letter::C,
            Self::D | Self::DSharp => Letter::D,
            Self::E => Letter::E,
            Self::F | Self::FSharp => Letter::F,
            Self::G | Self::GSharp => Letter::G,
            Self::A | Self::ASharp => Letter::A,
            Self::B => Letter::B,
        }
    }

    pub fn accidental(self) -> Accidental {
        match self {
            Self::CSharp | Self::DSharp | Self::FSharp | Self::GSharp | Self::ASharp => {
                Accidental::Sharp
            }
            _ => Accidental::Natural,
        }
    }
}

impl fmt::Display for PitchClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let letter = match self.letter() {
            Letter::C => "C",
            Letter::D => "D",
            Letter::E => "E",
            Letter::F => "F",
            Letter::G => "G",
            Letter::A => "A",
            Letter::B => "B",
        };
        let accidental = match self.accidental() {
            Accidental::Natural => "",
            Accidental::Sharp => "#",
            Accidental::Flat => "b",
        };
        write!(f, "{letter}{accidental}")
    }
}

/// Vertical position on a five-line staff, in half-spaces. 0 is the bottom
/// line, 8 is the top line; odd numbers are spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StaffPosition(pub i32);

impl StaffPosition {
    pub const BOTTOM_LINE: Self = Self(0);
    pub const MIDDLE_LINE: Self = Self(4);
    pub const TOP_LINE: Self = Self(8);

    /// Positions of the ledger lines needed to draw a note here, outermost last.
    pub fn ledger_lines(self) -> Vec<Self> {
        match self.0 {
            p if p <= -2 => (p..=-2).rev().filter(|v| v % 2 == 0).map(Self).collect(),
            p if p >= 10 => (10..=p).filter(|v| v % 2 == 0).map(Self).collect(),
            _ => Vec::new(),
        }
    }

    /// Stems point down for notes on or above the middle line.
    pub fn stem_direction(self) -> StemDirection {
        if self >= Self::MIDDLE_LINE {
            StemDirection::Down
        } else {
            StemDirection::Up
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StemDirection {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clef {
    Treble,
    Bass,
}

impl Clef {
    pub const ALL: [Self; 2] = [Self::Treble, Self::Bass];

    /// The note sitting on the bottom line of the staff.
    fn bottom_line(self) -> MidiNote {
        match self {
            Self::Treble => MidiNote(64), // E4
            Self::Bass => MidiNote(43),   // G2
        }
    }

    /// Where a spelled note sits: Bb on the B line, A# in the A space.
    pub fn position_of(self, spelled: Spelled) -> StaffPosition {
        StaffPosition(spelled.diatonic_step() - self.bottom_line().diatonic_step())
    }

    pub fn staff_position(self, note: MidiNote) -> StaffPosition {
        StaffPosition(note.diatonic_step() - self.bottom_line().diatonic_step())
    }

    /// Pick the clef a note is most naturally written on: middle C and above
    /// go on the treble staff.
    pub fn for_note(note: MidiNote) -> Self {
        if note >= MidiNote::MIDDLE_C {
            Self::Treble
        } else {
            Self::Bass
        }
    }
}
