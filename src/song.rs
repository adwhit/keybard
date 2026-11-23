//! Song model and a tiny text notation used for the built-in songs.

use std::ops::{Add, Sub};

use crate::error::Error;
use crate::fingering::{self, Finger};
use crate::music::{Letter, MidiNote};

/// A point in (or span of) song time, in seconds.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Seconds(pub f32);

impl Add for Seconds {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Seconds {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

/// Musical time in beats (quarter notes for the built-in songs).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Beats(pub f32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hand {
    Left,
    Right,
}

/// Index of a note within [`Song::notes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoteId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    pub pitch: MidiNote,
    pub start: Seconds,
    pub duration: Seconds,
    pub hand: Hand,
}

impl Note {
    pub fn end(&self) -> Seconds {
        self.start + self.duration
    }

    pub fn is_sounding_at(&self, time: Seconds) -> bool {
        self.start <= time && time < self.end()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tempo {
    pub seconds_per_beat: Seconds,
    pub beats_per_bar: u8,
}

impl Tempo {
    pub fn from_bpm(bpm: f32, beats_per_bar: u8) -> Self {
        Self {
            seconds_per_beat: Seconds(60.0 / bpm),
            beats_per_bar,
        }
    }

    pub fn to_seconds(self, beats: Beats) -> Seconds {
        Seconds(beats.0 * self.seconds_per_beat.0)
    }

    pub fn to_beats(self, seconds: Seconds) -> Beats {
        Beats(seconds.0 / self.seconds_per_beat.0)
    }

    pub fn bar_length(self) -> Seconds {
        self.to_seconds(Beats(f32::from(self.beats_per_bar)))
    }

    /// Count-in beats (one bar before time zero) that fall in `(from, to]`.
    pub fn count_in(self, from: Seconds, to: Seconds) -> impl Iterator<Item = CountIn> {
        let beat = self.seconds_per_beat.0;
        (1..=self.beats_per_bar)
            .rev()
            .map(move |remaining| CountIn {
                remaining,
                at: Seconds(-f32::from(remaining) * beat),
            })
            .filter(move |c| from < c.at && c.at <= to)
    }
}

/// One click of the count-in: `remaining` beats until the music starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CountIn {
    pub remaining: u8,
    pub at: Seconds,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Song {
    pub title: String,
    pub tempo: Tempo,
    /// Sorted by start time, then pitch.
    notes: Vec<Note>,
    /// Recommended finger for each note, parallel to `notes`.
    fingers: Vec<Finger>,
}

impl Song {
    pub fn new(
        title: impl Into<String>,
        tempo: Tempo,
        mut notes: Vec<Note>,
    ) -> Result<Self, Error> {
        if notes.is_empty() {
            return Err(Error::EmptySong);
        }
        notes.sort_by(|a, b| a.start.0.total_cmp(&b.start.0).then(a.pitch.cmp(&b.pitch)));
        let fingers = fingering::assign(&notes);
        Ok(Self {
            title: title.into(),
            tempo,
            notes,
            fingers,
        })
    }

    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    pub fn note(&self, id: NoteId) -> &Note {
        &self.notes[id.0]
    }

    /// The recommended finger for a note.
    pub fn finger(&self, id: NoteId) -> Finger {
        self.fingers[id.0]
    }

    pub fn iter(&self) -> impl Iterator<Item = (NoteId, &Note)> {
        self.notes.iter().enumerate().map(|(i, n)| (NoteId(i), n))
    }

    pub fn duration(&self) -> Seconds {
        self.notes
            .iter()
            .map(Note::end)
            .fold(Seconds(0.0), |a, b| if b > a { b } else { a })
    }

    /// Lowest and highest pitch in the song. Songs are never empty.
    pub fn pitch_range(&self) -> (MidiNote, MidiNote) {
        let low = self.notes.iter().map(|n| n.pitch).min();
        let high = self.notes.iter().map(|n| n.pitch).max();
        low.zip(high)
            .unwrap_or((MidiNote::MIDDLE_C, MidiNote::MIDDLE_C))
    }
}

/// Parse a pitch such as `C4`, `F#3` or `A-1`.
pub fn parse_pitch(token: &str) -> Result<MidiNote, Error> {
    let bad = || Error::InvalidNoteToken(token.to_owned());
    let mut chars = token.chars();
    let letter = match chars.next().ok_or_else(bad)?.to_ascii_uppercase() {
        'C' => Letter::C,
        'D' => Letter::D,
        'E' => Letter::E,
        'F' => Letter::F,
        'G' => Letter::G,
        'A' => Letter::A,
        'B' => Letter::B,
        _ => return Err(bad()),
    };
    let rest = chars.as_str();
    let (sharp, octave) = rest.strip_prefix('#').map_or((false, rest), |r| (true, r));
    let octave: i8 = octave.parse().map_err(|_| bad())?;
    let natural = MidiNote::from_parts(letter.natural(), octave)?;
    if sharp {
        natural.offset(1).ok_or_else(bad)
    } else {
        Ok(natural)
    }
}

/// One event of a voice: a chord (possibly a single note) or a rest.
enum VoiceEvent {
    Chord(Vec<MidiNote>, Beats),
    Rest(Beats),
}

fn parse_event(token: &str) -> Result<VoiceEvent, Error> {
    let (pitches, beats) = match token.split_once('/') {
        Some((p, b)) => (
            p,
            Beats(
                b.parse()
                    .map_err(|_| Error::InvalidNoteToken(token.to_owned()))?,
            ),
        ),
        None => (token, Beats(1.0)),
    };
    if pitches.eq_ignore_ascii_case("r") {
        return Ok(VoiceEvent::Rest(beats));
    }
    pitches
        .split('+')
        .map(parse_pitch)
        .collect::<Result<Vec<_>, _>>()
        .map(|chord| VoiceEvent::Chord(chord, beats))
}

/// Parse a whitespace-separated voice like `C4 E4/0.5 C4+E4+G4/2 R/1 |`.
///
/// Each token is `pitch[+pitch...][/beats]` or `R[/beats]`; beats default to 1.
/// `|` bar lines are ignored and are only there for readability.
pub fn parse_voice(text: &str, hand: Hand, tempo: Tempo) -> Result<Vec<Note>, Error> {
    let events = text
        .split_whitespace()
        .filter(|t| *t != "|")
        .map(parse_event)
        .collect::<Result<Vec<_>, _>>()?;
    let (notes, _) = events.into_iter().fold(
        (Vec::new(), Beats(0.0)),
        |(mut notes, cursor), event| match event {
            VoiceEvent::Rest(beats) => (notes, Beats(cursor.0 + beats.0)),
            VoiceEvent::Chord(chord, beats) => {
                notes.extend(chord.into_iter().map(|pitch| Note {
                    pitch,
                    start: tempo.to_seconds(cursor),
                    duration: tempo.to_seconds(beats),
                    hand,
                }));
                (notes, Beats(cursor.0 + beats.0))
            }
        },
    );
    Ok(notes)
}
