//! Scrolling grand-staff notation at the top of the screen. Notes scroll from
//! right to left and are "played" as they cross the playhead.

mod glyphs;

use bevy::prelude::*;

use super::songs::{SongLoad, SongScoped};
use super::transport::Transport;
use super::{ActiveSong, Judge, Phase, palette};
use crate::layout::{self, staff};
use crate::music::{Clef, StaffPosition, StemDirection};
use crate::notation::{Rest, RestShape, beams, rests};
use crate::performance::NoteStatus;
use crate::song::{Beats, Hand, Note, NoteId, Seconds, Song};

#[derive(Default, Reflect, GizmoConfigGroup)]
struct NotationGizmos;

#[derive(Default, Reflect, GizmoConfigGroup)]
struct ClefGizmos;

#[derive(Component, Debug, Clone, Copy)]
pub struct NoteHead(pub NoteId);

/// A beamed group: its notes, staff and shared stem direction.
#[derive(Debug, Clone)]
pub struct BeamGroup {
    pub notes: Vec<NoteId>,
    pub clef: Clef,
    pub direction: StemDirection,
}

/// Beam groups for the current song.
#[derive(Resource, Debug, Default, Clone)]
pub struct SongBeams {
    pub groups: Vec<BeamGroup>,
    member: std::collections::HashMap<NoteId, usize>,
}

/// Shortest stem a beamed note may have.
const MIN_BEAMED_STEM: f32 = 24.0;
/// Most a beam may rise or fall end to end.
const MAX_BEAM_SLOPE: f32 = 10.0;
const BEAM_THICKNESS: f32 = 4.0;

/// Rests for the current song, with the staff each belongs on.
#[derive(Resource, Debug, Default, Clone)]
pub struct SongRests(pub Vec<(Clef, Rest)>);

/// The recommended-finger digit beside a notehead.
#[derive(Component, Debug, Clone, Copy)]
pub struct HeadFinger;

/// Gap between a notehead's centre and its finger digit.
const FINGER_OFFSET: f32 = 13.0;

#[derive(Resource)]
struct HeadMaterials {
    ink: Handle<ColorMaterial>,
    missed: Handle<ColorMaterial>,
    played: Handle<ColorMaterial>,
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
/// Where the key signature starts, after the clef.
const KEY_SIGNATURE_X: f32 = 52.0;

pub fn plugin(app: &mut App) {
    app.init_gizmo_group::<NotationGizmos>()
        .init_gizmo_group::<ClefGizmos>()
        .add_systems(Startup, (configure_gizmos, create_head_assets))
        .init_resource::<SongRests>()
        .init_resource::<SongBeams>()
        .add_systems(
            Update,
            (spawn_heads, find_rests, find_beams).in_set(SongLoad),
        )
        .add_systems(
            Update,
            (
                update_heads,
                draw_staves,
                draw_note_marks,
                draw_rests,
                draw_beams,
            )
                .in_set(Phase::Present),
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

fn head_position(song: &Song, note: &Note) -> (Clef, StaffPosition) {
    let clef = clef_for(note.hand);
    (clef, clef.position_of(song.key().spell(note.pitch)))
}

fn shape_of(song: &Song, note: &Note) -> NoteShape {
    NoteShape::from_beats(song.tempo.to_beats(note.duration))
}

/// Meshes for noteheads, shared by every song.
#[derive(Resource)]
struct HeadMeshes {
    head: Handle<Mesh>,
    hole: Handle<Mesh>,
    paper: Handle<ColorMaterial>,
}

fn create_head_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.insert_resource(HeadMeshes {
        head: meshes.add(Ellipse::new(HEAD_HALF_SIZE.x, HEAD_HALF_SIZE.y)),
        hole: meshes.add(Ellipse::new(
            HEAD_HALF_SIZE.x * 0.62,
            HEAD_HALF_SIZE.y * 0.5,
        )),
        paper: materials.add(palette::PAPER),
    });
    commands.insert_resource(HeadMaterials {
        ink: materials.add(palette::INK),
        missed: materials.add(palette::MISSED),
        played: materials.add(palette::PLAYED_INK),
        right: materials.add(palette::hand(Hand::Right)),
        left: materials.add(palette::hand(Hand::Left)),
    });
}

fn spawn_heads(
    mut commands: Commands,
    meshes: Res<HeadMeshes>,
    materials: Res<HeadMaterials>,
    song: Res<ActiveSong>,
) {
    let song = &song.0;
    song.iter().for_each(|(id, note)| {
        let mut head = commands.spawn((
            SongScoped,
            NoteHead(id),
            Mesh2d(meshes.head.clone()),
            MeshMaterial2d(materials.ink.clone()),
            Transform::from_xyz(0.0, 0.0, HEAD_Z).with_rotation(Quat::from_rotation_z(HEAD_TILT)),
            Visibility::Hidden,
        ));
        // Put the digit on the side away from the stem.
        let (_, position) = head_position(song, note);
        let side = match position.stem_direction() {
            StemDirection::Up => -1.0,
            StemDirection::Down => 1.0,
        };
        head.with_child((
            HeadFinger,
            Text2d::new(song.finger(id).number().to_string()),
            TextFont {
                font_size: FontSize::Px(9.0),
                ..default()
            },
            TextColor(palette::STAFF_LINE),
            // Undo the head's tilt so the digit stands upright.
            Transform::from_rotation(Quat::from_rotation_z(-HEAD_TILT)).with_translation(
                Quat::from_rotation_z(-HEAD_TILT) * Vec3::new(0.0, side * FINGER_OFFSET, 0.2),
            ),
        ));
        if shape_of(song, note).is_hollow() {
            head.with_child((
                Mesh2d(meshes.hole.clone()),
                MeshMaterial2d(meshes.paper.clone()),
                Transform::from_xyz(0.0, 0.0, 0.1),
            ));
        }
    });
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
            let (clef, position) = head_position(&song.0, note);
            transform.translation.x = x;
            transform.translation.y = staff::y(clef, position);
            let wanted = if note.is_sounding_at(now) {
                materials.for_hand(note.hand)
            } else {
                match judge.0.status(head.0) {
                    NoteStatus::Missed => materials.missed.clone(),
                    NoteStatus::Hit(_) => materials.played.clone(),
                    NoteStatus::Pending | NoteStatus::AutoPlayed | NoteStatus::Skipped => {
                        materials.ink.clone()
                    }
                }
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
        glyphs::draw_key_signature(
            &mut gizmos,
            song.0.key(),
            clef,
            staff::LEFT_EDGE + KEY_SIGNATURE_X,
            palette::INK,
        );
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

fn find_rests(song: Res<ActiveSong>, mut found: ResMut<SongRests>) {
    found.0 = [Hand::Right, Hand::Left]
        .into_iter()
        .flat_map(|hand| {
            rests(&song.0, hand)
                .into_iter()
                .map(move |rest| (clef_for(hand), rest))
        })
        .collect();
}

fn find_beams(song: Res<ActiveSong>, mut found: ResMut<SongBeams>) {
    let song = &song.0;
    let groups: Vec<BeamGroup> = [Hand::Right, Hand::Left]
        .into_iter()
        .flat_map(|hand| {
            beams(song, hand).into_iter().map(move |notes| {
                let positions: Vec<i32> = notes
                    .iter()
                    .map(|id| head_position(song, song.note(*id)).1.0)
                    .collect();
                let mean = positions.iter().sum::<i32>() as f32 / positions.len() as f32;
                let direction = if mean >= StaffPosition::MIDDLE_LINE.0 as f32 {
                    StemDirection::Down
                } else {
                    StemDirection::Up
                };
                BeamGroup {
                    notes,
                    clef: clef_for(hand),
                    direction,
                }
            })
        })
        .collect();
    found.member = groups
        .iter()
        .enumerate()
        .flat_map(|(g, group)| group.notes.iter().map(move |id| (*id, g)))
        .collect();
    found.groups = groups;
}

/// Stems and beams for beamed groups: one stem per event reaching a shared,
/// gently sloped beam that clears every notehead.
fn draw_beams(
    mut gizmos: Gizmos<NotationGizmos>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    found: Res<SongBeams>,
) {
    let song = &song.0;
    let now = transport.time;
    found.groups.iter().for_each(|group| {
        // One entry per event: (stem x, outermost head y in stem direction).
        let dir = match group.direction {
            StemDirection::Up => 1.0,
            StemDirection::Down => -1.0,
        };
        let mut events: Vec<(f32, f32)> = Vec::new();
        group.notes.iter().for_each(|id| {
            let note = song.note(*id);
            let x = staff::x(note.start - now);
            let stem_x = x + dir * (HEAD_HALF_SIZE.x - 0.8);
            let y = staff::y(group.clef, head_position(song, note).1);
            match events.last_mut() {
                Some(last) if (last.0 - stem_x).abs() < 0.5 => {
                    // Chords: the stem starts from the far head.
                    last.1 = if dir > 0.0 {
                        last.1.min(y)
                    } else {
                        last.1.max(y)
                    };
                }
                _ => events.push((stem_x, y)),
            }
        });
        let (Some(first), Some(last)) = (events.first().copied(), events.last().copied()) else {
            return;
        };
        if !events.iter().any(|(x, _)| staff::is_visible(*x)) {
            return;
        }
        // Natural tips, then a slope limited to MAX_BEAM_SLOPE...
        let tip = |y: f32| y + dir * STEM_LENGTH;
        let rise = (tip(last.1) - tip(first.1)).clamp(-MAX_BEAM_SLOPE, MAX_BEAM_SLOPE);
        let span = (last.0 - first.0).max(1.0);
        let line = |x: f32, offset: f32| tip(first.1) + offset + rise * (x - first.0) / span;
        // ...shifted so every stem is at least MIN_BEAMED_STEM long.
        let offset = events
            .iter()
            .map(|(x, y)| {
                let length = (line(*x, 0.0) - y) * dir;
                (MIN_BEAMED_STEM - length).max(0.0) * dir
            })
            .fold(0.0_f32, |a, b| if dir > 0.0 { a.max(b) } else { a.min(b) });
        let visible = |x: f32| x.clamp(staff::CLEF_AREA_RIGHT, layout::RIGHT);
        events
            .iter()
            .filter(|(x, _)| staff::is_visible(*x))
            .for_each(|(x, y)| {
                gizmos.line_2d(
                    Vec2::new(*x, *y),
                    Vec2::new(*x, line(*x, offset)),
                    palette::INK,
                );
            });
        let (from, to) = (visible(first.0), visible(last.0));
        if to > from {
            let steps = (BEAM_THICKNESS * 2.0) as usize;
            (0..=steps).for_each(|i| {
                let t = -dir * BEAM_THICKNESS * i as f32 / steps as f32;
                gizmos.line_2d(
                    Vec2::new(from, line(from, offset) + t),
                    Vec2::new(to, line(to, offset) + t),
                    palette::INK,
                );
            });
        }
    });
}

fn draw_rests(mut gizmos: Gizmos<ClefGizmos>, transport: Res<Transport>, found: Res<SongRests>) {
    let now = transport.time;
    found.0.iter().for_each(|(clef, rest)| {
        // Whole-bar rests sit in the middle of their bar; others where they start.
        let at = match rest.shape {
            RestShape::WholeBar => rest.start + Seconds(rest.duration.0 / 2.0),
            RestShape::Half | RestShape::Quarter | RestShape::Eighth => rest.start,
        };
        let x = staff::x(at - now);
        if staff::is_visible(x) {
            glyphs::draw_rest(
                &mut gizmos,
                rest.shape,
                x,
                staff::bottom_line(*clef),
                palette::INK,
            );
        }
    });
}

/// Ledger lines, stems, flags and accidentals for visible notes.
fn draw_note_marks(
    mut gizmos: Gizmos<NotationGizmos>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    beams: Res<SongBeams>,
) {
    let now = transport.time;
    let (from, to) = visible_window(now);
    let song = &song.0;
    let notes = song.notes();
    let first = notes.partition_point(|n| n.start < from);
    notes[first..]
        .iter()
        .enumerate()
        .take_while(|(_, n)| n.start <= to)
        .for_each(|(offset, note)| {
            let id = NoteId(first + offset);
            let x = staff::x(note.start - now);
            let (clef, position) = head_position(song, note);
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

            // Beamed notes get their stems from `draw_beams`.
            let beamed = beams.member.contains_key(&id);
            if shape.has_stem() && !beamed {
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

            let key = song.key();
            if let Some(accidental) = key.accidental_to_show(key.spell(note.pitch)) {
                glyphs::draw_accidental(
                    &mut gizmos,
                    accidental,
                    Vec2::new(x - SHARP_OFFSET, y),
                    palette::INK,
                );
            }
        });
}
