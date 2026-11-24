use std::path::PathBuf;
use std::process::ExitCode;

use bevy::app::AppExit;
use clap::{Parser, ValueEnum};
use keybard::app::falling::View;
use keybard::app::menu::Screen;
use keybard::app::songs::{Playlist, SongSource};
use keybard::app::{self, Backend, Logging, MidiInput, Options, Player, RecordStore, SnapshotPlan};
use keybard::builtin::BuiltinSong;
use keybard::midi_port::{self, PortChoice};
use keybard::performance::{Hands, PlayMode};
use keybard::song::Seconds;
use keybard::{midi_file, records, render};

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
    /// Built-in song to play when no MIDI file is given. Skips the start
    /// screen, as do a MIDI file, --mode, --loop and --autopilot.
    #[arg(long, value_enum)]
    song: Option<BuiltinSong>,
    /// Play mode (default listen; Tab cycles in-app).
    #[arg(long, value_enum)]
    mode: Option<Mode>,
    /// Which hands you play; the computer plays the rest (F2 cycles in-app).
    #[arg(long, value_enum, default_value_t = HandsArg::Both)]
    hands: HandsArg,
    /// Start looping 4 bars from this bar number (F3 toggles in-app).
    #[arg(long = "loop", value_name = "BAR")]
    loop_from: Option<u32>,
    /// Connect to the MIDI input whose name contains this text.
    #[arg(long, value_name = "NAME", conflicts_with = "no_midi")]
    midi_port: Option<String>,
    /// List MIDI input ports and exit.
    #[arg(long)]
    list_midi_ports: bool,
    /// Start in sight-reading mode: no falling bars or keyboard cues (F6).
    #[arg(long)]
    sight_reading: bool,
    /// Open on the start screen even when a song or mode is given.
    #[arg(long)]
    start_screen: bool,
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
    /// Render the song through the synth into a WAV file and exit (no window).
    #[arg(long, value_name = "FILE", conflicts_with_all = ["snapshot", "headless"])]
    render_wav: Option<PathBuf>,
    /// Resolution multiplier for --snapshot images.
    #[arg(long, default_value_t = 1.0)]
    snapshot_scale: f32,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.list_midi_ports {
        let ports = midi_port::list_ports();
        if ports.is_empty() {
            println!("No MIDI input ports found.");
        }
        ports.iter().for_each(|name| println!("{name}"));
        return ExitCode::SUCCESS;
    }
    let builtin = cli.song.unwrap_or(BuiltinSong::OdeToJoy);
    let song = match &cli.midi {
        Some(path) => midi_file::load(path),
        None => builtin.load(),
    };
    // A MIDI file goes first, followed by every built-in.
    let builtins = BuiltinSong::value_variants()
        .iter()
        .map(|b| SongSource::Builtin(*b));
    let playlist = match &cli.midi {
        Some(path) => Playlist::new(
            std::iter::once(SongSource::File(path.clone()))
                .chain(builtins)
                .collect(),
            0,
        ),
        None => Playlist::new(
            builtins.collect(),
            BuiltinSong::value_variants()
                .iter()
                .position(|b| *b == builtin)
                .unwrap_or(0),
        ),
    };
    let song = match song {
        Ok(song) => song,
        Err(err) => {
            eprintln!("keybard: {err}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(path) = &cli.render_wav {
        return match render::write_wav(path, &render::render_song(&song)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("keybard: {err}");
                ExitCode::FAILURE
            }
        };
    }
    let transient = cli.headless || cli.snapshot.is_some();
    // Asking for something specific goes straight to it; otherwise start on
    // the start screen.
    let direct = transient
        || cli.autopilot
        || cli.midi.is_some()
        || cli.song.is_some()
        || cli.mode.is_some()
        || cli.loop_from.is_some();
    let practice_log = match (transient, records::default_path()) {
        (false, Some(bests)) => RecordStore::File(bests.with_file_name("practice.tsv")),
        _ => RecordStore::Memory,
    };
    let records = match (&cli.records, cli.headless || cli.snapshot.is_some()) {
        (Some(path), _) => RecordStore::File(path.clone()),
        (None, true) => RecordStore::Memory,
        (None, false) => records::default_path().map_or(RecordStore::Memory, RecordStore::File),
    };
    let exit = app::run(Options {
        song,
        mode: cli.mode.unwrap_or(Mode::Listen).into(),
        midi_input: match (cli.no_midi, &cli.midi_port) {
            (true, _) => MidiInput::Disabled,
            (false, Some(name)) => MidiInput::Enabled(PortChoice::Named(name.clone())),
            (false, None) => MidiInput::Enabled(PortChoice::First),
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
        playlist,
        practice_log,
        view: if cli.sight_reading {
            View::SightReading
        } else {
            View::Bars
        },
        screen: if direct && !cli.start_screen {
            Screen::Song
        } else {
            Screen::Menu
        },
    });
    match exit {
        AppExit::Success => ExitCode::SUCCESS,
        AppExit::Error(_) => ExitCode::FAILURE,
    }
}
