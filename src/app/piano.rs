//! The on-screen piano keyboard at the bottom of the screen.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::sprite::Anchor;

use super::falling::View;
use super::heat::HeatGlow;
use super::input::HeldKeys;
use super::songs::{SongLoad, SongScoped};
use super::textures::Textures;
use super::transport::Transport;
use super::{ActiveSong, Judge, Keys, Phase, palette};
use crate::layout::{self, KeyGeometry};
use crate::music::{KeyColor, MidiNote, PitchClass};
use crate::performance::NoteStatus;
use crate::song::{Hand, Seconds};

/// The finger digit shown on a key while its note approaches or sounds.
#[derive(Component, Debug, Clone, Copy)]
pub struct KeyFinger;

#[derive(Component, Debug, Clone, Copy)]
pub struct PianoKey {
    pub note: MidiNote,
    pub color: KeyColor,
}

const WHITE_Z: f32 = 10.0;
const BLACK_Z: f32 = 11.0;
const KEY_GAP: f32 = 1.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_key_bed)
        .add_systems(Update, spawn_keys.in_set(SongLoad))
        .add_systems(Update, show_key_fingers.in_set(Phase::Present))
        .add_systems(Startup, spawn_hit_line)
        .add_systems(Update, (color_keys, pulse_hit_line).in_set(Phase::Present));
}

/// Backdrop behind the keys so gaps read as dark lines.
fn spawn_key_bed(mut commands: Commands) {
    commands.spawn((
        Sprite::from_color(
            palette::BACKGROUND,
            Vec2::new(layout::CANVAS_WIDTH, layout::WHITE_KEY_HEIGHT),
        ),
        Anchor::TOP_CENTER,
        Transform::from_xyz(0.0, layout::HIT_LINE, WHITE_Z - 0.5),
    ));
}

fn spawn_keys(mut commands: Commands, keys: Res<Keys>) {
    let range = keys.0;
    range
        .notes()
        .filter_map(|note| range.geometry(note).map(|g| (note, g)))
        .for_each(|(note, geometry)| spawn_key(&mut commands, note, geometry));
}

fn spawn_key(commands: &mut Commands, note: MidiNote, geometry: KeyGeometry) {
    let (height, z, color) = match geometry.color {
        KeyColor::White => (layout::WHITE_KEY_HEIGHT, WHITE_Z, palette::WHITE_KEY),
        KeyColor::Black => (layout::BLACK_KEY_HEIGHT, BLACK_Z, palette::BLACK_KEY),
    };
    let mut key = commands.spawn((
        SongScoped,
        PianoKey {
            note,
            color: geometry.color,
        },
        Sprite::from_color(color, Vec2::new(geometry.width - KEY_GAP, height)),
        Anchor::TOP_CENTER,
        Transform::from_xyz(geometry.center_x, layout::HIT_LINE, z),
    ));
    let (finger_y, finger_color) = match geometry.color {
        KeyColor::White => (-layout::BLACK_KEY_HEIGHT - 22.0, palette::INK),
        KeyColor::Black => (-height + 16.0, palette::WHITE_KEY),
    };
    key.with_child((
        KeyFinger,
        Text2d::new(""),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(finger_color),
        Transform::from_xyz(0.0, finger_y, 0.2),
    ));
    // Label every C so players can find their place.
    if note.pitch_class() == PitchClass::C {
        key.with_child((
            Text2d::new(note.to_string()),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(palette::KEY_LABEL),
            Transform::from_xyz(0.0, -height + 12.0, 0.1),
        ));
    }
}

/// How a key is lit this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyLight {
    Idle,
    /// The song is playing this note (a hint in Wait/Play, the performance in Listen).
    Cue(Hand),
    /// The player is holding it and it's a right note.
    Correct(Hand),
    /// The player is holding it but nothing is playing there.
    Stray,
    /// One of the player's notes arrives here within [`APPROACH`].
    Upcoming(Hand),
}

/// How far ahead keys start hinting at their next note.
pub const APPROACH: Seconds = Seconds(0.35);

impl KeyLight {
    fn of(held: bool, sounding: Option<Hand>, upcoming: Option<Hand>) -> Self {
        match (held, sounding, upcoming) {
            (true, Some(hand), _) => Self::Correct(hand),
            (true, None, _) => Self::Stray,
            (false, Some(hand), _) => Self::Cue(hand),
            (false, None, Some(hand)) => Self::Upcoming(hand),
            (false, None, None) => Self::Idle,
        }
    }

    fn color(self, key: KeyColor) -> Color {
        match (self, key) {
            (Self::Idle, KeyColor::White) => palette::WHITE_KEY,
            (Self::Idle, KeyColor::Black) => palette::BLACK_KEY,
            (Self::Cue(hand), _) => palette::hand(hand),
            (Self::Correct(hand), _) => palette::hand_lit(hand),
            (Self::Stray, _) => palette::PRESSED,
            (Self::Upcoming(hand), key) => palette::hand_hint(hand, key),
        }
    }

    fn is_pressed(self) -> bool {
        matches!(self, Self::Correct(_) | Self::Stray)
    }
}

/// Pressed keys sink by this much.
const KEY_TRAVEL: f32 = 4.0;

fn color_keys(
    mut keys: Query<(&PianoKey, &mut Sprite, &mut Transform)>,
    held: Res<HeldKeys>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    judge: Res<Judge>,
    view: Res<View>,
) {
    let now = transport.time;
    let upcoming: HashMap<MidiNote, Hand> = song
        .0
        .iter()
        .filter(|(id, n)| {
            n.start > now
                && n.start <= now + APPROACH
                && judge.0.is_players(&song.0, *id)
                && judge.0.status(*id) == NoteStatus::Pending
        })
        .map(|(_, n)| (n.pitch, n.hand))
        .collect();
    let sounding: HashMap<MidiNote, Hand> = song
        .0
        .notes()
        .iter()
        .filter(|n| n.is_sounding_at(transport.time))
        .map(|n| (n.pitch, n.hand))
        .collect();
    keys.iter_mut()
        .for_each(|(key, mut sprite, mut transform)| {
            let light = match KeyLight::of(
                held.0.contains(&key.note),
                sounding.get(&key.note).copied(),
                upcoming.get(&key.note).copied(),
            ) {
                // Sight-reading: the keyboard mustn't give the answer away.
                KeyLight::Cue(_) | KeyLight::Upcoming(_) if *view == View::SightReading => {
                    KeyLight::Idle
                }
                light => light,
            };
            sprite.color = light.color(key.color);
            // Pressed keys sink, opening a dark sliver above them.
            transform.translation.y = if light.is_pressed() {
                layout::HIT_LINE - KEY_TRAVEL
            } else {
                layout::HIT_LINE
            };
        });
}

#[derive(Component)]
struct HitLine;

fn spawn_hit_line(mut commands: Commands, textures: Res<Textures>) {
    // Soft glow band plus a crisp core line.
    commands.spawn((
        HitLine,
        Sprite {
            image: textures.glow.clone(),
            color: palette::HIT_LINE.with_alpha(0.35),
            custom_size: Some(Vec2::new(layout::CANVAS_WIDTH * 1.4, 26.0)),
            ..default()
        },
        Transform::from_xyz(0.0, layout::HIT_LINE, 11.5),
    ));
    commands.spawn((
        Sprite::from_color(palette::HIT_LINE, Vec2::new(layout::CANVAS_WIDTH, 2.0)),
        Transform::from_xyz(0.0, layout::HIT_LINE + 1.0, 11.6),
    ));
}

/// The hit line breathes with the beat.
fn pulse_hit_line(
    mut line: Single<&mut Sprite, With<HitLine>>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    glow: Res<HeatGlow>,
) {
    let beat = transport.time.0 / song.0.tempo.seconds_per_beat.0;
    let phase = beat.rem_euclid(1.0);
    // Pulses harder and burns hotter as the streak heats up.
    let strength = (0.25 + 0.25 * glow.0) + (0.25 + 0.3 * glow.0) * (1.0 - phase).powi(3);
    line.color = palette::HIT_LINE
        .mix(&palette::FIRE, glow.0)
        .with_alpha(strength);
}

/// Shows which finger to use on keys whose (player's) note is about to land
/// or is sounding now.
fn show_key_fingers(
    keys: Query<(&PianoKey, &Children)>,
    mut digits: Query<&mut Text2d, With<KeyFinger>>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    judge: Res<Judge>,
    view: Res<View>,
) {
    let now = transport.time;
    if *view == View::SightReading {
        keys.iter().for_each(|(_, children)| {
            children.iter().for_each(|child| {
                if let Ok(mut digit) = digits.get_mut(child)
                    && !digit.0.is_empty()
                {
                    digit.0.clear();
                }
            });
        });
        return;
    }
    let wanted: HashMap<MidiNote, String> = song
        .0
        .iter()
        .filter(|(id, n)| {
            let upcoming = n.start > now
                && n.start <= now + APPROACH
                && judge.0.status(*id) == NoteStatus::Pending;
            (upcoming || n.is_sounding_at(now)) && judge.0.is_players(&song.0, *id)
        })
        .map(|(id, n)| (n.pitch, song.0.finger(id).number().to_string()))
        .collect();
    keys.iter().for_each(|(key, children)| {
        let text = wanted.get(&key.note).cloned().unwrap_or_default();
        children.iter().for_each(|child| {
            if let Ok(mut digit) = digits.get_mut(child)
                && digit.0 != text
            {
                digit.0.clone_from(&text);
            }
        });
    });
}
