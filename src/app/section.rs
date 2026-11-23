//! Looping a few bars for focused practice (F3).

use bevy::prelude::*;
use bevy::sprite::Anchor;
use tracing::info;

use super::effects::popup;
use super::results::LastResult;
use super::transport::{PlayState, Speed, Transport, advance};
use super::{ActiveSong, Judge, Phase, palette};
use crate::layout;
use crate::performance::{Score, Timing};
use crate::practice::{DRILL_START_SPEED, LoopRegion, trainer_speed};
use crate::song::{Seconds, Tempo};

/// How many bars F3 loops.
pub const LOOP_BARS: u8 = 4;

/// How one pass through the loop went (scored modes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassResult {
    pub hits: usize,
    pub missed: usize,
    pub wrong: usize,
}

impl PassResult {
    fn between(before: &Score, after: &Score) -> Self {
        Self {
            hits: after.hits() - before.hits(),
            missed: after.missed - before.missed,
            wrong: after.wrong - before.wrong,
        }
    }

    pub fn is_clean(self) -> bool {
        self.missed == 0 && self.wrong == 0 && self.hits > 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Looping {
    pub region: LoopRegion,
    /// Completed passes.
    pub passes: u32,
    /// Passes with no misses or wrong notes.
    pub clean_passes: u32,
    pub last_pass: Option<PassResult>,
    /// Score when the current pass started, to measure it.
    baseline: Score,
}

impl Looping {
    pub fn new(region: LoopRegion) -> Self {
        Self {
            region,
            passes: 0,
            clean_passes: 0,
            last_pass: None,
            baseline: Score::default(),
        }
    }

    /// Loop `LOOP_BARS` bars starting at 1-based `bar`.
    pub fn from_bar(tempo: Tempo, bar: u32, song_end: Seconds) -> Self {
        let time = Seconds(bar.saturating_sub(1) as f32 * tempo.bar_length().0);
        Self::new(LoopRegion::around(tempo, time, LOOP_BARS, song_end))
    }
}

#[derive(Resource, Debug, Default)]
pub struct SectionLoop(pub Option<Looping>);

/// Highlight of the looped section on the progress strip.
#[derive(Component)]
struct LoopHighlight;

pub fn plugin(app: &mut App) {
    app.init_resource::<SectionLoop>()
        .add_systems(Startup, spawn_highlight)
        .add_systems(
            Update,
            (
                toggle.in_set(Phase::Simulate).before(advance),
                wrap.in_set(Phase::Simulate).after(advance),
                show_highlight.in_set(Phase::Present),
            ),
        );
}

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut section: ResMut<SectionLoop>,
    mut transport: ResMut<Transport>,
    song: Res<ActiveSong>,
    mut judge: ResMut<Judge>,
    result: Option<Res<LastResult>>,
) {
    if !keys.just_pressed(KeyCode::F3) {
        return;
    }
    // From the results card, F3 drills the weakest stretch.
    let weakest = result
        .filter(|_| transport.state == PlayState::Finished)
        .and_then(|r| r.weakest);
    if let Some(bar) = weakest {
        let looping = Looping::from_bar(song.0.tempo, bar, song.0.duration());
        judge.0.reset(&song.0);
        transport.time = looping.region.start + transport.start;
        judge.0.seek(&song.0, transport.time);
        transport.state = PlayState::Playing;
        transport.speed = Speed::new(DRILL_START_SPEED);
        info!(bar, "drilling the weakest section");
        section.0 = Some(looping);
        return;
    }
    section.0 = match section.0 {
        Some(_) => {
            info!("loop cleared");
            None
        }
        None => {
            let region =
                LoopRegion::around(song.0.tempo, transport.time, LOOP_BARS, song.0.duration());
            info!(?region, "looping section");
            Some(Looping {
                baseline: judge.0.score().clone(),
                ..Looping::new(region)
            })
        }
    };
}

fn wrap(
    mut commands: Commands,
    mut section: ResMut<SectionLoop>,
    mut transport: ResMut<Transport>,
    song: Res<ActiveSong>,
    mut judge: ResMut<Judge>,
) {
    let Some(looping) = section.0.as_mut() else {
        return;
    };
    // Rewind as far as a normal start, so the notes fall into view again.
    let lead = Seconds(-transport.start.0);
    let Some(rewind) = looping.region.wrap(transport.time, lead) else {
        return;
    };
    let score = judge.0.score().clone();
    let pass = PassResult::between(&looping.baseline, &score);
    looping.passes += 1;
    looping.clean_passes += u32::from(pass.is_clean());
    looping.last_pass = Some(pass);
    looping.baseline = score;
    if transport.mode.is_scored() {
        let speed = trainer_speed(transport.speed.get(), pass.is_clean());
        if speed != transport.speed.get() {
            info!(speed, "speed trainer stepped up");
            popup(
                &mut commands,
                &format!("SPEED {:.0}%", speed * 100.0),
                palette::timing(Timing::Great),
                Vec2::new(0.0, 60.0),
                30.0,
                1.0,
            );
        }
        transport.speed = Speed::new(speed);
    }
    transport.time = rewind;
    judge.0.seek(&song.0, rewind);
    info!(passes = looping.passes, ?pass, "loop pass complete");
}

const STRIP_Z: f32 = 6.15;

fn spawn_highlight(mut commands: Commands) {
    commands.spawn((
        LoopHighlight,
        Sprite::from_color(palette::LOOP_REGION, Vec2::new(1.0, 5.0)),
        Anchor::TOP_LEFT,
        Transform::from_xyz(layout::LEFT, layout::NOTATION_BOTTOM, STRIP_Z),
        Visibility::Hidden,
    ));
}

fn show_highlight(
    section: Res<SectionLoop>,
    song: Res<ActiveSong>,
    mut highlight: Single<(&mut Transform, &mut Sprite, &mut Visibility), With<LoopHighlight>>,
) {
    let (transform, sprite, visibility) = &mut *highlight;
    match &section.0 {
        Some(looping) => {
            let duration = song.0.duration().0;
            let x = |t: Seconds| layout::LEFT + t.0 / duration * layout::CANVAS_WIDTH;
            transform.translation.x = x(looping.region.start);
            sprite.custom_size = Some(Vec2::new(
                x(looping.region.end) - x(looping.region.start),
                5.0,
            ));
            **visibility = Visibility::Inherited;
        }
        None => **visibility = Visibility::Hidden,
    }
}
