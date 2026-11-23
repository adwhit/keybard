//! Bevy application: wires the pure song/performance model to rendering,
//! input and audio.
//!
//! [`build`] produces an [`App`] for either a real window or a headless
//! backend (no window, no GPU, no audio device). The headless app can be
//! stepped frame-by-frame with [`App::update`] and inspected through the
//! public resources and components below, which is how the integration tests
//! drive it.

mod audio;
mod autopilot;
pub mod effects;
pub mod falling;
pub mod heat;
pub mod hud;
pub mod input;
pub mod notation;
pub mod palette;
pub mod piano;
pub mod practice;
pub mod results;
pub mod section;
mod snapshot;
pub mod songs;
mod textures;
pub mod transport;

use std::time::Duration;

use bevy::app::{PluginGroupBuilder, ScheduleRunnerPlugin};
use bevy::audio::AudioPlugin;
use bevy::camera::ScalingMode;
use bevy::ecs::schedule::SingleThreadedExecutor;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::WgpuSettings;
use bevy::time::TimeUpdateStrategy;
use bevy::window::{ExitCondition, WindowResolution};
use bevy::winit::WinitPlugin;
use tracing::info;

pub use audio::PlayTone;
pub use autopilot::jitter as autopilot_jitter;
pub use results::RecordStore;
pub use snapshot::SnapshotPlan;

use crate::dynamics::Touch;
use crate::layout::{self, KeyRange};
use crate::midi_port::PortChoice;
use crate::music::MidiNote;
use crate::performance::{Hands, Judgement, Performance, PlayMode};
use crate::song::{NoteId, Song};
use transport::{PlayState, Transport};

/// The song being played.
#[derive(Resource)]
pub struct ActiveSong(pub Song);

/// Judgement state for the active song.
#[derive(Resource)]
pub struct Judge(pub Performance);

/// What just happened, for visual and audio feedback. Emitted during
/// [`Phase::Simulate`].
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub enum Feedback {
    Hit(Judgement),
    /// Played by the computer (Listen mode).
    AutoPlayed(NoteId),
    Missed(NoteId),
    /// A MIDI hit played well away from its marked dynamic.
    Touch {
        note: NoteId,
        touch: Touch,
    },
    /// A press that matched no note.
    Wrong {
        pressed: MidiNote,
        /// The nearest note that was due, if any.
        expected: Option<MidiNote>,
    },
}

/// The span of piano keys on screen.
#[derive(Resource, Clone, Copy)]
pub struct Keys(pub KeyRange);

/// Run ordering within `Update`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Read keyboard / MIDI, emit `NoteInput`.
    Input,
    /// Advance the clock and judge presses.
    Simulate,
    /// Move sprites, draw gizmos, play sounds.
    Present,
}

#[derive(Debug, Clone)]
pub enum Backend {
    /// A real window with rendering and sound.
    Windowed,
    /// No window, GPU or audio device. Scene entities are still spawned and
    /// updated, so their state can be inspected.
    Headless,
    /// Render on the GPU into an image (no window) and save PNG snapshots at
    /// the planned song times, then exit. The clock runs at a fixed 60fps.
    Offscreen(SnapshotPlan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Logging {
    Enabled,
    /// Needed when building several apps in one process (e.g. tests), since
    /// the global tracing subscriber can only be installed once.
    Disabled,
}

/// Who presses the keys in Wait and Play modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Player {
    Human,
    /// The built-in autopilot plays (with slight timing jitter).
    Autopilot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MidiInput {
    Enabled(PortChoice),
    Disabled,
}

pub struct Options {
    pub song: Song,
    pub mode: PlayMode,
    pub midi_input: MidiInput,
    pub backend: Backend,
    pub logging: Logging,
    pub player: Player,
    pub records: RecordStore,
    pub hands: Hands,
    /// Start looping `section::LOOP_BARS` bars from this (1-based) bar.
    pub loop_from: Option<u32>,
    /// Songs PgUp/PgDn switch between (may be empty).
    pub playlist: songs::Playlist,
    /// Where the daily practice log lives.
    pub practice_log: RecordStore,
    /// Start with falling bars or in sight-reading mode.
    pub view: falling::View,
}

const HEADLESS_FRAME: Duration = Duration::from_micros(16_667);

fn default_plugins(backend: &Backend, logging: Logging, title: &str) -> PluginGroupBuilder {
    let plugins = match backend {
        Backend::Windowed => DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("keybard — {title}"),
                resolution: WindowResolution::new(
                    layout::CANVAS_WIDTH as u32,
                    layout::CANVAS_HEIGHT as u32,
                ),
                ..default()
            }),
            ..default()
        }),
        Backend::Headless => DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                render_creation: WgpuSettings {
                    backends: None,
                    ..default()
                }
                .into(),
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<AudioPlugin>()
            .add(ScheduleRunnerPlugin::run_loop(HEADLESS_FRAME)),
        Backend::Offscreen(_) => DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<AudioPlugin>()
            .add(ScheduleRunnerPlugin::run_loop(Duration::ZERO)),
    };
    match logging {
        Logging::Enabled => plugins,
        Logging::Disabled => plugins.disable::<LogPlugin>(),
    }
}

/// Assemble the app without running it.
pub fn build(options: Options) -> App {
    let (low, high) = options.song.pitch_range();
    let keys = KeyRange::covering(low, high);
    let mut performance = Performance::new(&options.song);
    performance.set_hands(options.hands);
    let mut transport = Transport::new(options.mode, options.song.tempo);
    let looping = options.loop_from.map(|bar| {
        let looping = section::Looping::from_bar(options.song.tempo, bar, options.song.duration());
        transport.time = looping.region.start + transport.start;
        performance.seek(&options.song, transport.time);
        looping
    });

    let mut app = App::new();
    app.add_plugins(default_plugins(
        &options.backend,
        options.logging,
        &options.song.title,
    ));
    info!(
        title = %options.song.title,
        notes = options.song.notes().len(),
        low = %keys.low(),
        high = %keys.high(),
        backend = ?options.backend,
        "built keybard"
    );
    app.insert_resource(ClearColor(palette::BACKGROUND))
        .insert_resource(Keys(keys))
        .insert_resource(Judge(performance))
        .insert_resource(transport)
        .insert_resource(section::SectionLoop(looping))
        .insert_resource(options.playlist)
        .insert_resource(options.view)
        .insert_resource(ActiveSong(options.song))
        .add_message::<PlayTone>()
        .add_message::<Feedback>()
        .configure_sets(
            Update,
            (Phase::Input, Phase::Simulate, Phase::Present).chain(),
        )
        .add_systems(Startup, spawn_camera)
        .add_plugins((
            textures::plugin,
            effects::plugin,
            results::plugin(options.records),
            transport::plugin,
            input::plugin(options.midi_input),
            piano::plugin,
            falling::plugin,
            notation::plugin,
            hud::plugin,
            section::plugin,
            songs::plugin,
            practice::plugin(options.practice_log),
            heat::plugin,
        ));
    if options.player == Player::Autopilot {
        app.add_plugins(autopilot::plugin);
    }
    match options.backend {
        Backend::Windowed => {
            app.add_plugins(audio::plugin);
        }
        Backend::Headless => {
            app.add_systems(Update, exit_when_finished.after(Phase::Present));
            // Headless worlds are tiny: thread hand-offs cost more than they
            // save, and many apps stepped in parallel (tests) fight over the
            // shared task pools.
            app.world_mut()
                .resource_mut::<Schedules>()
                .iter_mut()
                .for_each(|(_, schedule)| {
                    schedule.set_executor(SingleThreadedExecutor::new());
                });
        }
        Backend::Offscreen(plan) => {
            app.insert_resource(TimeUpdateStrategy::ManualDuration(HEADLESS_FRAME))
                .add_plugins(snapshot::plugin(plan));
        }
    }
    app
}

pub fn run(options: Options) -> AppExit {
    build(options).run()
}

/// Headless runs have no window to close, so stop once the song is over.
fn exit_when_finished(
    transport: Res<Transport>,
    judge: Res<Judge>,
    mut exit: MessageWriter<AppExit>,
) {
    if transport.state == PlayState::Finished {
        let score = judge.0.score();
        info!(?score, "headless run finished");
        exit.write(AppExit::Success);
    }
}

fn spawn_camera(mut commands: Commands, offscreen: Option<Res<snapshot::OffscreenTarget>>) {
    let mut camera = commands.spawn((
        Camera2d,
        // UI must follow this camera even when it renders offscreen.
        IsDefaultUiCamera,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: layout::CANVAS_WIDTH,
                min_height: layout::CANVAS_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
    if let Some(target) = offscreen {
        camera.insert(snapshot::render_target(&target));
    }
}
