//! Built-in public-domain songs, written in the notation parsed by
//! [`crate::song::parse_voice`].

use crate::error::Error;
use crate::song::{Hand, Song, Tempo, parse_voice};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum BuiltinSong {
    OdeToJoy,
    Twinkle,
    Scale,
}

struct Score {
    title: &'static str,
    bpm: f32,
    beats_per_bar: u8,
    right: &'static str,
    left: &'static str,
}

const ODE_TO_JOY: Score = Score {
    title: "Ode to Joy (Beethoven)",
    bpm: 100.0,
    beats_per_bar: 4,
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
    right: "
        C4 D4 E4 F4 | G4 A4 B4 C5 | B4 A4 G4 F4 | E4 D4 C4/2 |
        C#4 D#4 F#4 G#4 | A#4/2 R/2 |
    ",
    left: "
        C3 D3 E3 F3 | G3 A3 B3 C4 | B3 A3 G3 F3 | E3 D3 C3/2 |
        C#3 D#3 F#3 G#3 | A#3/2 R/2 |
    ",
};

impl BuiltinSong {
    fn score(self) -> &'static Score {
        match self {
            Self::OdeToJoy => &ODE_TO_JOY,
            Self::Twinkle => &TWINKLE,
            Self::Scale => &SCALE,
        }
    }

    pub fn load(self) -> Result<Song, Error> {
        let score = self.score();
        let tempo = Tempo::from_bpm(score.bpm, score.beats_per_bar);
        let right = parse_voice(score.right, Hand::Right, tempo)?;
        let left = parse_voice(score.left, Hand::Left, tempo)?;
        Song::new(score.title, tempo, right.into_iter().chain(left).collect())
    }
}
