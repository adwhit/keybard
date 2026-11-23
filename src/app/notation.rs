//! Scrolling grand-staff notation at the top of the screen. Notes scroll from
//! right to left and are "played" as they cross the playhead.

mod glyphs;

use bevy::prelude::*;

use super::transport::Transport;
use super::{ActiveSong, Judge, Phase, palette};
use crate::layout::{self, staff};
use crate::music::{Accidental, Clef, StaffPosition, StemDirection};
use crate::performance::NoteStatus;
use crate::song::{Beats, Hand, Note, NoteId, Seconds, Song};

#[derive(Default, Reflect, GizmoConfigGroup)]
struct NotationGizmos;

#[derive(Default, Reflect, GizmoConfigGroup)]
struct ClefGizmos;

#[derive(Component, Debug, Clone, Copy)]
pub struct NoteHead(pub NoteId);

#[derive(Resource)]
struct HeadMaterials {
    ink: Handle<ColorMaterial>,
    missed: Handle<ColorMaterial>,
    right: Handle<ColorMaterial>,
    left: Handle<ColorMaterial>,
}

impl HeadMaterials {
    fn for_hand(&self, hand: Hand) -> Handle<ColorMaterial> {
        match hand {
            Hand::Right => self.right.clone(),
            Hand::Left => self.left.clone(),
        }
    }
}

/// How a note's length is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoteShape {
    /// Hollow head, no stem.
    Whole,
    /// Hollow head with stem.
    Half,
    /// Filled head with stem.
    Quarter,
    /// Filled head, stem and flag.
    Eighth,
}

impl NoteShape {
    fn from_beats(beats: Beats) -> Self {
        match beats.0 {
            b if b >= 3.5 => Self::Whole,
            b if b >= 1.75 => Self::Half,
            b if b >= 0.75 => Self::Quarter,
            _ => Self::Eighth,
        }
    }

    fn is_hollow(self) -> bool {
        matches!(self, Self::Whole | Self::Half)
    }

    fn has_stem(self) -> bool {
        self != Self::Whole
    }
}

const HEAD_HALF_SIZE: Vec2 = Vec2::new(6.0, 4.4);
const HEAD_TILT: f32 = 0.35;
const HEAD_Z: f32 = 6.0;
const STEM_LENGTH: f32 = 34.0;
const LEDGER_HALF_WIDTH: f32 = 10.0;
const SHARP_OFFSET: f32 = 16.0;

pub fn plugin(app: &mut App) {
    app.init_gizmo_group::<NotationGizmos>()
        .init_gizmo_group::<ClefGizmos>()
        .add_systems(Startup, (configure_gizmos, spawn_heads))
        .add_systems(
            Update,
            (update_heads, draw_staves, draw_note_marks).in_set(Phase::Present),
        );
}

fn configure_gizmos(mut store: ResMut<GizmoConfigStore>) {
    store.config_mut::<NotationGizmos>().0.line.width = 1.5;
    store.config_mut::<ClefGizmos>().0.line.width = 2.5;
}

fn clef_for(hand: Hand) -> Clef {
    match hand {
        Hand::Right => Clef::Treble,
        Hand::Left => Clef::Bass,
    }
}

fn head_position(note: &Note) -> (Clef, StaffPosition) {
    let clef = clef_for(note.hand);
    (clef, clef.staff_position(note.pitch))
}

fn shape_of(song: &Song, note: &Note) -> NoteShape {
    NoteShape::from_beats(song.tempo.to_beats(note.duration))
}

fn spawn_heads(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    song: Res<ActiveSong>,
) {
    let head_mesh = meshes.add(Ellipse::new(HEAD_HALF_SIZE.x, HEAD_HALF_SIZE.y));
    let hole_mesh = meshes.add(Ellipse::new(
        HEAD_HALF_SIZE.x * 0.62,
        HEAD_HALF_SIZE.y * 0.5,
    ));
    let paper = materials.add(palette::PAPER);
    let head_materials = HeadMaterials {
        ink: materials.add(palette::INK),
        missed: materials.add(palette::MISSED),
        right: materials.add(palette::hand(Hand::Right)),
        left: materials.add(palette::hand(Hand::Left)),
    };
    let song = &song.0;
    song.iter().for_each(|(id, note)| {
        let mut head = commands.spawn((
            NoteHead(id),
            Mesh2d(head_mesh.clone()),
            MeshMaterial2d(head_materials.ink.clone()),
            Transform::from_xyz(0.0, 0.0, HEAD_Z).with_rotation(Quat::from_rotation_z(HEAD_TILT)),
            Visibility::Hidden,
        ));
        if shape_of(song, note).is_hollow() {
            head.with_child((
                Mesh2d(hole_mesh.clone()),
                MeshMaterial2d(paper.clone()),
                Transform::from_xyz(0.0, 0.0, 0.1),
            ));
        }
    });
    commands.insert_resource(head_materials);
}

/// Range of song time currently visible on the staff.
fn visible_window(now: Seconds) -> (Seconds, Seconds) {
    let behind = (staff::PLAYHEAD_X - staff::CLEF_AREA_RIGHT) / staff::PIXELS_PER_SECOND;
    let ahead = (layout::RIGHT - staff::PLAYHEAD_X) / staff::PIXELS_PER_SECOND;
    (Seconds(now.0 - behind), Seconds(now.0 + ahead))
}

fn update_heads(
    mut heads: Query<(
        &NoteHead,
        &mut Transform,
        &mut Visibility,
        &mut MeshMaterial2d<ColorMaterial>,
    )>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    judge: Res<Judge>,
    materials: Res<HeadMaterials>,
) {
    let now = transport.time;
    heads
        .iter_mut()
        .for_each(|(head, mut transform, mut visibility, mut material)| {
            let note = song.0.note(head.0);
            let x = staff::x(note.start - now);
            if !staff::is_visible(x) {
                *visibility = Visibility::Hidden;
                return;
            }
            *visibility = Visibility::Inherited;
            let (clef, position) = head_position(note);
            transform.translation.x = x;
            transform.translation.y = staff::y(clef, position);
            let wanted = if note.is_sounding_at(now) {
                materials.for_hand(note.hand)
            } else if judge.0.status(head.0) == NoteStatus::Missed {
                materials.missed.clone()
            } else {
                materials.ink.clone()
            };
            if material.0 != wanted {
                material.0 = wanted;
            }
        });
}

fn draw_staves(
    mut gizmos: Gizmos<NotationGizmos>,
    mut clef_gizmos: Gizmos<ClefGizmos>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
) {
    let right = layout::RIGHT - 10.0;
    Clef::ALL.into_iter().for_each(|clef| {
        (0..5).map(|i| StaffPosition(i * 2)).for_each(|p| {
            let y = staff::y(clef, p);
            gizmos.line_2d(
                Vec2::new(staff::LEFT_EDGE, y),
                Vec2::new(right, y),
                palette::STAFF_LINE,
            );
        });
        let origin = Vec2::new(staff::LEFT_EDGE + 22.0, staff::bottom_line(clef));
        glyphs::draw_clef(&mut clef_gizmos, clef, origin, palette::INK);
    });

    let top = staff::y(Clef::Treble, StaffPosition::TOP_LINE);
    let bottom = staff::y(Clef::Bass, StaffPosition::BOTTOM_LINE);
    // System barline joining the two staves.
    gizmos.line_2d(
        Vec2::new(staff::LEFT_EDGE, bottom),
        Vec2::new(staff::LEFT_EDGE, top),
        palette::INK,
    );

    // Bar lines scrolling with the music.
    let now = transport.time;
    let (from, to) = visible_window(now);
    let bar = song.0.tempo.bar_length();
    let first = (from.0 / bar.0).ceil().max(1.0) as u32;
    (first..)
        .map(|i| Seconds(i as f32 * bar.0))
        .take_while(|t| *t <= to)
        // Bar lines sit just before the downbeat they introduce.
        .map(|t| staff::x(t - now) - 2.0 * HEAD_HALF_SIZE.x)
        .filter(|x| staff::is_visible(*x))
        .for_each(|x| {
            [Clef::Treble, Clef::Bass].into_iter().for_each(|clef| {
                gizmos.line_2d(
                    Vec2::new(x, staff::y(clef, StaffPosition::BOTTOM_LINE)),
                    Vec2::new(x, staff::y(clef, StaffPosition::TOP_LINE)),
                    palette::STAFF_LINE,
                );
            });
        });

    gizmos.line_2d(
        Vec2::new(staff::PLAYHEAD_X, bottom - 30.0),
        Vec2::new(staff::PLAYHEAD_X, top + 30.0),
        palette::PLAYHEAD,
    );
}

/// Ledger lines, stems, flags and accidentals for visible notes.
fn draw_note_marks(
    mut gizmos: Gizmos<NotationGizmos>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
) {
    let now = transport.time;
    let (from, to) = visible_window(now);
    let song = &song.0;
    let notes = song.notes();
    let first = notes.partition_point(|n| n.start < from);
    notes[first..]
        .iter()
        .take_while(|n| n.start <= to)
        .for_each(|note| {
            let x = staff::x(note.start - now);
            let (clef, position) = head_position(note);
            let y = staff::y(clef, position);
            let shape = shape_of(song, note);

            position.ledger_lines().into_iter().for_each(|ledger| {
                let ly = staff::y(clef, ledger);
                gizmos.line_2d(
                    Vec2::new(x - LEDGER_HALF_WIDTH, ly),
                    Vec2::new(x + LEDGER_HALF_WIDTH, ly),
                    palette::INK,
                );
            });

            if shape.has_stem() {
                let (stem_x, dir) = match position.stem_direction() {
                    StemDirection::Up => (x + HEAD_HALF_SIZE.x - 0.8, 1.0),
                    StemDirection::Down => (x - HEAD_HALF_SIZE.x + 0.8, -1.0),
                };
                let tip = Vec2::new(stem_x, y + dir * STEM_LENGTH);
                gizmos.line_2d(Vec2::new(stem_x, y), tip, palette::INK);
                if shape == NoteShape::Eighth {
                    gizmos.line_2d(tip, tip + Vec2::new(7.0, -dir * 12.0), palette::INK);
                }
            }

            if note.pitch.pitch_class().accidental() == Accidental::Sharp {
                glyphs::draw_sharp(&mut gizmos, Vec2::new(x - SHARP_OFFSET, y), palette::INK);
            }
        });
}
