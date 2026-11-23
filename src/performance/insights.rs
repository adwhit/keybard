//! Coaching insights for the results card: per-hand accuracy, timing
//! tendency and the notes missed most.

use crate::music::MidiNote;
use crate::song::Seconds;

/// Average timing beyond this counts as consistently early or late.
const LEAN: Seconds = Seconds(0.02);
/// How many trouble notes to report.
pub const MOST_MISSED: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tendency {
    Early,
    OnTime,
    Late,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Insights {
    pub right_accuracy: Option<f32>,
    pub left_accuracy: Option<f32>,
    /// Average offset of hits (negative = early).
    pub mean_offset: Option<Seconds>,
    /// Pitches missed most often, worst first, with their miss counts.
    pub most_missed: Vec<(MidiNote, usize)>,
}

impl Insights {
    /// Short coaching lines for the results card.
    pub fn summary(&self) -> Vec<String> {
        let percent = |a: f32| format!("{:.0}%", a * 100.0);
        let hands = [
            ("Right hand", self.right_accuracy),
            ("Left hand", self.left_accuracy),
        ]
        .into_iter()
        .filter_map(|(name, accuracy)| accuracy.map(|a| format!("{name} {}", percent(a))))
        .collect::<Vec<_>>()
        .join(" | ");
        let ms = |s: Seconds| (s.0.abs() * 1000.0).round() as u32;
        let timing = self.mean_offset.map(|m| match self.tendency() {
            Tendency::Early => format!("You rush: hits average {} ms early", ms(m)),
            Tendency::Late => format!("You drag: hits average {} ms late", ms(m)),
            Tendency::OnTime => format!("Steady timing: {} ms average", ms(m)),
        });
        let missed = (!self.most_missed.is_empty()).then(|| {
            let list = self
                .most_missed
                .iter()
                .map(|(pitch, count)| format!("{pitch} x{count}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("Most missed: {list}")
        });
        [(!hands.is_empty()).then_some(hands), timing, missed]
            .into_iter()
            .flatten()
            .collect()
    }

    pub fn tendency(&self) -> Tendency {
        match self.mean_offset {
            Some(m) if m.0 < -LEAN.0 => Tendency::Early,
            Some(m) if m.0 > LEAN.0 => Tendency::Late,
            _ => Tendency::OnTime,
        }
    }
}
