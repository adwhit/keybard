use bevy::prelude::Color;

use crate::song::Hand;

pub const BACKGROUND: Color = Color::srgb(0.08, 0.08, 0.10);
pub const PAPER: Color = Color::srgb(0.96, 0.94, 0.88);
pub const INK: Color = Color::srgb(0.10, 0.10, 0.12);
pub const STAFF_LINE: Color = Color::srgb(0.35, 0.35, 0.38);
pub const PLAYHEAD: Color = Color::srgb(0.90, 0.30, 0.20);
pub const GUIDE: Color = Color::srgba(1.0, 1.0, 1.0, 0.06);
pub const HIT_LINE: Color = Color::srgb(0.90, 0.30, 0.20);
pub const MISSED: Color = Color::srgb(0.75, 0.20, 0.20);
pub const WHITE_KEY: Color = Color::srgb(0.97, 0.97, 0.95);
pub const BLACK_KEY: Color = Color::srgb(0.07, 0.07, 0.08);
pub const PRESSED: Color = Color::srgb(1.0, 0.65, 0.15);
pub const KEY_LABEL: Color = Color::srgb(0.45, 0.45, 0.50);
pub const HUD_TEXT: Color = Color::srgb(0.85, 0.85, 0.88);

pub fn hand(hand: Hand) -> Color {
    match hand {
        Hand::Right => Color::srgb(0.30, 0.80, 0.45),
        Hand::Left => Color::srgb(0.30, 0.55, 0.95),
    }
}

/// Dimmer variant used for bars that have already been played.
pub fn hand_played(hand: Hand) -> Color {
    match hand {
        Hand::Right => Color::srgb(0.20, 0.45, 0.28),
        Hand::Left => Color::srgb(0.20, 0.32, 0.55),
    }
}
