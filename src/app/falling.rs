//! Falling bars between the notation and the keyboard.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use super::songs::{SongLoad, SongScoped};
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

/// The note-name label on a bar. `stacked` labels sit above the finger
/// number; short bars only have room for one of the two.
#[derive(Component, Debug, Clone, Copy)]
pub struct BarLabel {
    stacked: bool,
}

/// Marks a bar whose note moves the hand to a new position.
#[derive(Component, Debug, Clone, Copy)]
pub struct ShiftMarker;

/// The recommended-finger number on a bar.
#[derive(Component, Debug, Clone, Copy)]
pub struct BarFinger;

/// Whether the falling bars guide you, or you read from the staff alone (F6).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Bars,
    /// Bars and keyboard cues hidden: play from the notation.
    SightReading,
}

/// What the bars are labelled with. F1 cycles through these.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarLabels {
    Both,
    Fingers,
    Names,
    Off,
}

impl BarLabels {
    fn next(self) -> Self {
        match self {
            Self::Both => Self::Fingers,
            Self::Fingers => Self::Names,
            Self::Names => Self::Off,
            Self::Off => Self::Both,
        }
    }

    fn shows_fingers(self) -> bool {
        matches!(self, Self::Both | Self::Fingers)
    }

    fn shows_name(self, label: BarLabel) -> bool {
        match self {
            Self::Names => true,
            Self::Both => label.stacked,
            Self::Fingers | Self::Off => false,
        }
    }
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
/// Seconds over which bars brighten as they approach the hit line.
const APPROACH_FADE: f32 = 0.6;
/// Opacity of bars the computer plays for you.
pub const GHOST_ALPHA: f32 = 0.3;
const MIN_BAR_HEIGHT: f32 = 14.0;
/// Labels need this much bar to sit inside.
const MIN_LABELLED_HEIGHT: f32 = 18.0;
/// Bars this tall fit both the finger number and the note name.
const MIN_STACKED_HEIGHT: f32 = 34.0;
const BACKDROP_Z: f32 = 0.0;
const LANE_Z: f32 = 0.1;
const GUIDE_Z: f32 = 0.2;
const WHITE_BAR_Z: f32 = 1.0;
const BLACK_BAR_Z: f32 = 1.5;
/// Opaque cover for the notation panel, hiding bars before they "enter".
const PANEL_Z: f32 = 5.0;

pub fn plugin(app: &mut App) {
    app.insert_resource(BarLabels::Both)
        .init_resource::<View>()
        .add_systems(Update, toggle_view.in_set(Phase::Input))
        .add_systems(Startup, spawn_backdrop)
        .add_systems(Update, (spawn_lanes, spawn_bars).in_set(SongLoad))
        .add_systems(Update, cycle_labels.in_set(Phase::Input))
        .add_systems(
            Update,
            (move_bars, show_labels, draw_bar_lines).in_set(Phase::Present),
        );
}

fn spawn_backdrop(mut commands: Commands, textures: Res<Textures>) {
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
}

fn spawn_lanes(mut commands: Commands, keys: Res<Keys>) {
    let fall_height = layout::NOTATION_BOTTOM - layout::HIT_LINE;
    let range = keys.0;
    let half_white = range.white_key_width() / 2.0;
    let lanes = range
        .notes()
        .filter_map(|n| range.geometry(n).map(|g| (n, g)))
        .filter_map(|(note, g)| match g.color {
            KeyColor::Black => Some((
                SongScoped,
                Sprite::from_color(palette::BLACK_LANE, Vec2::new(g.width, fall_height)),
                Anchor::BOTTOM_CENTER,
                Transform::from_xyz(g.center_x, layout::HIT_LINE, LANE_Z),
            )),
            // Thin separators at every C and F, where there's no black key.
            KeyColor::White if matches!(note.pitch_class(), PitchClass::C | PitchClass::F) => {
                Some((
                    SongScoped,
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
            SongScoped,
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
        let shift = song.0.starts_new_position(id);
        if shift {
            // A bright strip along the edge you press: "move your hand here".
            bar.with_child((
                ShiftMarker,
                Sprite::from_color(
                    palette::SHIFT,
                    Vec2::new(geometry.width - 2.0 * BAR_GAP, 4.0),
                ),
                Anchor::BOTTOM_CENTER,
                Transform::from_xyz(0.0, 0.0, 0.04),
            ));
        }
        if height >= MIN_LABELLED_HEIGHT {
            let stacked = height >= MIN_STACKED_HEIGHT;
            bar.with_child((
                BarFinger,
                Text2d::new(song.0.finger(id).number().to_string()),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(if shift {
                    palette::SHIFT_INK
                } else {
                    Color::srgba(0.0, 0.0, 0.0, 0.75)
                }),
                Transform::from_xyz(0.0, 12.0, 0.05),
            ));
            bar.with_child((
                BarLabel { stacked },
                Text2d::new(note.pitch.pitch_class().to_string()),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                Transform::from_xyz(0.0, if stacked { 26.0 } else { 10.0 }, 0.05),
            ));
        }
    });
}

fn toggle_view(keys: Res<ButtonInput<KeyCode>>, mut view: ResMut<View>) {
    if keys.just_pressed(KeyCode::F6) {
        *view = match *view {
            View::Bars => View::SightReading,
            View::SightReading => View::Bars,
        };
    }
}

fn cycle_labels(keys: Res<ButtonInput<KeyCode>>, mut labels: ResMut<BarLabels>) {
    if keys.just_pressed(KeyCode::F1) {
        *labels = labels.next();
    }
}

fn show_labels(
    labels: Res<BarLabels>,
    mut names: Query<(&BarLabel, &mut Visibility), Without<BarFinger>>,
    mut fingers: Query<&mut Visibility, With<BarFinger>>,
) {
    let visible = |shown: bool| {
        if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    // Runs every frame so newly spawned bars (song switches) follow suit.
    names.iter_mut().for_each(|(label, mut v)| {
        v.set_if_neq(visible(labels.shows_name(*label)));
    });
    fingers.iter_mut().for_each(|mut v| {
        v.set_if_neq(visible(labels.shows_fingers()));
    });
}

fn move_bars(
    mut bars: Query<(&FallingBar, &mut Transform, &mut Sprite, &mut Visibility)>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    judge: Res<Judge>,
    view: Res<View>,
) {
    let lookahead = layout::fall_lookahead();
    let now = transport.time;
    bars.iter_mut()
        .for_each(|(bar, mut transform, mut sprite, mut visibility)| {
            let note = song.0.note(bar.0);
            let on_screen = *view == View::Bars && note.start < now + lookahead && note.end() > now;
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
            // Bars warm towards their lit colour over the last stretch.
            let approach = 1.0 - ((note.start - now).0 / APPROACH_FADE).clamp(0.0, 1.0);
            let color = match state {
                BarState::Upcoming => palette::by_dynamics(
                    palette::hand(note.hand).mix(&palette::hand_lit(note.hand), 0.6 * approach),
                    note.velocity,
                ),
                BarState::Sounding => {
                    palette::by_dynamics(palette::hand_lit(note.hand), note.velocity)
                }
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
