//! Clef shapes as filled outlines, independent of rendering.
//!
//! Each clef is one smooth pen stroke (a Catmull-Rom curve through hand-placed
//! points) plus filled dots. The stroke is drawn as if with a broad nib held
//! at an angle: strokes across the nib are thick, strokes along it are
//! hairlines, which gives clefs their engraved thick-and-thin look.
//!
//! Coordinates are in staff steps (half a space) from the staff's bottom
//! line, x growing rightwards from the glyph's left edge.

use std::f32::consts::TAU;

use bevy::math::Vec2;

use crate::music::Clef;

/// A filled shape as triangles, in staff steps.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outline {
    pub triangles: Vec<[Vec2; 3]>,
}

impl Outline {
    /// Smallest box around the shape: (min, max).
    pub fn bounds(&self) -> (Vec2, Vec2) {
        self.triangles.iter().flatten().fold(
            (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        )
    }

    /// Whether `p` lies inside any triangle.
    pub fn contains(&self, p: Vec2) -> bool {
        self.triangles.iter().any(|[a, b, c]| {
            let side = |u: Vec2, v: Vec2| (v - u).perp_dot(p - u);
            let (d1, d2, d3) = (side(*a, *b), side(*b, *c), side(*c, *a));
            let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
            let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
            !(negative && positive)
        })
    }

    fn extend(&mut self, other: Outline) {
        self.triangles.extend(other.triangles);
    }
}

/// A broad-nib pen.
#[derive(Debug, Clone, Copy)]
struct Pen {
    /// Width of a stroke drawn along the nib.
    thin: f32,
    /// Width of a stroke drawn across the nib.
    thick: f32,
    /// Direction the nib's edge points.
    nib: Vec2,
}

impl Pen {
    fn width(self, tangent: Vec2) -> f32 {
        let across = tangent.perp_dot(self.nib).abs();
        self.thin + (self.thick - self.thin) * across
    }
}

/// How a stroke's width eases in and out at its ends (fractions of its
/// length).
#[derive(Debug, Clone, Copy)]
struct Taper {
    start: f32,
    end: f32,
}

const SAMPLES_PER_SEGMENT: usize = 12;

/// Points along a Catmull-Rom curve through `points` (ends included).
fn smooth(points: &[Vec2]) -> Vec<Vec2> {
    let at = |i: isize| points[i.clamp(0, points.len() as isize - 1) as usize];
    let mut out: Vec<Vec2> = (0..points.len().saturating_sub(1) as isize)
        .flat_map(|i| {
            let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
            (0..SAMPLES_PER_SEGMENT).map(move |s| {
                let t = s as f32 / SAMPLES_PER_SEGMENT as f32;
                let (t2, t3) = (t * t, t * t * t);
                0.5 * ((2.0 * p1)
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
            })
        })
        .collect();
    out.extend(points.last());
    out
}

/// A pen stroke along the curve through `points`, as a strip of triangles.
fn stroke(points: &[Vec2], pen: Pen, taper: Taper) -> Outline {
    let path = smooth(points);
    let lengths: Vec<f32> = path
        .iter()
        .scan((0.0, path[0]), |(total, last), p| {
            *total += p.distance(*last);
            *last = *p;
            Some(*total)
        })
        .collect();
    let length = lengths.last().copied().unwrap_or(1.0).max(1e-3);
    let edges: Vec<(Vec2, Vec2)> = path
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let before = path[i.saturating_sub(1)];
            let after = path[(i + 1).min(path.len() - 1)];
            let tangent = (after - before).normalize_or(Vec2::X);
            let s = lengths[i] / length;
            let ease = |x: f32| x.clamp(0.0, 1.0).sqrt();
            let taper_in = if taper.start > 0.0 {
                ease(s / taper.start)
            } else {
                1.0
            };
            let taper_out = if taper.end > 0.0 {
                ease((1.0 - s) / taper.end)
            } else {
                1.0
            };
            let width = (pen.width(tangent) * taper_in.min(taper_out)).max(pen.thin * 0.5);
            let normal = tangent.perp() * width / 2.0;
            (*p + normal, *p - normal)
        })
        .collect();
    Outline {
        triangles: edges
            .windows(2)
            .flat_map(|w| {
                let ((l0, r0), (l1, r1)) = (w[0], w[1]);
                [[l0, r0, l1], [r0, r1, l1]]
            })
            .collect(),
    }
}

fn dot(center: Vec2, radius: f32) -> Outline {
    const SIDES: usize = 20;
    let rim = |i: usize| {
        let a = i as f32 / SIDES as f32 * TAU;
        center + Vec2::new(a.cos(), a.sin()) * radius
    };
    Outline {
        triangles: (0..SIDES).map(|i| [center, rim(i), rim(i + 1)]).collect(),
    }
}

/// Nib held about 35 degrees up from horizontal, as calligraphers do.
const NIB: Vec2 = Vec2::new(0.819, 0.574);

/// The treble (G) clef: a spiral around the G line, rising into a loop above
/// the staff, then a long stroke down through the spiral ending in a curl.
pub fn treble() -> Outline {
    let center = Vec2::new(2.2, 2.3);
    // A turn and a quarter outwards, anticlockwise, from just under the eye
    // to the right side of the belly, heading up.
    let spiral = (0..=20).map(|i| {
        let t = i as f32 / 20.0;
        let angle = -TAU / 4.0 + t * TAU * 1.25;
        let radius = 0.9 + 2.1 * t;
        center + Vec2::new(angle.cos(), angle.sin() * 1.25) * radius
    });
    let rest = [
        Vec2::new(5.2, 4.4),
        Vec2::new(4.4, 5.9),
        Vec2::new(3.5, 7.2),
        Vec2::new(2.9, 8.6),
        Vec2::new(2.8, 9.9),
        Vec2::new(3.3, 10.8),
        Vec2::new(3.9, 10.6),
        Vec2::new(4.1, 9.6),
        Vec2::new(3.6, 8.3),
        Vec2::new(3.1, 6.6),
        Vec2::new(3.0, 4.5),
        Vec2::new(3.2, 1.5),
        Vec2::new(3.4, -1.0),
        Vec2::new(3.3, -2.6),
        Vec2::new(2.6, -3.3),
        Vec2::new(1.6, -3.2),
        Vec2::new(1.0, -2.6),
    ];
    let points: Vec<Vec2> = spiral.chain(rest).collect();
    let mut outline = stroke(
        &points,
        Pen {
            thin: 0.28,
            thick: 1.0,
            nib: NIB,
        },
        Taper {
            start: 0.09,
            end: 0.0,
        },
    );
    outline.extend(dot(Vec2::new(1.65, -2.45), 0.95));
    outline
}

/// The bass (F) clef: a dot on the F line, a curl over the top and a tail
/// sweeping down to the left, with two dots either side of the F line.
pub fn bass() -> Outline {
    let points = [
        Vec2::new(0.2, 6.3),
        Vec2::new(0.7, 7.5),
        Vec2::new(1.9, 8.15),
        Vec2::new(3.3, 8.0),
        Vec2::new(4.4, 7.0),
        Vec2::new(4.7, 5.4),
        Vec2::new(4.3, 3.7),
        Vec2::new(3.3, 2.3),
        Vec2::new(1.8, 1.0),
        Vec2::new(-0.1, -0.1),
    ];
    let mut outline = stroke(
        &points,
        Pen {
            thin: 0.3,
            thick: 1.25,
            nib: NIB,
        },
        Taper {
            start: 0.0,
            end: 0.45,
        },
    );
    [
        dot(Vec2::new(1.0, 6.0), 1.0),
        dot(Vec2::new(6.1, 7.0), 0.55),
        dot(Vec2::new(6.1, 5.0), 0.55),
    ]
    .into_iter()
    .for_each(|d| outline.extend(d));
    outline
}

pub fn clef(clef: Clef) -> Outline {
    match clef {
        Clef::Treble => treble(),
        Clef::Bass => bass(),
    }
}
