use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("MIDI note {0} is out of range (0..=127)")]
    InvalidMidiNote(u8),
    #[error("octave {0} is out of the MIDI range")]
    InvalidOctave(i8),
    #[error("could not parse note token `{0}`")]
    InvalidNoteToken(String),
    #[error("could not read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid MIDI file: {0}")]
    Midi(#[from] midly::Error),
    #[error("SMPTE-timecode MIDI files are not supported")]
    UnsupportedTiming,
    #[error("song contains no notes")]
    EmptySong,
}
