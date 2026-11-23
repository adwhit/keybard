use keybard::Error;
use keybard::builtin::BuiltinSong;
use keybard::music::MidiNote;
use keybard::song::{Hand, Note, Seconds, Song, Tempo, parse_pitch, parse_voice};

fn n(v: u8) -> MidiNote {
    MidiNote::new(v).expect("valid note")
}

fn approx(a: Seconds, b: f32) -> bool {
    (a.0 - b).abs() < 1e-4
}

#[test]
fn parses_pitches() {
    assert_eq!(parse_pitch("C4").unwrap(), n(60));
    assert_eq!(parse_pitch("c4").unwrap(), n(60));
    assert_eq!(parse_pitch("F#3").unwrap(), n(54));
    assert_eq!(parse_pitch("A0").unwrap(), n(21));
    assert_eq!(parse_pitch("C-1").unwrap(), n(0));
    for bad in ["", "H4", "C", "C#", "Cx4", "B#9"] {
        assert!(parse_pitch(bad).is_err(), "{bad} should fail");
    }
}

#[test]
fn parses_voice_timing_rests_and_chords() {
    let tempo = Tempo::from_bpm(120.0, 4); // half a second per beat
    let notes = parse_voice("C4 | E4/0.5 R/1.5 C3+G3/2", Hand::Left, tempo).unwrap();
    let summary: Vec<(u8, f32, f32)> = notes
        .iter()
        .map(|n| (n.pitch.value(), n.start.0, n.duration.0))
        .collect();
    assert_eq!(
        summary,
        vec![
            (60, 0.0, 0.5),
            (64, 0.5, 0.25),
            (48, 1.5, 1.0),
            (55, 1.5, 1.0)
        ]
    );
    assert!(notes.iter().all(|n| n.hand == Hand::Left));
}

#[test]
fn voice_errors_name_the_token() {
    let tempo = Tempo::from_bpm(120.0, 4);
    match parse_voice("C4 Q4", Hand::Right, tempo) {
        Err(Error::InvalidNoteToken(t)) => assert_eq!(t, "Q4"),
        other => panic!("unexpected {other:?}"),
    }
    assert!(parse_voice("C4/x", Hand::Right, tempo).is_err());
}

#[test]
fn songs_sort_notes_and_report_extent() {
    let tempo = Tempo::from_bpm(60.0, 4);
    let note = |pitch, start, duration| Note {
        pitch: n(pitch),
        start: Seconds(start),
        duration: Seconds(duration),
        hand: Hand::Right,
    };
    let song = Song::new(
        "t",
        tempo,
        vec![note(72, 2.0, 1.0), note(60, 0.0, 4.0), note(55, 0.0, 1.0)],
    )
    .unwrap();
    let order: Vec<u8> = song.notes().iter().map(|n| n.pitch.value()).collect();
    assert_eq!(order, vec![55, 60, 72]);
    assert!(approx(song.duration(), 4.0));
    assert_eq!(song.pitch_range(), (n(55), n(72)));
    assert!(song.notes()[1].is_sounding_at(Seconds(3.9)));
    assert!(!song.notes()[1].is_sounding_at(Seconds(4.0)));
}

#[test]
fn empty_songs_are_rejected() {
    let tempo = Tempo::from_bpm(60.0, 4);
    assert!(matches!(
        Song::new("t", tempo, vec![]),
        Err(Error::EmptySong)
    ));
}

#[test]
fn tempo_conversions() {
    let tempo = Tempo::from_bpm(100.0, 3);
    assert!(approx(tempo.seconds_per_beat, 0.6));
    assert!(approx(tempo.bar_length(), 1.8));
}

#[test]
fn builtin_songs_load_with_both_hands() {
    [
        BuiltinSong::OdeToJoy,
        BuiltinSong::Twinkle,
        BuiltinSong::Scale,
    ]
    .into_iter()
    .for_each(|b| {
        let song = b.load().unwrap_or_else(|e| panic!("{b:?}: {e}"));
        assert!(song.notes().iter().any(|n| n.hand == Hand::Left));
        assert!(song.notes().iter().any(|n| n.hand == Hand::Right));
    });
}

#[test]
fn ode_to_joy_is_sixteen_bars() {
    let song = BuiltinSong::OdeToJoy.load().unwrap();
    let bars = song.duration().0 / song.tempo.bar_length().0;
    assert!((bars - 16.0).abs() < 1e-3, "got {bars} bars");
    let right_end = song
        .notes()
        .iter()
        .filter(|n| n.hand == Hand::Right)
        .map(|n| n.end().0)
        .fold(0.0, f32::max);
    assert!((right_end - song.duration().0).abs() < 1e-3);
}
