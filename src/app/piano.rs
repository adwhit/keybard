//! The on-screen piano keyboard at the bottom of the screen.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::sprite::Anchor;

use super::input::HeldKeys;
use super::transport::Transport;
use super::{ActiveSong, Keys, Phase, palette};
use crate::layout::{self, KeyGeometry};
use crate::music::{KeyColor, MidiNote, PitchClass};
use crate::song::Hand;

#[derive(Component, Debug, Clone, Copy)]
pub struct PianoKey {
    pub note: MidiNote,
    pub color: KeyColor,
}

const WHITE_Z: f32 = 10.0;
const BLACK_Z: f32 = 11.0;
const KEY_GAP: f32 = 1.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_keys)
        .add_systems(Update, (color_keys, draw_hit_line).in_set(Phase::Present));
}

fn spawn_keys(mut commands: Commands, keys: Res<Keys>) {
    let range = keys.0;
    // Backdrop behind the keys so gaps read as dark lines.
    commands.spawn((
        Sprite::from_color(
            palette::BACKGROUND,
            Vec2::new(layout::CANVAS_WIDTH, layout::WHITE_KEY_HEIGHT),
        ),
        Anchor::TOP_CENTER,
        Transform::from_xyz(0.0, layout::HIT_LINE, WHITE_Z - 0.5),
    ));
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
        PianoKey {
            note,
            color: geometry.color,
        },
        Sprite::from_color(color, Vec2::new(geometry.width - KEY_GAP, height)),
        Anchor::TOP_CENTER,
        Transform::from_xyz(geometry.center_x, layout::HIT_LINE, z),
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

fn color_keys(
    mut keys: Query<(&PianoKey, &mut Sprite)>,
    held: Res<HeldKeys>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
) {
    let sounding: HashMap<MidiNote, Hand> = song
        .0
        .notes()
        .iter()
        .filter(|n| n.is_sounding_at(transport.time))
        .map(|n| (n.pitch, n.hand))
        .collect();
    keys.iter_mut().for_each(|(key, mut sprite)| {
        sprite.color = if held.0.contains(&key.note) {
            palette::PRESSED
        } else if let Some(hand) = sounding.get(&key.note) {
            palette::hand(*hand)
        } else {
            match key.color {
                KeyColor::White => palette::WHITE_KEY,
                KeyColor::Black => palette::BLACK_KEY,
            }
        };
    });
}

fn draw_hit_line(mut gizmos: Gizmos) {
    gizmos.line_2d(
        Vec2::new(layout::LEFT, layout::HIT_LINE + 1.0),
        Vec2::new(layout::RIGHT, layout::HIT_LINE + 1.0),
        palette::HIT_LINE,
    );
}
