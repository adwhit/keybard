//! Screen geometry. Everything is laid out in a fixed virtual canvas of
//! `CANVAS_WIDTH` x `CANVAS_HEIGHT` world units centred on the origin; the
//! camera scales that canvas to fit the window.

use crate::music::{KeyColor, MidiNote};
use crate::song::Seconds;

pub const CANVAS_WIDTH: f32 = 1280.0;
pub const CANVAS_HEIGHT: f32 = 800.0;
pub const LEFT: f32 = -CANVAS_WIDTH / 2.0;
pub const RIGHT: f32 = CANVAS_WIDTH / 2.0;
pub const TOP: f32 = CANVAS_HEIGHT / 2.0;
pub const BOTTOM: f32 = -CANVAS_HEIGHT / 2.0;

/// Notation panel occupies the top of the screen down to this y.
pub const NOTATION_BOTTOM: f32 = 160.0;
/// The keyboard's top edge, which is also where falling bars are "played".
pub const HIT_LINE: f32 = -240.0;
pub const WHITE_KEY_HEIGHT: f32 = HIT_LINE - BOTTOM;
pub const BLACK_KEY_HEIGHT: f32 = WHITE_KEY_HEIGHT * 0.62;
const BLACK_KEY_WIDTH_RATIO: f32 = 0.6;

/// Vertical speed of the falling bars.
pub const FALL_PIXELS_PER_SECOND: f32 = 150.0;

/// How many seconds of music are visible above the hit line.
pub fn fall_lookahead() -> Seconds {
    Seconds((NOTATION_BOTTOM - HIT_LINE) / FALL_PIXELS_PER_SECOND)
}

/// Y coordinate of a moment `ahead` seconds in the future, in the falling area.
pub fn fall_y(ahead: Seconds) -> f32 {
    HIT_LINE + ahead.0 * FALL_PIXELS_PER_SECOND
}

/// Staff geometry for the grand staff at the top.
pub mod staff {
    use crate::music::{Clef, StaffPosition};
    use crate::song::Seconds;

    /// Distance between adjacent staff positions (half a space).
    pub const STEP: f32 = 5.0;
    pub const TREBLE_BOTTOM_LINE: f32 = 290.0;
    pub const BASS_BOTTOM_LINE: f32 = 210.0;
    /// Left edge of the staff lines.
    pub const LEFT_EDGE: f32 = super::LEFT + 20.0;
    /// Notes are culled once they scroll left of this x (behind the clefs).
    pub const CLEF_AREA_RIGHT: f32 = super::LEFT + 90.0;
    /// The "now" cursor; notes scroll leftwards towards it.
    pub const PLAYHEAD_X: f32 = super::LEFT + 150.0;
    pub const PIXELS_PER_SECOND: f32 = 130.0;

    pub fn bottom_line(clef: Clef) -> f32 {
        match clef {
            Clef::Treble => TREBLE_BOTTOM_LINE,
            Clef::Bass => BASS_BOTTOM_LINE,
        }
    }

    pub fn y(clef: Clef, position: StaffPosition) -> f32 {
        bottom_line(clef) + position.0 as f32 * STEP
    }

    /// X coordinate for a moment `ahead` seconds from now (negative = past).
    pub fn x(ahead: Seconds) -> f32 {
        PLAYHEAD_X + ahead.0 * PIXELS_PER_SECOND
    }

    pub fn is_visible(x: f32) -> bool {
        (CLEF_AREA_RIGHT..super::RIGHT).contains(&x)
    }
}

/// The span of piano keys drawn, always starting on a C and ending on a B so
/// that both edges are white keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyRange {
    low: MidiNote,
    high: MidiNote,
}

/// Horizontal placement of a single key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyGeometry {
    pub center_x: f32,
    pub width: f32,
    pub color: KeyColor,
}

impl KeyRange {
    /// Smallest number of octaves we ever show.
    const MIN_OCTAVES: i16 = 4;

    /// A range covering `low..=high`, padded out to whole octaves.
    pub fn covering(low: MidiNote, high: MidiNote) -> Self {
        let low_c = i16::from(low.value()) / 12 * 12;
        let high_b = i16::from(high.value()) / 12 * 12 + 11;
        let octaves = (high_b + 1 - low_c) / 12;
        let missing = (Self::MIN_OCTAVES - octaves).max(0);
        // Grow downwards first (bass), then upwards, staying within MIDI.
        let extra_low = (missing + 1) / 2;
        let low_c = (low_c - extra_low * 12).max(0);
        let high_b = (low_c + Self::MIN_OCTAVES.max(octaves) * 12 - 1).min(127);
        let to_note = |v: i16| {
            u8::try_from(v)
                .ok()
                .and_then(|v| MidiNote::new(v).ok())
                .unwrap_or(MidiNote::MIDDLE_C)
        };
        Self {
            low: to_note(low_c),
            high: to_note(high_b),
        }
    }

    pub fn low(self) -> MidiNote {
        self.low
    }

    pub fn high(self) -> MidiNote {
        self.high
    }

    pub fn contains(self, note: MidiNote) -> bool {
        (self.low..=self.high).contains(&note)
    }

    pub fn notes(self) -> impl Iterator<Item = MidiNote> {
        self.low.range_inclusive(self.high)
    }

    fn white_keys_before(self, note: MidiNote) -> usize {
        self.low
            .range_inclusive(note)
            .filter(|n| *n != note && n.key_color() == KeyColor::White)
            .count()
    }

    pub fn white_key_count(self) -> usize {
        self.notes()
            .filter(|n| n.key_color() == KeyColor::White)
            .count()
    }

    pub fn white_key_width(self) -> f32 {
        CANVAS_WIDTH / self.white_key_count() as f32
    }

    /// Where a key sits horizontally, or `None` if it is off the keyboard.
    pub fn geometry(self, note: MidiNote) -> Option<KeyGeometry> {
        if !self.contains(note) {
            return None;
        }
        let white = self.white_key_width();
        let left_of_key = LEFT + self.white_keys_before(note) as f32 * white;
        Some(match note.key_color() {
            KeyColor::White => KeyGeometry {
                center_x: left_of_key + white / 2.0,
                width: white,
                color: KeyColor::White,
            },
            // A black key straddles the boundary after the preceding white key.
            KeyColor::Black => KeyGeometry {
                center_x: left_of_key,
                width: white * BLACK_KEY_WIDTH_RATIO,
                color: KeyColor::Black,
            },
        })
    }
}
