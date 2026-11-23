//! Procedurally generated textures, so the repo needs no image assets.

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::{BorderRect, SpriteImageMode, TextureSlicer};

#[derive(Resource, Clone)]
pub struct Textures {
    /// White rounded rectangle with a soft darker rim; 9-slice it.
    pub rounded: Handle<Image>,
    /// Radial white glow fading to transparent.
    pub glow: Handle<Image>,
    /// Vertical gradient, opaque white at the bottom fading out at the top.
    pub fade_up: Handle<Image>,
}

const ROUNDED_SIZE: u32 = 32;
/// Corner radius of the rounded texture, in texels (and in world units when
/// sliced, since slices are drawn at 1:1).
pub const ROUNDED_RADIUS: f32 = 6.0;

impl Textures {
    /// Sprite mode that keeps the rounded corners crisp at any size.
    pub fn rounded_mode() -> SpriteImageMode {
        SpriteImageMode::Sliced(TextureSlicer {
            border: BorderRect::all(ROUNDED_RADIUS + 2.0),
            ..default()
        })
    }
}

fn image(width: u32, height: u32, pixel: impl Fn(f32, f32) -> [u8; 4]) -> Image {
    let data = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| {
            // Sample at texel centres, normalised to 0..1.
            pixel(
                (x as f32 + 0.5) / width as f32,
                (y as f32 + 0.5) / height as f32,
            )
        })
        .collect();
    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn to_byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Signed distance from a point to a rounded box centred at the origin.
fn rounded_box_distance(p: Vec2, half: Vec2, radius: f32) -> f32 {
    let q = p.abs() - half + Vec2::splat(radius);
    q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - radius
}

fn rounded() -> Image {
    let size = ROUNDED_SIZE as f32;
    image(ROUNDED_SIZE, ROUNDED_SIZE, |u, v| {
        let p = Vec2::new(u, v) * size - Vec2::splat(size / 2.0);
        let d = rounded_box_distance(p, Vec2::splat(size / 2.0 - 0.5), ROUNDED_RADIUS);
        let alpha = (0.5 - d).clamp(0.0, 1.0);
        // Slightly darker rim gives each bar a defined edge.
        let shade = if d > -1.5 { 0.72 } else { 1.0 };
        let s = to_byte(shade);
        [s, s, s, to_byte(alpha)]
    })
}

fn glow() -> Image {
    image(64, 64, |u, v| {
        let d = Vec2::new(u - 0.5, v - 0.5).length() * 2.0;
        let a = (1.0 - d).clamp(0.0, 1.0).powf(2.2);
        [255, 255, 255, to_byte(a)]
    })
}

fn fade_up() -> Image {
    // Row 0 is the top of the image.
    image(1, 64, |_, v| [255, 255, 255, to_byte(v.powf(1.6))])
}

pub fn plugin(app: &mut App) {
    app.add_systems(PreStartup, create);
}

fn create(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(Textures {
        rounded: images.add(rounded()),
        glow: images.add(glow()),
        fade_up: images.add(fade_up()),
    });
}
