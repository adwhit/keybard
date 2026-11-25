//! Fira Sans for every piece of text: the regular face replaces Bevy's
//! default (monospace) font, and the bold face is for headings and big
//! numbers. Both are compiled in, so nothing is loaded from disk.

use bevy::asset::uuid_handle;
use bevy::prelude::*;
use bevy::text::{FontFeatureTag, FontFeatures, FontSource};
use tracing::warn;

const REGULAR_DATA: &[u8] = include_bytes!("../../assets/fonts/FiraSans-Regular.ttf");
const BOLD_DATA: &[u8] = include_bytes!("../../assets/fonts/FiraSans-Bold.ttf");

/// The bold face.
pub const BOLD: Handle<Font> = uuid_handle!("7855a5d1-3a3f-4cb2-9832-72db76ff5563");

pub fn plugin(app: &mut App) {
    let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
    let inserted = fonts
        .insert(AssetId::default(), Font::from_bytes(REGULAR_DATA.to_vec()))
        .and_then(|_| fonts.insert(BOLD.id(), Font::from_bytes(BOLD_DATA.to_vec())));
    if let Err(err) = inserted {
        warn!(?err, "could not install fonts");
    }
}

/// Regular text at `size`.
pub fn regular(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

/// Bold text at `size`.
pub fn bold(size: f32) -> TextFont {
    TextFont {
        font: FontSource::Handle(BOLD),
        ..regular(size)
    }
}

/// Bold, with every digit the same width so counting numbers don't wobble.
pub fn numbers(size: f32) -> TextFont {
    TextFont {
        font_features: FontFeatures::builder()
            .enable(FontFeatureTag::TABULAR_FIGURES)
            .build(),
        ..bold(size)
    }
}
