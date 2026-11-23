use std::io::Write;

use keybard::Error;
use keybard::midi_file;
use keybard::song::Hand;
use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

const TPB: u16 = 480;

fn ev(delta: u32, kind: TrackEventKind<'static>) -> TrackEvent<'static> {
    TrackEvent {
        delta: u28::from(delta),
        kind,
    }
}

fn on(delta: u32, key: u8) -> TrackEvent<'static> {
    ev(
        delta,
        TrackEventKind::Midi {
            channel: u4::from(0),
            message: MidiMessage::NoteOn {
                key: u7::from(key),
                vel: u7::from(90),
            },
        },
    )
}

/// Note-off expressed as note-on with zero velocity, as many files do.
fn off(delta: u32, key: u8) -> TrackEvent<'static> {
    ev(
        delta,
        TrackEventKind::Midi {
            channel: u4::from(0),
            message: MidiMessage::NoteOn {
                key: u7::from(key),
                vel: u7::from(0),
            },
        },
    )
}

fn tempo(delta: u32, micros: u32) -> TrackEvent<'static> {
    ev(
        delta,
        TrackEventKind::Meta(MetaMessage::Tempo(u24::from(micros))),
    )
}

fn end() -> TrackEvent<'static> {
    ev(0, TrackEventKind::Meta(MetaMessage::EndOfTrack))
}

fn encode(timing: Timing, tracks: Vec<Vec<TrackEvent<'static>>>) -> Vec<u8> {
    let smf = Smf {
        header: Header::new(Format::Parallel, timing),
        tracks,
    };
    let mut bytes = Vec::new();
    smf.write_std(&mut bytes).expect("encode");
    bytes
}

#[test]
fn converts_ticks_to_seconds_across_tempo_changes() {
    let metrical = Timing::Metrical(u15::from(TPB));
    let tpb = u32::from(TPB);
    let conductor = vec![tempo(0, 500_000), tempo(2 * tpb, 1_000_000), end()];
    // Right hand: C4 for two beats at 120bpm, then E4 for one beat at 60bpm.
    let right = vec![on(0, 60), off(2 * tpb, 60), on(0, 64), off(tpb, 64), end()];
    // Left hand: C3 for one beat.
    let left = vec![on(0, 48), off(tpb, 48), end()];
    let song = midi_file::parse("demo", &encode(metrical, vec![conductor, right, left])).unwrap();

    let summary: Vec<(u8, f32, f32, Hand)> = song
        .notes()
        .iter()
        .map(|n| (n.pitch.value(), n.start.0, n.duration.0, n.hand))
        .collect();
    let expected = [
        (48, 0.0, 0.5, Hand::Left),
        (60, 0.0, 1.0, Hand::Right),
        (64, 1.0, 1.0, Hand::Right),
    ];
    assert_eq!(summary.len(), expected.len());
    summary.iter().zip(expected).for_each(|(got, want)| {
        assert_eq!((got.0, got.3), (want.0, want.3));
        assert!((got.1 - want.1).abs() < 1e-4, "{got:?} vs {want:?}");
        assert!((got.2 - want.2).abs() < 1e-4, "{got:?} vs {want:?}");
    });
    assert!((song.tempo.seconds_per_beat.0 - 0.5).abs() < 1e-4);
    assert_eq!(song.title, "demo");
}

#[test]
fn single_track_files_split_hands_at_middle_c() {
    let metrical = Timing::Metrical(u15::from(TPB));
    let track = vec![on(0, 60), on(0, 59), off(480, 60), off(0, 59), end()];
    let song = midi_file::parse("one", &encode(metrical, vec![track])).unwrap();
    let hands: Vec<(u8, Hand)> = song
        .notes()
        .iter()
        .map(|n| (n.pitch.value(), n.hand))
        .collect();
    assert_eq!(hands, vec![(59, Hand::Left), (60, Hand::Right)]);
}

#[test]
fn defaults_to_120_bpm_without_tempo_events() {
    let metrical = Timing::Metrical(u15::from(TPB));
    let track = vec![on(480, 60), off(480, 60), end()];
    let song = midi_file::parse("t", &encode(metrical, vec![track])).unwrap();
    assert!((song.notes()[0].start.0 - 0.5).abs() < 1e-4);
}

#[test]
fn rejects_files_without_notes_or_garbage() {
    let metrical = Timing::Metrical(u15::from(TPB));
    assert!(matches!(
        midi_file::parse("t", &encode(metrical, vec![vec![end()]])),
        Err(Error::EmptySong)
    ));
    assert!(matches!(
        midi_file::parse("t", b"not midi"),
        Err(Error::Midi(_))
    ));
}

#[test]
fn rejects_timecode_files() {
    let timecode = Timing::Timecode(midly::Fps::Fps25, 40);
    let track = vec![on(0, 60), off(40, 60), end()];
    assert!(matches!(
        midi_file::parse("t", &encode(timecode, vec![track])),
        Err(Error::UnsupportedTiming)
    ));
}

#[test]
fn loads_from_disk_and_reports_missing_files() {
    let metrical = Timing::Metrical(u15::from(TPB));
    let bytes = encode(metrical, vec![vec![on(0, 60), off(480, 60), end()]]);
    let mut file = tempfile::Builder::new().suffix(".mid").tempfile().unwrap();
    file.write_all(&bytes).unwrap();
    let song = midi_file::load(file.path()).unwrap();
    assert_eq!(song.notes().len(), 1);
    assert!(matches!(
        midi_file::load(std::path::Path::new("/nonexistent/x.mid")),
        Err(Error::Io { .. })
    ));
}

#[test]
fn note_velocities_are_kept() {
    let metrical = Timing::Metrical(u15::from(TPB));
    let loud = ev(
        0,
        TrackEventKind::Midi {
            channel: u4::from(0),
            message: MidiMessage::NoteOn {
                key: u7::from(62),
                vel: u7::from(120),
            },
        },
    );
    let track = vec![on(0, 60), off(480, 60), loud, off(480, 62), end()];
    let song = midi_file::parse("v", &encode(metrical, vec![track])).unwrap();
    let velocities: Vec<u8> = song.notes().iter().map(|n| n.velocity.value()).collect();
    assert_eq!(velocities, vec![90, 120]);
}
