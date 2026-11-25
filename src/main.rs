use std::path::PathBuf;
use std::process::ExitCode;

use bevy::app::AppExit;
use clap::{Parser, ValueEnum};
use keybard::app::menu::Screen;
use keybard::app::songs::{Playlist, SongSource};
use keybard::app::{self, Backend, Logging, MidiInput, Options, Player, SnapshotPlan};
use keybard::builtin::BuiltinSong;
use keybard::midi_port::{self, PortChoice};
use keybard::performance::{Hands, PlayMode};
use keybard::saves::{PlayerName, Saves, Settings};
use keybard::song::Seconds;
use keybard::{midi_file, render};

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
    Play,
    Listen,
    Wait,
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

/// A screen to open on instead of the start screen or a song.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum OpenArg {
    ChordBook,
    FlashCards,
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
    /// Play mode (default play; Tab cycles in-app).
    #[arg(long, value_enum)]
    mode: Option<Mode>,
    /// Which hands you play; the computer plays the rest (F2 cycles in-app).
    /// Defaults to the player's saved choice.
    #[arg(long, value_enum)]
    hands: Option<HandsArg>,
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
    /// Open on the chord book, or straight into its flash cards.
    #[arg(long, value_enum, value_name = "SCREEN")]
    open: Option<OpenArg>,
    /// Don't sound notes played on the MIDI keyboard (it has its own
    /// sound); everything the computer plays still sounds. F7 toggles.
    #[arg(long)]
    mute_midi: bool,
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
    /// Where players and their saves live. Defaults to the user data
    /// directory in windowed mode; headless and snapshot runs keep nothing.
    #[arg(long, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    /// Play as this player (created if new) instead of whoever played last.
    #[arg(long, value_name = "NAME")]
    player: Option<String>,
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
    let transient = cli.headless || cli.snapshot.is_some();
    let saves = match (&cli.data_dir, transient) {
        (Some(dir), _) => Saves::at(dir),
        (None, true) => Saves::memory(),
        (None, false) => Saves::default_dir().map_or_else(Saves::memory, Saves::at),
    };
    if let Some(name) = &cli.player {
        match PlayerName::new(name) {
            Ok(name) => {
                if let Err(err) = saves.create(&name) {
                    eprintln!("keybard: {err}");
                    return ExitCode::FAILURE;
                }
            }
            Err(err) => {
                eprintln!("keybard: {err}");
                return ExitCode::FAILURE;
            }
        }
    }
    // The player's saved settings, with anything given here taking over.
    let saved = saves.load_settings(&saves.current());
    let settings = Settings {
        mode: cli.mode.map_or(saved.mode, Into::into),
        hands: cli.hands.map_or(saved.hands, Into::into),
        sight_reading: cli.sight_reading || saved.sight_reading,
        midi_muted: cli.mute_midi || saved.midi_muted,
        ..saved
    };
    let builtin = cli
        .song
        .or_else(|| {
            // Otherwise the song this player last chose.
            let title = settings.song.as_deref()?;
            BuiltinSong::value_variants()
                .iter()
                .copied()
                .find(|b| b.load().is_ok_and(|s| s.title == title))
        })
        .unwrap_or(BuiltinSong::OdeToJoy);
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
    // Asking for something specific goes straight to it; otherwise start on
    // the start screen.
    let direct = transient
        || cli.autopilot
        || cli.midi.is_some()
        || cli.song.is_some()
        || cli.mode.is_some()
        || cli.loop_from.is_some();
    let exit = app::run(Options {
        song,
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
        loop_from: cli.loop_from,
        playlist,
        settings,
        saves,
        screen: match (cli.open, direct && !cli.start_screen) {
            (Some(OpenArg::ChordBook), _) => Screen::ChordBook,
            (Some(OpenArg::FlashCards), _) => Screen::Drill,
            (None, true) => Screen::Song,
            (None, false) => Screen::Menu,
        },
    });
    match exit {
        AppExit::Success => ExitCode::SUCCESS,
        AppExit::Error(_) => ExitCode::FAILURE,
    }
}
