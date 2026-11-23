//! Streak "heat": the backdrop and hit line warm up as the multiplier
//! climbs, "ON FIRE!" at the top with embers drifting up, and a cool-down
//! when the streak breaks.

use bevy::prelude::*;

use super::effects::{Ephemeral, PARTICLE_Z, Particle, popup};
use super::falling::Backdrop;
use super::textures::Textures;
use super::{Judge, Phase, palette};
use crate::layout;
use crate::performance::Heat;

/// Displayed heat, 0..=1, easing towards the streak's real heat.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct HeatGlow(pub f32);

/// An ember drifting up while on fire.
#[derive(Component, Debug, Clone, Copy)]
pub struct Ember;

/// Embers spawned per second while on fire.
const EMBER_RATE: f32 = 24.0;
/// How fast the glow follows the streak (per second).
const EASE: f32 = 4.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<HeatGlow>().add_systems(
        Update,
        (ease_glow, tint_backdrop, announce, embers)
            .chain()
            .in_set(Phase::Present),
    );
}

fn ease_glow(mut glow: ResMut<HeatGlow>, judge: Res<Judge>, time: Res<Time>) {
    let target = judge.0.score().heat().intensity();
    let blend = 1.0 - (-EASE * time.delta_secs()).exp();
    glow.0 += (target - glow.0) * blend;
}

fn tint_backdrop(glow: Res<HeatGlow>, mut backdrop: Single<&mut Sprite, With<Backdrop>>) {
    backdrop.color = palette::BACKDROP_GLOW.mix(&palette::BACKDROP_FIRE, glow.0);
}

fn announce(mut commands: Commands, judge: Res<Judge>, mut last: Local<Option<Heat>>) {
    let heat = judge.0.score().heat();
    let previous = last.replace(heat).unwrap_or(Heat::Cold);
    let banner = match (previous, heat) {
        (p, Heat::OnFire) if p != Heat::OnFire => Some(("ON FIRE!", palette::FIRE)),
        (Heat::Hot | Heat::OnFire, Heat::Cold) => Some(("STREAK LOST", palette::MISSED)),
        _ => None,
    };
    if let Some((text, color)) = banner {
        popup(&mut commands, text, color, Vec2::new(0.0, 110.0), 30.0, 1.0);
    }
}

/// Deterministic sequence so ember placement varies without an RNG.
fn scatter(n: u32) -> f32 {
    ((n.wrapping_mul(2_654_435_761) >> 8) % 10_000) as f32 / 10_000.0
}

fn embers(
    mut commands: Commands,
    judge: Res<Judge>,
    time: Res<Time>,
    textures: Res<Textures>,
    mut owed: Local<f32>,
    mut count: Local<u32>,
) {
    if judge.0.score().heat() != Heat::OnFire {
        *owed = 0.0;
        return;
    }
    *owed += EMBER_RATE * time.delta_secs();
    let spawn = owed.floor() as u32;
    *owed -= spawn as f32;
    let new = (0..spawn)
        .map(|_| {
            *count += 1;
            let n = *count;
            let x = layout::LEFT + scatter(n) * layout::CANVAS_WIDTH;
            let drift = (scatter(n ^ 0x5555) - 0.5) * 40.0;
            let size = 3.0 + scatter(n ^ 0xAAAA) * 5.0;
            (
                Ember,
                // Gentle upward float; gravity is outweighed by the start speed.
                Particle::new(Vec2::new(drift, 260.0 + scatter(n ^ 0x1234) * 120.0), 0.7),
                Ephemeral::new(1.2),
                Sprite {
                    image: textures.glow.clone(),
                    color: palette::FIRE.with_alpha(0.7),
                    custom_size: Some(Vec2::splat(size)),
                    ..default()
                },
                Transform::from_xyz(x, layout::HIT_LINE, PARTICLE_Z),
            )
        })
        .collect::<Vec<_>>();
    commands.spawn_batch(new);
}
