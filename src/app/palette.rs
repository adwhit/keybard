use bevy::prelude::{Color, Mix};

use crate::performance::{Grade, Timing};
use crate::song::Hand;

pub const BACKGROUND: Color = Color::srgb(0.055, 0.055, 0.075);
/// Top of the falling area's backdrop gradient.
pub const BACKDROP_GLOW: Color = Color::srgba(0.25, 0.22, 0.45, 0.35);
/// What the backdrop glow becomes at the top of a streak.
pub const BACKDROP_FIRE: Color = Color::srgba(0.85, 0.35, 0.10, 0.55);
pub const FIRE: Color = Color::srgb(1.0, 0.55, 0.12);
/// Darker lanes under the black keys.
pub const BLACK_LANE: Color = Color::srgba(0.0, 0.0, 0.0, 0.28);
pub const PAPER: Color = Color::srgb(0.96, 0.94, 0.88);
pub const INK: Color = Color::srgb(0.10, 0.10, 0.12);
/// Noteheads you've played correctly.
pub const PLAYED_INK: Color = Color::srgb(0.20, 0.55, 0.32);
pub const STAFF_LINE: Color = Color::srgb(0.35, 0.35, 0.38);
pub const PLAYHEAD: Color = Color::srgb(0.90, 0.30, 0.20);
pub const GUIDE: Color = Color::srgb(0.13, 0.13, 0.17);
pub const BAR_LINE: Color = Color::srgb(0.17, 0.17, 0.22);
pub const HIT_LINE: Color = Color::srgb(1.0, 0.45, 0.30);
pub const MISSED: Color = Color::srgb(0.85, 0.22, 0.22);
pub const WHITE_KEY: Color = Color::srgb(0.97, 0.97, 0.95);
pub const BLACK_KEY: Color = Color::srgb(0.07, 0.07, 0.08);
pub const PRESSED: Color = Color::srgb(1.0, 0.65, 0.15);
pub const KEY_LABEL: Color = Color::srgb(0.45, 0.45, 0.50);
pub const HUD_TEXT: Color = Color::srgb(0.85, 0.85, 0.88);
pub const HUD_DIM: Color = Color::srgb(0.50, 0.50, 0.56);
pub const PROGRESS_TRACK: Color = Color::srgb(0.16, 0.16, 0.20);
/// The looped section on the progress strip.
pub const LOOP_REGION: Color = Color::srgba(1.0, 0.84, 0.30, 0.55);
pub const PROGRESS_FILL: Color = Color::srgb(0.55, 0.50, 0.90);

pub fn hand(hand: Hand) -> Color {
    match hand {
        Hand::Right => Color::srgb(0.30, 0.82, 0.48),
        Hand::Left => Color::srgb(0.32, 0.58, 0.98),
    }
}

/// Bars and keys while their note is sounding.
pub fn hand_lit(hand: Hand) -> Color {
    match hand {
        Hand::Right => Color::srgb(0.62, 1.0, 0.72),
        Hand::Left => Color::srgb(0.62, 0.80, 1.0),
    }
}

/// Bars that have finished playing.
pub fn hand_played(hand: Hand) -> Color {
    match hand {
        Hand::Right => Color::srgb(0.16, 0.36, 0.24),
        Hand::Left => Color::srgb(0.16, 0.26, 0.46),
    }
}

/// Per-bar accuracy cells: red (bad) through amber to green (perfect).
pub fn bar_accuracy(accuracy: Option<f32>) -> Color {
    match accuracy {
        None => PROGRESS_TRACK,
        Some(a) if a >= 0.5 => {
            Color::srgb(0.95, 0.75, 0.25).mix(&Color::srgb(0.30, 0.85, 0.45), (a - 0.5) * 2.0)
        }
        Some(a) => MISSED.mix(&Color::srgb(0.95, 0.75, 0.25), a * 2.0),
    }
}

pub fn timing(timing: Timing) -> Color {
    match timing {
        Timing::Perfect => Color::srgb(1.0, 0.84, 0.30),
        Timing::Great => Color::srgb(0.40, 0.88, 1.0),
        Timing::Good => Color::srgb(0.55, 0.92, 0.55),
    }
}

pub fn grade(grade: Grade) -> Color {
    match grade {
        Grade::S => Color::srgb(1.0, 0.84, 0.30),
        Grade::A => Color::srgb(0.40, 0.88, 1.0),
        Grade::B => Color::srgb(0.55, 0.92, 0.55),
        Grade::C => Color::srgb(0.95, 0.65, 0.30),
        Grade::D => Color::srgb(0.85, 0.35, 0.35),
    }
}
