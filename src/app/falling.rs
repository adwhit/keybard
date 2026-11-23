//! Falling bars between the notation and the keyboard.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use super::textures::Textures;
use super::transport::Transport;
use super::{ActiveSong, Judge, Keys, Phase, palette};
use crate::layout;
use crate::music::{KeyColor, PitchClass};
use crate::performance::NoteStatus;
use crate::song::{NoteId, Seconds};

#[derive(Component, Debug, Clone, Copy)]
pub struct FallingBar(pub NoteId);

/// The glow behind the falling bars.
#[derive(Component, Debug, Clone, Copy)]
pub struct Backdrop;

/// The note-name label on a bar.
#[derive(Component, Debug, Clone, Copy)]
pub struct BarLabel;

/// Whether bars show their note names. Toggled with F1.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteNames {
    Shown,
    Hidden,
}

/// What a bar looks like right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarState {
    Upcoming,
    Sounding,
    Played,
    Missed,
}

impl BarState {
    pub fn of(status: NoteStatus, sounding: bool) -> Self {
        match (status, sounding) {
            (NoteStatus::Missed, _) => Self::Missed,
            (_, true) => Self::Sounding,
            (NoteStatus::Pending, false) => Self::Upcoming,
            (NoteStatus::Hit(_) | NoteStatus::AutoPlayed | NoteStatus::Skipped, false) => {
                Self::Played
            }
        }
    }
}

const BAR_GAP: f32 = 2.0;
/// Opacity of bars the computer plays for you.
pub const GHOST_ALPHA: f32 = 0.3;
const MIN_BAR_HEIGHT: f32 = 14.0;
/// Labels need this much bar to sit inside.
const MIN_LABELLED_HEIGHT: f32 = 18.0;
const BACKDROP_Z: f32 = 0.0;
const LANE_Z: f32 = 0.1;
const GUIDE_Z: f32 = 0.2;
const WHITE_BAR_Z: f32 = 1.0;
const BLACK_BAR_Z: f32 = 1.5;
/// Opaque cover for the notation panel, hiding bars before they "enter".
const PANEL_Z: f32 = 5.0;

pub fn plugin(app: &mut App) {
    app.insert_resource(NoteNames::Shown)
        .add_systems(Startup, (spawn_backdrop, spawn_bars))
        .add_systems(Update, toggle_note_names.in_set(Phase::Input))
        .add_systems(
            Update,
            (move_bars, show_labels, draw_bar_lines).in_set(Phase::Present),
        );
}

fn spawn_backdrop(mut commands: Commands, keys: Res<Keys>, textures: Res<Textures>) {
    let fall_height = layout::NOTATION_BOTTOM - layout::HIT_LINE;
    commands.spawn((
        Sprite::from_color(
            palette::PAPER,
            Vec2::new(layout::CANVAS_WIDTH, layout::TOP - layout::NOTATION_BOTTOM),
        ),
        Anchor::BOTTOM_CENTER,
        Transform::from_xyz(0.0, layout::NOTATION_BOTTOM, PANEL_Z),
    ));
    // A soft glow rising from the keyboard; its colour tracks the streak.
    commands.spawn((
        Backdrop,
        Sprite {
            image: textures.fade_up.clone(),
            color: palette::BACKDROP_GLOW,
            custom_size: Some(Vec2::new(layout::CANVAS_WIDTH, fall_height)),
            ..default()
        },
        Anchor::BOTTOM_CENTER,
        Transform::from_xyz(0.0, layout::HIT_LINE, BACKDROP_Z),
    ));

    let range = keys.0;
    let half_white = range.white_key_width() / 2.0;
    let lanes = range
        .notes()
        .filter_map(|n| range.geometry(n).map(|g| (n, g)))
        .filter_map(|(note, g)| match g.color {
            KeyColor::Black => Some((
                Sprite::from_color(palette::BLACK_LANE, Vec2::new(g.width, fall_height)),
                Anchor::BOTTOM_CENTER,
                Transform::from_xyz(g.center_x, layout::HIT_LINE, LANE_Z),
            )),
            // Thin separators at every C and F, where there's no black key.
            KeyColor::White if matches!(note.pitch_class(), PitchClass::C | PitchClass::F) => {
                Some((
                    Sprite::from_color(palette::GUIDE, Vec2::new(1.0, fall_height)),
                    Anchor::BOTTOM_CENTER,
                    Transform::from_xyz(g.center_x - half_white, layout::HIT_LINE, GUIDE_Z),
                ))
            }
            KeyColor::White => None,
        })
        .collect::<Vec<_>>();
    commands.spawn_batch(lanes);
}

fn spawn_bars(
    mut commands: Commands,
    song: Res<ActiveSong>,
    keys: Res<Keys>,
    textures: Res<Textures>,
) {
    song.0.iter().for_each(|(id, note)| {
        let Some(geometry) = keys.0.geometry(note.pitch) else {
            return;
        };
        let z = match geometry.color {
            KeyColor::White => WHITE_BAR_Z,
            KeyColor::Black => BLACK_BAR_Z,
        };
        let height =
            (note.duration.0 * layout::FALL_PIXELS_PER_SECOND - BAR_GAP).max(MIN_BAR_HEIGHT);
        let mut bar = commands.spawn((
            FallingBar(id),
            Sprite {
                image: textures.rounded.clone(),
                color: palette::hand(note.hand),
                custom_size: Some(Vec2::new(geometry.width - 2.0 * BAR_GAP, height)),
                image_mode: Textures::rounded_mode(),
                ..default()
            },
            Anchor::BOTTOM_CENTER,
            Transform::from_xyz(geometry.center_x, layout::TOP, z),
            Visibility::Hidden,
        ));
        if height >= MIN_LABELLED_HEIGHT {
            bar.with_child((
                BarLabel,
                Text2d::new(note.pitch.pitch_class().to_string()),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                Transform::from_xyz(0.0, 9.0, 0.05),
            ));
        }
    });
}

fn toggle_note_names(keys: Res<ButtonInput<KeyCode>>, mut names: ResMut<NoteNames>) {
    if keys.just_pressed(KeyCode::F1) {
        *names = match *names {
            NoteNames::Shown => NoteNames::Hidden,
            NoteNames::Hidden => NoteNames::Shown,
        };
    }
}

fn show_labels(names: Res<NoteNames>, mut labels: Query<&mut Visibility, With<BarLabel>>) {
    if !names.is_changed() {
        return;
    }
    let visibility = match *names {
        NoteNames::Shown => Visibility::Inherited,
        NoteNames::Hidden => Visibility::Hidden,
    };
    labels.iter_mut().for_each(|mut v| *v = visibility);
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
            let state = BarState::of(judge.0.status(bar.0), note.is_sounding_at(now));
            let color = match state {
                BarState::Upcoming => palette::hand(note.hand),
                BarState::Sounding => palette::hand_lit(note.hand),
                BarState::Played => palette::hand_played(note.hand),
                BarState::Missed => palette::MISSED.darker(0.25),
            };
            // The computer's part is a translucent ghost when you practise
            // one hand, so your own notes stand out.
            let yours = !transport.mode.is_scored() || judge.0.is_players(&song.0, bar.0);
            sprite.color = if yours {
                color
            } else {
                color.with_alpha(GHOST_ALPHA)
            };
        });
}

/// A faint horizontal line on each bar line of the music.
fn draw_bar_lines(mut gizmos: Gizmos, transport: Res<Transport>, song: Res<ActiveSong>) {
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
                palette::BAR_LINE,
            );
        });
}
