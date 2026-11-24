//! Offscreen rendering: draw into a GPU image instead of a window and save
//! PNGs at chosen song times. Used to review visuals without opening a window.

use std::path::PathBuf;

use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use tracing::{info, warn};

use super::Phase;
use super::transport::{PlayState, Transport};
use crate::layout;
use crate::song::Seconds;

/// Where and when to capture frames.
#[derive(Debug, Clone)]
pub struct SnapshotPlan {
    pub dir: PathBuf,
    /// Song times to capture, in any order.
    pub times: Vec<Seconds>,
    /// Output resolution scale relative to the canvas (2.0 = retina-like).
    pub scale: f32,
}

/// The image the camera renders into.
#[derive(Resource, Clone)]
pub struct OffscreenTarget(pub Handle<Image>);

/// Frames to draw before any capture, so layout and text are ready (the
/// clock may never move, e.g. on the start screen).
const WARM_UP_FRAMES: u32 = 5;
/// Give up waiting for captures to reach disk after this many frames.
const FRAMES_TO_FLUSH: u32 = 600;
/// Backstop so a snapshot run can never spin forever (10 minutes of song).
const MAX_FRAMES: u32 = 60 * 60 * 10;

impl SnapshotPlan {
    /// Whether a capture planned for `at` should happen now. Times the clock
    /// can never reach (past the end of the song) are taken once it finishes.
    pub fn is_due(at: Seconds, now: Seconds, finished: bool) -> bool {
        finished || now >= at
    }
}

#[derive(Resource, Debug)]
struct Pending {
    dir: PathBuf,
    /// Sorted descending so the next capture is `last()`.
    times: Vec<Seconds>,
    /// Files captured so far, to wait for before exiting.
    saved: Vec<PathBuf>,
    /// Frames spent waiting for the files once every capture is taken.
    flushing: Option<u32>,
}

pub fn plugin(plan: SnapshotPlan) -> impl Fn(&mut App) {
    move |app: &mut App| {
        let mut times = plan.times.clone();
        times.sort_by(|a, b| b.0.total_cmp(&a.0));
        app.insert_resource(Pending {
            dir: plan.dir.clone(),
            times,
            saved: Vec::new(),
            flushing: None,
        })
        .add_systems(Update, capture.after(Phase::Present));
        let scale = plan.scale;
        app.add_systems(
            PreStartup,
            move |mut commands: Commands, mut images: ResMut<Assets<Image>>| {
                commands.insert_resource(create_target(&mut images, scale));
            },
        );
    }
}

fn create_target(images: &mut Assets<Image>, scale: f32) -> OffscreenTarget {
    let mut image = Image::new_target_texture(
        (layout::CANVAS_WIDTH * scale) as u32,
        (layout::CANVAS_HEIGHT * scale) as u32,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    OffscreenTarget(images.add(image))
}

pub fn render_target(target: &OffscreenTarget) -> RenderTarget {
    RenderTarget::Image(target.0.clone().into())
}

fn capture(
    mut pending: ResMut<Pending>,
    transport: Res<Transport>,
    target: Res<OffscreenTarget>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut frames: Local<u32>,
) {
    *frames += 1;
    if *frames > MAX_FRAMES {
        warn!(
            remaining = pending.times.len(),
            "snapshot run hit its frame cap; exiting"
        );
        exit.write(AppExit::error());
        return;
    }
    if let Some(waited) = pending.flushing {
        // GPU readback can take many (fast, offscreen) frames, more so for
        // big images: exit once every file is on disk.
        if pending.saved.iter().all(|p| p.exists()) {
            exit.write(AppExit::Success);
        } else if waited >= FRAMES_TO_FLUSH {
            warn!("snapshots did not reach disk in time; exiting");
            exit.write(AppExit::error());
        } else {
            pending.flushing = Some(waited + 1);
        }
        return;
    }
    if *frames <= WARM_UP_FRAMES {
        return;
    }
    let finished = transport.state == PlayState::Finished;
    let due = pending
        .times
        .last()
        .is_some_and(|at| SnapshotPlan::is_due(*at, transport.time, finished));
    if !due {
        return;
    }
    let at = pending.times.pop().unwrap_or(transport.time);
    let path = pending.dir.join(format!("t{:06.2}.png", at.0.max(0.0)));
    info!(path = %path.display(), time = transport.time.0, "capturing snapshot");
    commands
        .spawn(Screenshot::image(target.0.clone()))
        .observe(save_to_disk(path.clone()));
    pending.saved.push(path);
    if pending.times.is_empty() {
        pending.flushing = Some(0);
    }
}
