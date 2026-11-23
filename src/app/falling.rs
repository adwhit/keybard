//! Falling bars between the notation and the keyboard.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use super::transport::Transport;
use super::{ActiveSong, Judge, Keys, Phase, palette};
use crate::layout;
use crate::music::{KeyColor, PitchClass};
use crate::performance::NoteStatus;
use crate::song::{NoteId, Seconds};

#[derive(Component, Debug, Clone, Copy)]
pub struct FallingBar(pub NoteId);

const BAR_GAP: f32 = 2.0;
const MIN_BAR_HEIGHT: f32 = 4.0;
const WHITE_BAR_Z: f32 = 1.0;
const BLACK_BAR_Z: f32 = 1.5;
/// Opaque cover for the notation panel, hiding bars before they "enter".
const PANEL_Z: f32 = 5.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_bars)
        .add_systems(Update, (move_bars, draw_guides).in_set(Phase::Present));
}

fn spawn_bars(mut commands: Commands, song: Res<ActiveSong>, keys: Res<Keys>) {
    commands.spawn((
        Sprite::from_color(
            palette::PAPER,
            Vec2::new(layout::CANVAS_WIDTH, layout::TOP - layout::NOTATION_BOTTOM),
        ),
        Anchor::BOTTOM_CENTER,
        Transform::from_xyz(0.0, layout::NOTATION_BOTTOM, PANEL_Z),
    ));
    let bars = song
        .0
        .iter()
        .filter_map(|(id, note)| {
            let geometry = keys.0.geometry(note.pitch)?;
            let z = match geometry.color {
                KeyColor::White => WHITE_BAR_Z,
                KeyColor::Black => BLACK_BAR_Z,
            };
            let height =
                (note.duration.0 * layout::FALL_PIXELS_PER_SECOND - BAR_GAP).max(MIN_BAR_HEIGHT);
            Some((
                FallingBar(id),
                Sprite::from_color(
                    palette::hand(note.hand),
                    Vec2::new(geometry.width - 2.0 * BAR_GAP, height),
                ),
                Anchor::BOTTOM_CENTER,
                Transform::from_xyz(geometry.center_x, layout::TOP, z),
                Visibility::Hidden,
            ))
        })
        .collect::<Vec<_>>();
    commands.spawn_batch(bars);
}

fn move_bars(
    mut bars: Query<(&FallingBar, &mut Transform, &mut Sprite, &mut Visibility)>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    judge: Res<Judge>,
) {
    let lookahead = layout::fall_lookahead();
    let now = transport.time;
    bars.iter_mut()
        .for_each(|(bar, mut transform, mut sprite, mut visibility)| {
            let note = song.0.note(bar.0);
            let on_screen = note.start < now + lookahead && note.end() > now;
            *visibility = if on_screen {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if !on_screen {
                return;
            }
            transform.translation.y = layout::fall_y(note.start - now);
            sprite.color = match judge.0.status(bar.0) {
                NoteStatus::Hit => palette::hand_played(note.hand),
                NoteStatus::Missed => palette::MISSED,
                NoteStatus::Pending | NoteStatus::Skipped => palette::hand(note.hand),
            };
        });
}

/// Faint vertical lines at every C and F to help track columns, plus a
/// horizontal line on each bar line of the music.
fn draw_guides(
    mut gizmos: Gizmos,
    keys: Res<Keys>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
) {
    let range = keys.0;
    let half_white = range.white_key_width() / 2.0;
    range
        .notes()
        .filter(|n| matches!(n.pitch_class(), PitchClass::C | PitchClass::F))
        .filter_map(|n| range.geometry(n))
        .map(|g| g.center_x - half_white)
        .for_each(|x| {
            gizmos.line_2d(
                Vec2::new(x, layout::HIT_LINE),
                Vec2::new(x, layout::NOTATION_BOTTOM),
                palette::GUIDE,
            );
        });

    let bar = song.0.tempo.bar_length();
    let now = transport.time;
    let first = (now.0 / bar.0).ceil().max(0.0) as u32;
    (first..)
        .map(|i| Seconds(i as f32 * bar.0))
        .take_while(|t| *t < now + layout::fall_lookahead())
        .for_each(|t| {
            let y = layout::fall_y(t - now);
            gizmos.line_2d(
                Vec2::new(layout::LEFT, y),
                Vec2::new(layout::RIGHT, y),
                palette::GUIDE,
            );
        });
}
