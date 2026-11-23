use std::path::PathBuf;
use std::process::ExitCode;

use bevy::app::AppExit;
use clap::Parser;
use directories::ProjectDirs;
use keybard::app::{self, Backend, Logging, MidiInput, Options, Player, RecordStore, SnapshotPlan};
use keybard::builtin::BuiltinSong;
use keybard::midi_file;
use keybard::performance::{Hands, PlayMode};
use keybard::song::Seconds;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum HandsArg {
    Both,
    Right,
    Left,
}

impl From<HandsArg> for Hands {
    fn from(hands: HandsArg) -> Self {
        match hands {
            HandsArg::Both => Self::Both,
            HandsArg::Right => Self::Right,
            HandsArg::Left => Self::Left,
        }
    }
}

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
    /// Which hands you play; the computer plays the rest (F2 cycles in-app).
    #[arg(long, value_enum, default_value_t = HandsArg::Both)]
    hands: HandsArg,
    /// Start looping 4 bars from this bar number (F3 toggles in-app).
    #[arg(long = "loop", value_name = "BAR")]
    loop_from: Option<u32>,
    /// Don't connect to MIDI input devices.
    #[arg(long)]
    no_midi: bool,
    /// Run without a window, GPU or sound; exits when the song finishes.
    #[arg(long, conflicts_with = "snapshot")]
    headless: bool,
    /// Render offscreen (no window) and save PNGs into this directory at each
    /// --at time, then exit.
    #[arg(long, value_name = "DIR")]
    snapshot: Option<PathBuf>,
    /// Song time in seconds to capture with --snapshot (repeatable).
    #[arg(long = "at", value_name = "SECONDS", default_values_t = [4.0])]
    snapshot_times: Vec<f32>,
    /// Let the built-in autopilot play (Wait/Play modes) — handy as a demo.
    #[arg(long)]
    autopilot: bool,
    /// Where to keep personal bests. Defaults to the user data directory in
    /// windowed mode; headless and snapshot runs never touch it.
    #[arg(long, value_name = "FILE")]
    records: Option<PathBuf>,
    /// Resolution multiplier for --snapshot images.
    #[arg(long, default_value_t = 1.0)]
    snapshot_scale: f32,
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
    let records = match (&cli.records, cli.headless || cli.snapshot.is_some()) {
        (Some(path), _) => RecordStore::File(path.clone()),
        (None, true) => RecordStore::Memory,
        (None, false) => ProjectDirs::from("", "", "keybard").map_or(RecordStore::Memory, |dirs| {
            RecordStore::File(dirs.data_dir().join("bests.tsv"))
        }),
    };
    let exit = app::run(Options {
        song,
        mode: cli.mode.into(),
        midi_input: if cli.no_midi {
            MidiInput::Disabled
        } else {
            MidiInput::Enabled
        },
        backend: match (&cli.snapshot, cli.headless) {
            (Some(dir), _) => Backend::Offscreen(SnapshotPlan {
                dir: dir.clone(),
                times: cli.snapshot_times.iter().copied().map(Seconds).collect(),
                scale: cli.snapshot_scale,
            }),
            (None, true) => Backend::Headless,
            (None, false) => Backend::Windowed,
        },
        logging: Logging::Enabled,
        player: if cli.autopilot {
            Player::Autopilot
        } else {
            Player::Human
        },
        records,
        hands: cli.hands.into(),
        loop_from: cli.loop_from,
    });
    match exit {
        AppExit::Success => ExitCode::SUCCESS,
        AppExit::Error(_) => ExitCode::FAILURE,
    }
}
