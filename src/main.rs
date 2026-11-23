use std::path::PathBuf;
use std::process::ExitCode;

use bevy::app::AppExit;
use clap::Parser;
use keybard::app::{self, Backend, Logging, MidiInput, Options};
use keybard::builtin::BuiltinSong;
use keybard::midi_file;
use keybard::performance::PlayMode;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum Mode {
    Listen,
    Wait,
    Play,
}

impl From<Mode> for PlayMode {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Listen => Self::Listen,
            Mode::Wait => Self::Wait,
            Mode::Play => Self::Play,
        }
    }
}

/// Falling-bars piano trainer.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// A Standard MIDI File (.mid) to play. Overrides --song.
    midi: Option<PathBuf>,
    /// Built-in song to play when no MIDI file is given.
    #[arg(long, value_enum, default_value_t = BuiltinSong::OdeToJoy)]
    song: BuiltinSong,
    /// Starting play mode (Tab cycles in-app).
    #[arg(long, value_enum, default_value_t = Mode::Listen)]
    mode: Mode,
    /// Don't connect to MIDI input devices.
    #[arg(long)]
    no_midi: bool,
    /// Run without a window, GPU or sound; exits when the song finishes.
    #[arg(long)]
    headless: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let song = match &cli.midi {
        Some(path) => midi_file::load(path),
        None => cli.song.load(),
    };
    let song = match song {
        Ok(song) => song,
        Err(err) => {
            eprintln!("keybard: {err}");
            return ExitCode::FAILURE;
        }
    };
    let exit = app::run(Options {
        song,
        mode: cli.mode.into(),
        midi_input: if cli.no_midi {
            MidiInput::Disabled
        } else {
            MidiInput::Enabled
        },
        backend: if cli.headless {
            Backend::Headless
        } else {
            Backend::Windowed
        },
        logging: Logging::Enabled,
    });
    match exit {
        AppExit::Success => ExitCode::SUCCESS,
        AppExit::Error(_) => ExitCode::FAILURE,
    }
}
