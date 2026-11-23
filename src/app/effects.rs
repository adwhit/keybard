//! Short-lived eye candy driven by [`Feedback`]: particle bursts, floating
//! judgement text, key flashes and combo milestones.

use bevy::prelude::*;
use bevy::sprite::Text2dShadow;

use super::textures::Textures;
use super::{ActiveSong, Feedback, Judge, Keys, Phase, palette};
use crate::layout;
use crate::performance::Timing;
use crate::song::Hand;

/// Something that fades out and despawns after `lifetime` seconds.
#[derive(Component, Debug, Clone, Copy)]
pub struct Ephemeral {
    age: f32,
    lifetime: f32,
}

impl Ephemeral {
    fn new(lifetime: f32) -> Self {
        Self { age: 0.0, lifetime }
    }

    /// 0 when spawned, 1 when about to vanish.
    fn progress(self) -> f32 {
        (self.age / self.lifetime).clamp(0.0, 1.0)
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct Particle {
    velocity: Vec2,
    base_alpha: f32,
}

/// Floating text such as "PERFECT".
#[derive(Component, Debug, Clone, Copy)]
pub struct Popup {
    rise: f32,
    origin_y: f32,
    pop: f32,
}

/// A glow over a key that was just hit.
#[derive(Component, Debug, Clone, Copy)]
pub struct KeyFlash;

/// Tiny deterministic PRNG (xorshift32) so effects are reproducible in tests.
#[derive(Resource, Debug)]
struct Rng(u32);

impl Rng {
    fn next_f32(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next_f32()
    }
}

const GRAVITY: f32 = -700.0;
const PARTICLE_Z: f32 = 4.0;
const FLASH_Z: f32 = 12.0;
const POPUP_Z: f32 = 13.0;
const MILESTONES: [u32; 6] = [25, 50, 100, 200, 300, 500];

pub fn plugin(app: &mut App) {
    app.insert_resource(Rng(0x9E37_79B9)).add_systems(
        Update,
        (
            (spawn_from_feedback, combo_milestones),
            (age_ephemerals, move_particles, animate_popups, fade_flashes),
        )
            .chain()
            .in_set(Phase::Present),
    );
}

fn particle_count(timing: Option<Timing>) -> usize {
    match timing {
        Some(Timing::Perfect) => 22,
        Some(Timing::Great) => 14,
        Some(Timing::Good) => 8,
        // Auto-played notes in Listen mode get a gentle sprinkle.
        None => 6,
    }
}

fn spawn_from_feedback(
    mut commands: Commands,
    mut feedback: MessageReader<Feedback>,
    mut rng: ResMut<Rng>,
    song: Res<ActiveSong>,
    keys: Res<Keys>,
    textures: Res<Textures>,
    judge: Res<Judge>,
) {
    let range = keys.0;
    feedback.read().for_each(|event| {
        let (pitch, hand, timing) = match *event {
            Feedback::Hit(j) => {
                let note = song.0.note(j.note);
                (note.pitch, Some(note.hand), Some(j.timing))
            }
            Feedback::AutoPlayed(id) => {
                let note = song.0.note(id);
                (note.pitch, Some(note.hand), None)
            }
            Feedback::Missed(id) => (song.0.note(id).pitch, None, None),
            Feedback::Wrong(pitch) => (pitch, None, None),
        };
        let Some(geometry) = range.geometry(pitch) else {
            return;
        };
        let x = geometry.center_x;
        match (*event, hand) {
            (Feedback::Hit(_) | Feedback::AutoPlayed(_), Some(hand)) => {
                burst(&mut commands, &mut rng, &textures, x, hand, timing);
                flash(
                    &mut commands,
                    &textures,
                    x,
                    geometry.width,
                    palette::hand_lit(hand),
                );
            }
            _ => flash(&mut commands, &textures, x, geometry.width, palette::MISSED),
        }
        let text = match *event {
            Feedback::Hit(j) => Some((j.timing.label(), palette::timing(j.timing), 1.0)),
            Feedback::Missed(_) => Some(("MISS", palette::MISSED, 0.9)),
            Feedback::Wrong(_) | Feedback::AutoPlayed(_) => None,
        };
        if let Some((label, color, scale)) = text {
            // Bigger pop for big combos.
            let combo_boost = (judge.0.score().combo as f32 / 100.0).min(0.4);
            popup(
                &mut commands,
                label,
                color,
                Vec2::new(x, layout::HIT_LINE + 34.0),
                16.0,
                scale + combo_boost,
            );
        }
    });
}

fn burst(
    commands: &mut Commands,
    rng: &mut Rng,
    textures: &Textures,
    x: f32,
    hand: Hand,
    timing: Option<Timing>,
) {
    let color = match timing {
        Some(t @ Timing::Perfect) => palette::timing(t),
        _ => palette::hand_lit(hand),
    };
    let base_alpha = if timing.is_some() { 0.95 } else { 0.5 };
    let particles = (0..particle_count(timing))
        .map(|_| {
            let size = rng.range(5.0, 12.0);
            let velocity = Vec2::new(rng.range(-140.0, 140.0), rng.range(160.0, 420.0));
            (
                Particle {
                    velocity,
                    base_alpha,
                },
                Ephemeral::new(rng.range(0.35, 0.8)),
                Sprite {
                    image: textures.glow.clone(),
                    color: color.with_alpha(base_alpha),
                    custom_size: Some(Vec2::splat(size)),
                    ..default()
                },
                Transform::from_xyz(x + rng.range(-6.0, 6.0), layout::HIT_LINE + 2.0, PARTICLE_Z),
            )
        })
        .collect::<Vec<_>>();
    commands.spawn_batch(particles);
}

fn flash(commands: &mut Commands, textures: &Textures, x: f32, width: f32, color: Color) {
    commands.spawn((
        KeyFlash,
        Ephemeral::new(0.35),
        Sprite {
            image: textures.glow.clone(),
            color: color.with_alpha(0.8),
            custom_size: Some(Vec2::new(width * 3.5, 90.0)),
            ..default()
        },
        Transform::from_xyz(x, layout::HIT_LINE, FLASH_Z),
    ));
}

fn popup(commands: &mut Commands, label: &str, color: Color, at: Vec2, size: f32, scale: f32) {
    commands.spawn((
        Popup {
            rise: 46.0,
            origin_y: at.y,
            pop: scale,
        },
        Ephemeral::new(0.75),
        Text2d::new(label),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
        // Keeps the text readable over bars of a similar colour.
        Text2dShadow {
            offset: Vec2::new(1.5, -1.5),
            color: Color::BLACK.with_alpha(0.85),
        },
        Transform::from_xyz(at.x, at.y, POPUP_Z).with_scale(Vec3::splat(scale)),
    ));
}

fn combo_milestones(mut commands: Commands, judge: Res<Judge>, mut last_combo: Local<u32>) {
    let combo = judge.0.score().combo;
    if combo > *last_combo
        && let Some(milestone) = MILESTONES
            .iter()
            .find(|m| (*last_combo + 1..=combo).contains(*m))
    {
        popup(
            &mut commands,
            &format!("{milestone} COMBO!"),
            palette::timing(Timing::Perfect),
            Vec2::new(0.0, 20.0),
            36.0,
            1.0,
        );
    }
    *last_combo = combo;
}

fn age_ephemerals(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Ephemeral)>,
) {
    let dt = time.delta_secs();
    query.iter_mut().for_each(|(entity, mut e)| {
        e.age += dt;
        if e.age >= e.lifetime {
            commands.entity(entity).despawn();
        }
    });
}

fn move_particles(
    time: Res<Time>,
    mut query: Query<(&mut Particle, &Ephemeral, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    query
        .iter_mut()
        .for_each(|(mut p, e, mut transform, mut sprite)| {
            p.velocity.y += GRAVITY * dt;
            transform.translation += (p.velocity * dt).extend(0.0);
            let fade = 1.0 - e.progress();
            sprite.color = sprite.color.with_alpha(p.base_alpha * fade);
            transform.scale = Vec3::splat(0.4 + 0.6 * fade);
        });
}

fn animate_popups(mut query: Query<(&Popup, &Ephemeral, &mut Transform, &mut TextColor)>) {
    query
        .iter_mut()
        .for_each(|(p, e, mut transform, mut color)| {
            let t = e.progress();
            // Ease out: fast rise that settles.
            let eased = 1.0 - (1.0 - t).powi(3);
            transform.translation.y = p.origin_y + p.rise * eased;
            // Pop in larger, settle to the base scale.
            let pop = 1.0 + 0.5 * (1.0 - (t * 6.0).min(1.0));
            transform.scale = Vec3::splat(p.pop * pop);
            color.0 = color.0.with_alpha((1.0 - t).powf(0.6));
        });
}

fn fade_flashes(mut query: Query<(&Ephemeral, &mut Sprite), With<KeyFlash>>) {
    query.iter_mut().for_each(|(e, mut sprite)| {
        sprite.color = sprite.color.with_alpha(0.8 * (1.0 - e.progress()).powi(2));
    });
}
