//! Hand-drawn vector glyphs for clefs and accidentals, built from gizmo lines.
//!
//! Glyph coordinates are in staff steps (half a space) relative to the
//! staff's bottom line, so they scale with `layout::staff::STEP`.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::layout::staff;
use crate::music::Clef;

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
