//! Load a Standard MIDI File into a [`Song`].

use std::collections::HashMap;
use std::path::Path;

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use tracing::{debug, info};

use crate::error::Error;
use crate::music::{Clef, MidiNote};
use crate::song::{Hand, Note, Seconds, Song, Tempo};

/// Absolute position in MIDI ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Tick(u64);

/// A tempo change: from `at` onwards, each beat lasts `micros_per_beat`.
#[derive(Debug, Clone, Copy)]
struct TempoChange {
    at: Tick,
    micros_per_beat: u32,
}

const DEFAULT_MICROS_PER_BEAT: u32 = 500_000;

/// Converts ticks to seconds, honouring every tempo change.
struct TempoMap {
    ticks_per_beat: f64,
    /// Sorted by tick, always starting at tick 0.
    changes: Vec<TempoChange>,
}

impl TempoMap {
    fn new(ticks_per_beat: u16, mut changes: Vec<TempoChange>) -> Self {
        changes.sort_by_key(|c| c.at);
        if changes.first().is_none_or(|c| c.at != Tick(0)) {
            changes.insert(
                0,
                TempoChange {
                    at: Tick(0),
                    micros_per_beat: DEFAULT_MICROS_PER_BEAT,
                },
            );
        }
        Self {
            ticks_per_beat: f64::from(ticks_per_beat),
            changes,
        }
    }

    fn seconds_for_span(&self, ticks: u64, micros_per_beat: u32) -> f64 {
        ticks as f64 / self.ticks_per_beat * f64::from(micros_per_beat) / 1e6
    }

    fn to_seconds(&self, tick: Tick) -> Seconds {
        let total: f64 = self
            .changes
            .iter()
            .zip(
                self.changes
                    .iter()
                    .skip(1)
                    .map(|c| c.at)
                    .chain([Tick(u64::MAX)]),
            )
            .take_while(|(change, _)| change.at < tick)
            .map(|(change, next)| {
                self.seconds_for_span(next.min(tick).0 - change.at.0, change.micros_per_beat)
            })
            .sum();
        Seconds(total as f32)
    }

    fn initial_micros_per_beat(&self) -> u32 {
        self.changes
            .first()
            .map_or(DEFAULT_MICROS_PER_BEAT, |c| c.micros_per_beat)
    }
}

#[derive(Debug, Clone, Copy)]
struct RawNote {
    track: usize,
    pitch: MidiNote,
    start: Tick,
    end: Tick,
}

#[derive(Debug, Clone, Copy)]
enum NoteEdge {
    On,
    Off,
}

/// A single absolute-time event from any track that we care about.
#[derive(Debug, Clone, Copy)]
enum Event {
    Note {
        track: usize,
        channel: u8,
        pitch: MidiNote,
        edge: NoteEdge,
    },
    Tempo(u32),
    TimeSignature(u8),
}

fn absolute_events(smf: &Smf<'_>) -> Vec<(Tick, Event)> {
    smf.tracks
        .iter()
        .enumerate()
        .flat_map(|(track, events)| {
            events
                .iter()
                .scan(0u64, |tick, ev| {
                    *tick += u64::from(ev.delta.as_int());
                    Some((Tick(*tick), ev.kind))
                })
                .filter_map(move |(tick, kind)| {
                    let event = match kind {
                        TrackEventKind::Midi { channel, message } => {
                            let (key, edge) = match message {
                                MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                                    (key, NoteEdge::On)
                                }
                                MidiMessage::NoteOn { key, .. }
                                | MidiMessage::NoteOff { key, .. } => (key, NoteEdge::Off),
                                _ => return None,
                            };
                            Event::Note {
                                track,
                                channel: channel.as_int(),
                                pitch: MidiNote::new(key.as_int()).ok()?,
                                edge,
                            }
                        }
                        TrackEventKind::Meta(MetaMessage::Tempo(t)) => Event::Tempo(t.as_int()),
                        TrackEventKind::Meta(MetaMessage::TimeSignature(num, ..)) => {
                            Event::TimeSignature(num)
                        }
                        _ => return None,
                    };
                    Some((tick, event))
                })
        })
        .collect()
}

/// Pair note-on/note-off events into notes.
fn pair_notes(events: &[(Tick, Event)]) -> Vec<RawNote> {
    let mut open: HashMap<(usize, u8, MidiNote), Tick> = HashMap::new();
    events
        .iter()
        .filter_map(|&(tick, event)| match event {
            Event::Note {
                track,
                channel,
                pitch,
                edge: NoteEdge::On,
            } => {
                open.insert((track, channel, pitch), tick);
                None
            }
            Event::Note {
                track,
                channel,
                pitch,
                edge: NoteEdge::Off,
            } => open.remove(&(track, channel, pitch)).map(|start| RawNote {
                track,
                pitch,
                start,
                end: tick,
            }),
            Event::Tempo(_) | Event::TimeSignature(_) => None,
        })
        .collect()
}

/// Decide which hand plays each note. Files with two or more note-bearing
/// tracks are assumed to be "right hand first"; otherwise split at middle C.
fn hand_assigner(notes: &[RawNote]) -> impl Fn(&RawNote) -> Hand {
    let mut tracks: Vec<usize> = notes.iter().map(|n| n.track).collect();
    tracks.sort_unstable();
    tracks.dedup();
    let right_track = (tracks.len() >= 2).then(|| tracks[0]);
    move |note| match right_track {
        Some(t) if note.track == t => Hand::Right,
        Some(_) => Hand::Left,
        None => match Clef::for_note(note.pitch) {
            Clef::Treble => Hand::Right,
            Clef::Bass => Hand::Left,
        },
    }
}

pub fn parse(title: &str, bytes: &[u8]) -> Result<Song, Error> {
    let smf = Smf::parse(bytes)?;
    let ticks_per_beat = match smf.header.timing {
        Timing::Metrical(t) => t.as_int(),
        Timing::Timecode(..) => return Err(Error::UnsupportedTiming),
    };
    let mut events = absolute_events(&smf);
    events.sort_by_key(|(tick, _)| *tick);

    let tempo_map = TempoMap::new(
        ticks_per_beat,
        events
            .iter()
            .filter_map(|&(at, e)| match e {
                Event::Tempo(micros_per_beat) => Some(TempoChange {
                    at,
                    micros_per_beat,
                }),
                _ => None,
            })
            .collect(),
    );
    let beats_per_bar = events
        .iter()
        .find_map(|(_, e)| match e {
            Event::TimeSignature(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(4);

    let raw = pair_notes(&events);
    debug!(count = raw.len(), "paired MIDI notes");
    let hand_of = hand_assigner(&raw);
    let notes = raw
        .iter()
        .map(|n| {
            let start = tempo_map.to_seconds(n.start);
            Note {
                pitch: n.pitch,
                start,
                duration: tempo_map.to_seconds(n.end) - start,
                hand: hand_of(n),
                finger: None,
            }
        })
        .collect();
    let bpm = 60e6 / f64::from(tempo_map.initial_micros_per_beat());
    let tempo = Tempo::from_bpm(bpm as f32, beats_per_bar);
    let song = Song::new(title, tempo, notes)?;
    info!(title, notes = song.notes().len(), bpm, "loaded MIDI file");
    Ok(song)
}

pub fn load(path: &Path) -> Result<Song, Error> {
    let bytes = std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })?;
    let title = path.file_stem().map_or_else(
        || "Untitled".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    parse(&title, &bytes)
}
