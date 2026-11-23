//! Hand-drawn vector glyphs for clefs and accidentals, built from gizmo lines.
//!
//! Glyph coordinates are in staff steps (half a space) relative to the
//! staff's bottom line, so they scale with `layout::staff::STEP`.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::key::KeySignature;
use crate::layout::staff;
use crate::music::{Accidental, Clef, Letter, StaffPosition};
use crate::notation::RestShape;

fn to_world(origin: Vec2, p: Vec2) -> Vec2 {
    origin + p * staff::STEP
}

/// Points of a treble (G) clef: a spiral around the G line, a tall loop above
/// the staff and a descending tail that ends in a hook.
fn treble_path() -> Vec<Vec2> {
    let center = Vec2::new(0.4, 2.6);
    let spiral = (0..=40).map(|i| {
        let t = i as f32 / 40.0;
        let angle = -PI / 2.0 + t * TAU * 1.1;
        let radius = 0.6 + 2.0 * t;
        center + Vec2::new(angle.cos(), angle.sin()) * radius * Vec2::new(1.0, 1.15)
    });
    let upper = [
        Vec2::new(1.3, 6.4),
        Vec2::new(1.9, 8.6),
        Vec2::new(1.7, 10.8),
        Vec2::new(0.9, 11.9),
        Vec2::new(0.2, 11.2),
        Vec2::new(-0.1, 9.4),
        Vec2::new(0.3, 6.5),
        Vec2::new(0.8, 2.0),
        Vec2::new(1.2, -2.4),
        Vec2::new(0.8, -3.6),
        Vec2::new(-0.1, -3.9),
        Vec2::new(-0.8, -3.3),
        Vec2::new(-0.7, -2.4),
    ];
    spiral.chain(upper).collect()
}

/// Points of a bass (F) clef curl starting at the F line.
fn bass_path() -> Vec<Vec2> {
    vec![
        Vec2::new(0.3, 6.0),
        Vec2::new(0.6, 7.2),
        Vec2::new(1.6, 7.9),
        Vec2::new(3.0, 7.8),
        Vec2::new(4.0, 6.8),
        Vec2::new(4.1, 5.0),
        Vec2::new(3.3, 3.0),
        Vec2::new(1.8, 1.3),
        Vec2::new(-0.2, -0.2),
    ]
}

pub fn draw_clef<C: GizmoConfigGroup>(
    gizmos: &mut Gizmos<C>,
    clef: Clef,
    origin: Vec2,
    color: Color,
) {
    match clef {
        Clef::Treble => {
            gizmos.linestrip_2d(
                treble_path().into_iter().map(|p| to_world(origin, p)),
                color,
            );
            filled_dot(gizmos, to_world(origin, Vec2::new(-0.55, -2.6)), 3.0, color);
        }
        Clef::Bass => {
            gizmos.linestrip_2d(bass_path().into_iter().map(|p| to_world(origin, p)), color);
            filled_dot(gizmos, to_world(origin, Vec2::new(0.7, 6.0)), 4.0, color);
            [7.0, 5.0].into_iter().for_each(|y| {
                filled_dot(gizmos, to_world(origin, Vec2::new(5.3, y)), 1.6, color);
            });
        }
    }
}

/// Gizmos only draw outlines, so fill a dot with concentric circles.
fn filled_dot<C: GizmoConfigGroup>(
    gizmos: &mut Gizmos<C>,
    center: Vec2,
    radius: f32,
    color: Color,
) {
    (0..=(radius as usize * 2)).for_each(|i| {
        gizmos.circle_2d(center, i as f32 * 0.5, color);
    });
}

/// A sharp sign centred at `center`.
pub fn draw_sharp<C: GizmoConfigGroup>(gizmos: &mut Gizmos<C>, center: Vec2, color: Color) {
    let s = staff::STEP;
    [-0.5, 0.5].into_iter().for_each(|dx| {
        let x = center.x + dx * s;
        gizmos.line_2d(
            Vec2::new(x, center.y - 1.5 * s),
            Vec2::new(x, center.y + 1.7 * s),
            color,
        );
    });
    [-0.6, 0.6].into_iter().for_each(|dy| {
        let y = center.y + dy * s;
        gizmos.line_2d(
            Vec2::new(center.x - 1.1 * s, y - 0.25 * s),
            Vec2::new(center.x + 1.1 * s, y + 0.25 * s),
            color,
        );
    });
}

/// Fill a rectangle (in world units) with horizontal gizmo lines.
fn filled_rect<C: GizmoConfigGroup>(gizmos: &mut Gizmos<C>, min: Vec2, max: Vec2, color: Color) {
    let rows = ((max.y - min.y) * 2.0).ceil().max(1.0) as usize;
    (0..=rows).for_each(|i| {
        let y = min.y + (max.y - min.y) * i as f32 / rows as f32;
        gizmos.line_2d(Vec2::new(min.x, y), Vec2::new(max.x, y), color);
    });
}

/// A rest glyph centred horizontally on `x`, on the staff whose bottom line
/// is at `bottom`.
pub fn draw_rest<C: GizmoConfigGroup>(
    gizmos: &mut Gizmos<C>,
    shape: RestShape,
    x: f32,
    bottom: f32,
    color: Color,
) {
    let s = staff::STEP;
    let at = |dx: f32, steps: f32| Vec2::new(x + dx * s, bottom + steps * s);
    match shape {
        // Hangs from the fourth line.
        RestShape::WholeBar => filled_rect(gizmos, at(-1.2, 5.0), at(1.2, 6.0), color),
        // Sits on the middle line.
        RestShape::Half => filled_rect(gizmos, at(-1.2, 4.0), at(1.2, 5.0), color),
        RestShape::Quarter => gizmos.linestrip_2d(
            [
                at(-0.6, 7.0),
                at(0.7, 5.6),
                at(-0.5, 4.6),
                at(0.7, 3.2),
                at(-0.6, 3.4),
                at(0.2, 2.0),
            ],
            color,
        ),
        RestShape::Eighth => {
            filled_dot(gizmos, at(-0.7, 5.3), 1.5, color);
            gizmos.linestrip_2d([at(-0.7, 5.0), at(0.9, 5.6), at(-0.2, 2.2)], color);
        }
    }
}

/// A flat sign centred on `center` (the notehead's line or space).
pub fn draw_flat<C: GizmoConfigGroup>(gizmos: &mut Gizmos<C>, center: Vec2, color: Color) {
    let s = staff::STEP;
    let at = |dx: f32, dy: f32| center + Vec2::new(dx * s, dy * s);
    gizmos.line_2d(at(-0.6, 3.2), at(-0.6, -0.8), color);
    gizmos.linestrip_2d(
        [at(-0.6, -0.8), at(0.7, 0.3), at(0.6, 1.0), at(-0.6, 0.6)],
        color,
    );
}

/// A natural sign centred on `center`.
pub fn draw_natural<C: GizmoConfigGroup>(gizmos: &mut Gizmos<C>, center: Vec2, color: Color) {
    let s = staff::STEP;
    let at = |dx: f32, dy: f32| center + Vec2::new(dx * s, dy * s);
    gizmos.line_2d(at(-0.5, 2.0), at(-0.5, -0.9), color);
    gizmos.line_2d(at(0.5, 0.9), at(0.5, -2.0), color);
    gizmos.line_2d(at(-0.5, -0.9), at(0.5, -0.5), color);
    gizmos.line_2d(at(-0.5, 0.5), at(0.5, 0.9), color);
}

pub fn draw_accidental<C: GizmoConfigGroup>(
    gizmos: &mut Gizmos<C>,
    accidental: Accidental,
    center: Vec2,
    color: Color,
) {
    match accidental {
        Accidental::Sharp => draw_sharp(gizmos, center, color),
        Accidental::Flat => draw_flat(gizmos, center, color),
        Accidental::Natural => draw_natural(gizmos, center, color),
    }
}

/// Standard treble-staff positions of each signature accidental; the bass
/// staff uses the same shapes two steps lower.
fn signature_position(letter: Letter, sharp: bool) -> i32 {
    match (letter, sharp) {
        (Letter::F, true) => 8,
        (Letter::C, true) => 5,
        (Letter::G, true) => 9,
        (Letter::D, true) => 6,
        (Letter::A, true) => 4,
        (Letter::E, true) => 7,
        (Letter::B, true) => 3,
        (Letter::B, false) => 4,
        (Letter::E, false) => 7,
        (Letter::A, false) => 3,
        (Letter::D, false) => 6,
        (Letter::G, false) => 2,
        (Letter::C, false) => 5,
        (Letter::F, false) => 1,
    }
}

/// Gap between successive signature accidentals.
const SIGNATURE_SPACING: f32 = 8.0;

pub fn draw_key_signature<C: GizmoConfigGroup>(
    gizmos: &mut Gizmos<C>,
    key: KeySignature,
    clef: Clef,
    x: f32,
    color: Color,
) {
    let sharp = key.accidentals() > 0;
    let offset = match clef {
        Clef::Treble => 0,
        Clef::Bass => -2,
    };
    key.altered_letters()
        .into_iter()
        .enumerate()
        .for_each(|(i, letter)| {
            let position = StaffPosition(signature_position(letter, sharp) + offset);
            let center = Vec2::new(x + i as f32 * SIGNATURE_SPACING, staff::y(clef, position));
            let accidental = if sharp {
                Accidental::Sharp
            } else {
                Accidental::Flat
            };
            draw_accidental(gizmos, accidental, center, color);
        });
}
