//! Bevy application: wires the pure song/performance model to rendering,
//! input and audio.
//!
//! [`build`] produces an [`App`] for either a real window or a headless
//! backend (no window, no GPU, no audio device). The headless app can be
//! stepped frame-by-frame with [`App::update`] and inspected through the
//! public resources and components below, which is how the integration tests
//! drive it.

mod audio;
pub mod falling;
mod hud;
pub mod input;
pub mod notation;
pub mod palette;
pub mod piano;
pub mod transport;

use std::time::Duration;

use bevy::app::{PluginGroupBuilder, ScheduleRunnerPlugin};
use bevy::audio::AudioPlugin;
use bevy::camera::ScalingMode;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::WgpuSettings;
use bevy::window::{ExitCondition, WindowResolution};
use bevy::winit::WinitPlugin;
use tracing::info;

pub use audio::PlayTone;

use crate::layout::{self, KeyRange};
use crate::performance::{Performance, PlayMode};
use crate::song::Song;
use transport::{PlayState, Transport};

/// The song being played.
#[derive(Resource)]
pub struct ActiveSong(pub Song);

/// Judgement state for the active song.
#[derive(Resource)]
pub struct Judge(pub Performance);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// A real window with rendering and sound.
    Windowed,
    /// No window, GPU or audio device. Scene entities are still spawned and
    /// updated, so their state can be inspected.
    Headless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Logging {
    Enabled,
    /// Needed when building several apps in one process (e.g. tests), since
    /// the global tracing subscriber can only be installed once.
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiInput {
    Enabled,
    Disabled,
}

pub struct Options {
    pub song: Song,
    pub mode: PlayMode,
    pub midi_input: MidiInput,
    pub backend: Backend,
    pub logging: Logging,
}

const HEADLESS_FRAME: Duration = Duration::from_micros(16_667);

fn default_plugins(backend: Backend, logging: Logging, title: &str) -> PluginGroupBuilder {
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
    let performance = Performance::new(&options.song);

    let mut app = App::new();
    app.add_plugins(default_plugins(
        options.backend,
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
        .insert_resource(Transport::new(options.mode))
        .insert_resource(ActiveSong(options.song))
        .add_message::<PlayTone>()
        .configure_sets(
            Update,
            (Phase::Input, Phase::Simulate, Phase::Present).chain(),
        )
        .add_systems(Startup, spawn_camera)
        .add_plugins((
            transport::plugin,
            input::plugin(options.midi_input),
            piano::plugin,
            falling::plugin,
            notation::plugin,
            hud::plugin,
        ));
    match options.backend {
        Backend::Windowed => {
            app.add_plugins(audio::plugin);
        }
        Backend::Headless => {
            app.add_systems(Update, exit_when_finished.after(Phase::Present));
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

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: layout::CANVAS_WIDTH,
                min_height: layout::CANVAS_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}
