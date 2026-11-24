//! Practice tools: looping a section of the song.

use crate::song::{Seconds, Tempo};

/// The 0-based first bar of the `window`-bar stretch with the lowest
/// average accuracy, ignoring bars with nothing judged. `None` when there's
/// nothing to improve.
pub fn weakest_section(accuracy: &[Option<f32>], window: usize) -> Option<usize> {
    let window = window.max(1);
    let last_start = accuracy.len().saturating_sub(window);
    (0..=last_start)
        .filter_map(|start| {
            let judged: Vec<f32> = accuracy[start..(start + window).min(accuracy.len())]
                .iter()
                .flatten()
                .copied()
                .collect();
            (!judged.is_empty()).then(|| (start, judged.iter().sum::<f32>() / judged.len() as f32))
        })
        .fold(
            None,
            |best: Option<(usize, f32)>, (start, mean)| match best {
                Some((_, best_mean)) if best_mean <= mean => best,
                _ => Some((start, mean)),
            },
        )
        .filter(|(_, mean)| *mean < 1.0 - 1e-6)
        .map(|(start, _)| start)
}

/// A bar-aligned stretch of song time to repeat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoopRegion {
    pub start: Seconds,
    pub end: Seconds,
}

impl LoopRegion {
    /// `bars` bars starting at the bar that contains `time`, clipped to
    /// `0..song_end`.
    pub fn around(tempo: Tempo, time: Seconds, bars: u8, song_end: Seconds) -> Self {
        let bar = tempo.bar_length().0;
        let first = (time.0.max(0.0) / bar).floor();
        let start = Seconds(first * bar);
        let end = Seconds(((first + f32::from(bars)) * bar).min(song_end.0));
        Self { start, end }
    }

    /// Whether `time` falls in the region (start included, end not).
    pub fn contains(self, time: Seconds) -> bool {
        time >= self.start && time < self.end
    }

    /// 1-based number of the first bar.
    pub fn first_bar(self, tempo: Tempo) -> u32 {
        (self.start.0 / tempo.bar_length().0).round() as u32 + 1
    }

    pub fn bars(self, tempo: Tempo) -> u32 {
        ((self.end.0 - self.start.0) / tempo.bar_length().0).ceil() as u32
    }

    /// Where to rewind to once `time` has run off the end, leaving `lead` to
    /// see the first notes coming.
    pub fn wrap(self, time: Seconds, lead: Seconds) -> Option<Seconds> {
        (time >= self.end).then_some(self.start - lead)
    }
}

/// Drilling a weak section from the results card starts this slow.
pub const DRILL_START_SPEED: f32 = 0.7;
const TRAINER_STEP: f32 = 0.05;

/// The speed for the next pass of a loop: a clean pass earns a step towards
/// full tempo; anything else holds. Speeds above full tempo are left alone.
pub fn trainer_speed(current: f32, clean: bool) -> f32 {
    match (clean, current < 1.0) {
        (true, true) => (current + TRAINER_STEP).min(1.0),
        _ => current,
    }
}
