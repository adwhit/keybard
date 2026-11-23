use keybard::music::MidiNote;
use keybard::performance::{HIT_WINDOW, NoteStatus, Performance, PlayMode, Score};
use keybard::song::{Hand, NoteId, Seconds, Song, Tempo, parse_voice};

fn n(v: u8) -> MidiNote {
    MidiNote::new(v).expect("valid note")
}

/// C4 at 0s, E4 at 1s, G4 at 2s (one beat per second).
fn song() -> Song {
    let tempo = Tempo::from_bpm(60.0, 4);
    Song::new(
        "test",
        tempo,
        parse_voice("C4 E4 G4", Hand::Right, tempo).unwrap(),
    )
    .unwrap()
}

#[test]
fn listen_mode_auto_plays_each_note_once() {
    let song = song();
    let mut perf = Performance::new(&song);
    let a = perf.advance(&song, PlayMode::Listen, Seconds(-1.0), Seconds(0.5));
    assert_eq!(a.auto_played, vec![NoteId(0)]);
    assert_eq!(a.time, Seconds(0.5));
    let b = perf.advance(&song, PlayMode::Listen, Seconds(0.5), Seconds(0.9));
    assert!(b.auto_played.is_empty());
    let c = perf.advance(&song, PlayMode::Listen, Seconds(0.9), Seconds(5.0));
    assert_eq!(c.auto_played, vec![NoteId(1), NoteId(2)]);
    assert_eq!(perf.status(NoteId(2)), NoteStatus::Hit);
}

#[test]
fn wait_mode_stops_at_each_note_until_played() {
    let song = song();
    let mut perf = Performance::new(&song);
    let a = perf.advance(&song, PlayMode::Wait, Seconds(-1.0), Seconds(0.5));
    assert_eq!(a.time, Seconds(0.0), "gated at the first note");
    let b = perf.advance(&song, PlayMode::Wait, a.time, Seconds(0.3));
    assert_eq!(b.time, Seconds(0.0), "still waiting");
    assert!(b.auto_played.is_empty());

    assert_eq!(perf.press(&song, n(60), b.time), Some(NoteId(0)));
    let c = perf.advance(&song, PlayMode::Wait, b.time, Seconds(1.5));
    assert_eq!(c.time, Seconds(1.0), "released, then gated at E4");
}

#[test]
fn wait_mode_waits_for_every_note_of_a_chord() {
    let tempo = Tempo::from_bpm(60.0, 4);
    let song = Song::new(
        "chord",
        tempo,
        parse_voice("C4+E4 G4", Hand::Right, tempo).unwrap(),
    )
    .unwrap();
    let mut perf = Performance::new(&song);
    let t = perf
        .advance(&song, PlayMode::Wait, Seconds(-1.0), Seconds(0.5))
        .time;
    perf.press(&song, n(60), t);
    assert_eq!(
        perf.advance(&song, PlayMode::Wait, t, Seconds(0.5)).time,
        Seconds(0.0)
    );
    perf.press(&song, n(64), t);
    assert_eq!(
        perf.advance(&song, PlayMode::Wait, t, Seconds(0.5)).time,
        Seconds(0.5)
    );
}

#[test]
fn play_mode_misses_notes_after_the_window() {
    let song = song();
    let mut perf = Performance::new(&song);
    perf.advance(&song, PlayMode::Play, Seconds(-1.0), Seconds(0.1));
    assert_eq!(
        perf.status(NoteId(0)),
        NoteStatus::Pending,
        "still in window"
    );
    let t = Seconds(HIT_WINDOW.0 + 0.01);
    perf.advance(&song, PlayMode::Play, Seconds(0.1), t);
    assert_eq!(perf.status(NoteId(0)), NoteStatus::Missed);
    assert_eq!(perf.press(&song, n(60), t), None, "too late now");
    assert_eq!(
        perf.score(),
        Score {
            hit: 0,
            missed: 1,
            wrong: 1
        }
    );
}

#[test]
fn presses_are_judged_by_pitch_and_window() {
    let song = song();
    let mut perf = Performance::new(&song);
    assert_eq!(perf.press(&song, n(62), Seconds(0.0)), None, "wrong pitch");
    assert_eq!(perf.press(&song, n(64), Seconds(0.0)), None, "too early");
    assert_eq!(perf.press(&song, n(64), Seconds(0.85)), Some(NoteId(1)));
    assert_eq!(perf.press(&song, n(64), Seconds(0.9)), None, "already hit");
    assert_eq!(
        perf.score(),
        Score {
            hit: 1,
            missed: 0,
            wrong: 3
        }
    );
}

#[test]
fn seeking_skips_unplayed_notes_and_rearms_later_ones() {
    let song = song();
    let mut perf = Performance::new(&song);
    perf.press(&song, n(60), Seconds(0.0));
    perf.seek(&song, Seconds(1.5));
    assert_eq!(perf.status(NoteId(0)), NoteStatus::Hit);
    assert_eq!(perf.status(NoteId(1)), NoteStatus::Skipped);
    assert_eq!(perf.status(NoteId(2)), NoteStatus::Pending);
    // Wait mode must not get stuck on a skipped note.
    let a = perf.advance(&song, PlayMode::Wait, Seconds(1.5), Seconds(1.8));
    assert_eq!(a.time, Seconds(1.8));

    perf.seek(&song, Seconds(0.0));
    assert_eq!(perf.status(NoteId(0)), NoteStatus::Pending);
    perf.reset(&song);
    assert_eq!(perf.score(), Score::default());
}

#[test]
fn modes_cycle() {
    assert_eq!(PlayMode::Listen.next(), PlayMode::Wait);
    assert_eq!(PlayMode::Wait.next(), PlayMode::Play);
    assert_eq!(PlayMode::Play.next(), PlayMode::Listen);
}
