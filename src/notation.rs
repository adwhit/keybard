//! Notation helpers that don't depend on rendering: where rests go.

use crate::song::{Beats, Hand, Seconds, Song};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestShape {
    /// A whole bar's silence, whatever the time signature.
    WholeBar,
    Half,
    Quarter,
    Eighth,
}

impl RestShape {
    fn beats(self) -> f32 {
        match self {
            // Not used for splitting; a whole-bar rest fills its bar.
            Self::WholeBar => f32::INFINITY,
            Self::Half => 2.0,
            Self::Quarter => 1.0,
            Self::Eighth => 0.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rest {
    pub start: Seconds,
    pub duration: Seconds,
    pub shape: RestShape,
}

const EPSILON: f32 = 1e-3;
/// Subdivisions per beat that rest positions snap to.
const GRID: f32 = 48.0;
const SPLIT_ORDER: [RestShape; 3] = [RestShape::Half, RestShape::Quarter, RestShape::Eighth];

/// Silent stretches of one hand's part, as rests that never cross a bar line.
pub fn rests(song: &Song, hand: Hand) -> Vec<Rest> {
    let tempo = song.tempo;
    // Snap to a fine rhythmic grid so float noise doesn't leak into rests.
    let beats = |s: Seconds| (tempo.to_beats(s).0 * GRID).round() / GRID;
    let bar = f32::from(tempo.beats_per_bar);
    let end = beats(song.duration());

    let mut spans: Vec<(f32, f32)> = song
        .notes()
        .iter()
        .filter(|n| n.hand == hand)
        .map(|n| (beats(n.start), beats(n.end())))
        .collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Walk the sounding spans, collecting the gaps between them.
    let (mut gaps, covered_to) = spans.iter().fold(
        (Vec::new(), 0.0_f32),
        |(mut gaps, cursor), &(start, stop)| {
            if start > cursor + EPSILON {
                gaps.push((cursor, start));
            }
            (gaps, cursor.max(stop))
        },
    );
    if end > covered_to + EPSILON {
        gaps.push((covered_to, end));
    }

    gaps.into_iter()
        .flat_map(|(from, to)| split_at_bars(from, to, bar))
        .flat_map(|(from, to)| shapes_for(from, to, bar))
        .map(|(start, length, shape)| Rest {
            start: tempo.to_seconds(Beats(start)),
            duration: tempo.to_seconds(Beats(length)),
            shape,
        })
        .collect()
}

fn split_at_bars(from: f32, to: f32, bar: f32) -> Vec<(f32, f32)> {
    let mut pieces = Vec::new();
    let mut start = from;
    while to - start > EPSILON {
        let next_bar = ((start + EPSILON) / bar).floor() * bar + bar;
        let stop = next_bar.min(to);
        pieces.push((start, stop));
        start = stop;
    }
    pieces
}

/// Rest symbols filling `from..to` (within one bar), largest first.
fn shapes_for(from: f32, to: f32, bar: f32) -> Vec<(f32, f32, RestShape)> {
    let starts_bar = (from / bar - (from / bar).round()).abs() < EPSILON;
    if starts_bar && (to - from - bar).abs() < EPSILON {
        return vec![(from, bar, RestShape::WholeBar)];
    }
    let mut out = Vec::new();
    let mut cursor = from;
    while let Some(shape) = SPLIT_ORDER
        .iter()
        .find(|s| to - cursor >= s.beats() - EPSILON)
    {
        out.push((cursor, shape.beats(), *shape));
        cursor += shape.beats();
    }
    out
}

/// Notes shorter than this many beats are flagged/beamed.
const BEAMABLE: f32 = 0.75;

/// Groups of notes (in one hand) joined by a beam: consecutive eighths or
/// shorter within the same beat, with no gap between them. Chords count as
/// one event; a group needs at least two events. Ids are in song order.
pub fn beams(song: &Song, hand: Hand) -> Vec<Vec<crate::song::NoteId>> {
    let tempo = song.tempo;
    let beats = |s: Seconds| (tempo.to_beats(s).0 * GRID).round() / GRID;
    // Events: (start, end, ids) for each distinct start time in this hand.
    let mut events: Vec<(f32, f32, Vec<crate::song::NoteId>)> = Vec::new();
    song.iter()
        .filter(|(_, n)| n.hand == hand)
        .for_each(|(id, n)| {
            let (start, end) = (beats(n.start), beats(n.end()));
            match events.last_mut() {
                Some(event) if (event.0 - start).abs() < EPSILON => {
                    event.1 = event.1.max(end);
                    event.2.push(id);
                }
                _ => events.push((start, end, vec![id])),
            }
        });
    let short = |e: &(f32, f32, Vec<crate::song::NoteId>)| e.1 - e.0 < BEAMABLE;

    let mut groups: Vec<Vec<&(f32, f32, Vec<crate::song::NoteId>)>> = Vec::new();
    events.iter().for_each(|event| {
        let joins = groups.last().and_then(|g| g.last()).is_some_and(|prev| {
            short(prev)
                && short(event)
                && (prev.1 - event.0).abs() < EPSILON
                && (prev.0 + EPSILON).floor() == (event.0 + EPSILON).floor()
        });
        if joins {
            if let Some(group) = groups.last_mut() {
                group.push(event);
            }
        } else {
            groups.push(vec![event]);
        }
    });
    groups
        .into_iter()
        .filter(|g| g.len() >= 2)
        .map(|g| g.iter().flat_map(|e| e.2.iter().copied()).collect())
        .collect()
}
