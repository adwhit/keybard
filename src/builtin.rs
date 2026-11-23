//! Built-in public-domain songs, written in the notation parsed by
//! [`crate::song::parse_voice`].

use crate::error::Error;
use crate::song::{Beats, Hand, Song, Tempo, parse_voice};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum BuiltinSong {
    OdeToJoy,
    Twinkle,
    Scale,
    FurElise,
    Minuet,
    JingleBells,
}

struct Score {
    title: &'static str,
    bpm: f32,
    beats_per_bar: u8,
    /// Beats per beam group.
    beam_group: f32,
    right: &'static str,
    left: &'static str,
}

const ODE_TO_JOY: Score = Score {
    title: "Ode to Joy (Beethoven)",
    bpm: 100.0,
    beats_per_bar: 4,
    beam_group: 1.0,
    right: "
        E4 E4 F4 G4 | G4 F4 E4 D4 | C4 C4 D4 E4 | E4/1.5 D4/0.5 D4/2 |
        E4 E4 F4 G4 | G4 F4 E4 D4 | C4 C4 D4 E4 | D4/1.5 C4/0.5 C4/2 |
        D4 D4 E4 C4 | D4 E4/0.5 F4/0.5 E4 C4 | D4 E4/0.5 F4/0.5 E4 D4 | C4 D4 G3/2 |
        E4 E4 F4 G4 | G4 F4 E4 D4 | C4 C4 D4 E4 | D4/1.5 C4/0.5 C4/2 |
    ",
    left: "
        C3+G3/4 | G2+G3/4 | C3+G3/4 | G2+G3/4 |
        C3+G3/4 | G2+G3/4 | C3+G3/2 A2+E3/2 | G2+D3/2 C3+G3/2 |
        G2+D3/4 | C3+G3/4 | G2+D3/4 | C3/2 B2/2 |
        C3+G3/4 | G2+G3/4 | C3+G3/2 A2+E3/2 | G2+D3/2 C3+G3/2 |
    ",
};

const TWINKLE: Score = Score {
    title: "Twinkle, Twinkle, Little Star",
    bpm: 110.0,
    beats_per_bar: 4,
    beam_group: 1.0,
    right: "
        C4 C4 G4 G4 | A4 A4 G4/2 | F4 F4 E4 E4 | D4 D4 C4/2 |
        G4 G4 F4 F4 | E4 E4 D4/2 | G4 G4 F4 F4 | E4 E4 D4/2 |
        C4 C4 G4 G4 | A4 A4 G4/2 | F4 F4 E4 E4 | D4 D4 C4/2 |
    ",
    left: "
        C3/2 E3/2 | F3/2 E3/2 | D3/2 C3/2 | G2/2 C3/2 |
        E3/2 D3/2 | C3/2 G2/2 | E3/2 D3/2 | C3/2 G2/2 |
        C3/2 E3/2 | F3/2 E3/2 | D3/2 C3/2 | G2/2 C3/2 |
    ",
};

const SCALE: Score = Score {
    title: "C major scale, hands together",
    bpm: 90.0,
    beats_per_bar: 4,
    beam_group: 1.0,
    right: "
        C4 D4 E4 F4 | G4 A4 B4 C5 | B4 A4 G4 F4 | E4 D4 C4/2 |
        C#4 D#4 F#4 G#4 | A#4/4 |
    ",
    left: "
        C3 D3 E3 F3 | G3 A3 B3 C4 | B3 A3 G3 F3 | E3 D3 C3/2 |
        C#3 D#3 F#3 G#3 | A#3/4 |
    ",
};

/// Beat = eighth note, so the sixteenths read as flagged notes.
const FUR_ELISE: Score = Score {
    title: "Fur Elise, opening (Beethoven)",
    bpm: 140.0,
    beats_per_bar: 3,
    // 3/8: the printed score beams each bar's sixteenths together.
    beam_group: 3.0,
    right: "
        R/2 E5/0.5 D#5/0.5 |
        E5/0.5 D#5/0.5 E5/0.5 B4/0.5 D5/0.5 C5/0.5 |
        A4 R/0.5 C4/0.5 E4/0.5 A4/0.5 |
        B4 R/0.5 E4/0.5 G#4/0.5 B4/0.5 |
        C5 R/0.5 E4/0.5 E5/0.5 D#5/0.5 |
        E5/0.5 D#5/0.5 E5/0.5 B4/0.5 D5/0.5 C5/0.5 |
        A4 R/0.5 C4/0.5 E4/0.5 A4/0.5 |
        B4 R/0.5 E4/0.5 C5/0.5 B4/0.5 |
        A4/3 |
    ",
    left: "
        R/3 | R/3 |
        A2/0.5 E3/0.5 A3/0.5 R/1.5 |
        E2/0.5 E3/0.5 G#3/0.5 R/1.5 |
        A2/0.5 E3/0.5 A3/0.5 R/1.5 |
        R/3 |
        A2/0.5 E3/0.5 A3/0.5 R/1.5 |
        E2/0.5 E3/0.5 G#3/0.5 R/1.5 |
        A2/3 |
    ",
};

const MINUET: Score = Score {
    title: "Minuet in G (Petzold), simplified",
    bpm: 120.0,
    beats_per_bar: 3,
    beam_group: 1.0,
    right: "
        D5 G4/0.5 A4/0.5 B4/0.5 C5/0.5 | D5 G4 G4 |
        E5 C5/0.5 D5/0.5 E5/0.5 F#5/0.5 | G5 G4 G4 |
        C5 D5/0.5 C5/0.5 B4/0.5 A4/0.5 | B4 C5/0.5 B4/0.5 A4/0.5 G4/0.5 |
        F#4 G4/0.5 A4/0.5 B4/0.5 G4/0.5 | A4/3 |
    ",
    left: "
        G3+B3/2 A3 | B3/3 |
        C4/3 | B3/3 |
        A3/3 | G3/3 |
        D4 B3 G3 | D4 D3/2 |
    ",
};

const JINGLE_BELLS: Score = Score {
    title: "Jingle Bells, chorus (Pierpont)",
    bpm: 132.0,
    beats_per_bar: 4,
    beam_group: 1.0,
    right: "
        E4 E4 E4/2 | E4 E4 E4/2 | E4 G4 C4/1.5 D4/0.5 | E4/4 |
        F4 F4 F4/1.5 F4/0.5 | F4 E4 E4 E4/0.5 E4/0.5 | E4 D4 D4 E4 | D4/2 G4/2 |
        E4 E4 E4/2 | E4 E4 E4/2 | E4 G4 C4/1.5 D4/0.5 | E4/4 |
        F4 F4 F4 F4 | F4 E4 E4 E4/0.5 E4/0.5 | G4 G4 F4 D4 | C4/4 |
    ",
    left: "
        C3+G3/4 | C3+G3/4 | C3+G3/4 | C3+G3/4 |
        F2+C3/4 | C3+G3/4 | G2+D3/4 | G2+B2/4 |
        C3+G3/4 | C3+G3/4 | C3+G3/4 | C3+G3/4 |
        F2+C3/4 | C3+G3/4 | G2+D3/4 | C3+G3/4 |
    ",
};

impl BuiltinSong {
    fn score(self) -> &'static Score {
        match self {
            Self::OdeToJoy => &ODE_TO_JOY,
            Self::Twinkle => &TWINKLE,
            Self::Scale => &SCALE,
            Self::FurElise => &FUR_ELISE,
            Self::Minuet => &MINUET,
            Self::JingleBells => &JINGLE_BELLS,
        }
    }

    pub fn load(self) -> Result<Song, Error> {
        let score = self.score();
        let tempo = Tempo::from_bpm(score.bpm, score.beats_per_bar)
            .with_beam_group(Beats(score.beam_group));
        let right = parse_voice(score.right, Hand::Right, tempo)?;
        let left = parse_voice(score.left, Hand::Left, tempo)?;
        Song::new(score.title, tempo, right.into_iter().chain(left).collect())
    }
}
