use keybard::music::MidiNote;
use keybard::performance::{
    Grade, HIT_WINDOW, Hands, NoteStatus, Performance, PlayMode, PressOutcome, Score, Timing,
};
use keybard::song::{Hand, NoteId, Seconds, Song, Tempo, parse_voice};

fn n(v: u8) -> MidiNote {
    MidiNote::new(v).expect("valid note")
}

fn song_from(voice: &str) -> Song {
    let tempo = Tempo::from_bpm(60.0, 4);
    Song::new(
        "test",
        tempo,
        parse_voice(voice, Hand::Right, tempo).unwrap(),
    )
    .unwrap()
}

/// C4 at 0s, E4 at 1s, G4 at 2s (one beat per second).
fn song() -> Song {
    song_from("C4 E4 G4")
}

fn hit(outcome: PressOutcome) -> (NoteId, Timing) {
    match outcome {
        PressOutcome::Hit(j) => (j.note, j.timing),
        PressOutcome::Wrong => panic!("expected a hit"),
    }
}

#[test]
fn timing_tiers_by_offset() {
    assert_eq!(Timing::from_offset(Seconds(0.0)), Some(Timing::Perfect));
    assert_eq!(Timing::from_offset(Seconds(-0.05)), Some(Timing::Perfect));
    assert_eq!(Timing::from_offset(Seconds(0.08)), Some(Timing::Great));
    assert_eq!(Timing::from_offset(Seconds(-0.15)), Some(Timing::Good));
    assert_eq!(Timing::from_offset(Seconds(HIT_WINDOW.0 + 0.01)), None);
}

#[test]
fn listen_mode_auto_plays_each_note_once_without_scoring() {
    let song = song();
    let mut perf = Performance::new(&song);
    let a = perf.advance(&song, PlayMode::Listen, Seconds(-1.0), Seconds(0.5));
    assert_eq!(a.auto_played, vec![NoteId(0)]);
    assert_eq!(a.time, Seconds(0.5));
    let b = perf.advance(&song, PlayMode::Listen, Seconds(0.5), Seconds(0.9));
    assert!(b.auto_played.is_empty());
    let c = perf.advance(&song, PlayMode::Listen, Seconds(0.9), Seconds(5.0));
    assert_eq!(c.auto_played, vec![NoteId(1), NoteId(2)]);
    assert_eq!(perf.status(NoteId(2)), NoteStatus::AutoPlayed);
    assert_eq!(perf.score(), &Score::default());
}

#[test]
fn wait_mode_stops_at_each_note_until_played() {
    let song = song();
    let mut perf = Performance::new(&song);
    let a = perf.advance(&song, PlayMode::Wait, Seconds(-1.0), Seconds(0.5));
    assert_eq!(a.time, Seconds(0.0), "gated at the first note");
    let b = perf.advance(&song, PlayMode::Wait, a.time, Seconds(0.3));
    assert_eq!(b.time, Seconds(0.0), "still waiting");

    assert_eq!(hit(perf.press(&song, n(60), b.time)).0, NoteId(0));
    let c = perf.advance(&song, PlayMode::Wait, b.time, Seconds(1.5));
    assert_eq!(c.time, Seconds(1.0), "released, then gated at E4");
}

#[test]
fn wait_mode_grades_by_how_long_it_waited() {
    let song = song();
    let mut perf = Performance::new(&song);
    // Arrive exactly on time and press immediately: perfect.
    perf.advance(&song, PlayMode::Wait, Seconds(-0.5), Seconds(0.0));
    assert_eq!(
        hit(perf.press(&song, n(60), Seconds(0.0))).1,
        Timing::Perfect
    );
    // Arrive at E4 but dawdle for a long time: still a hit, but only Good.
    let t = perf
        .advance(&song, PlayMode::Wait, Seconds(0.0), Seconds(1.2))
        .time;
    let t = perf.advance(&song, PlayMode::Wait, t, Seconds(3.0)).time;
    assert_eq!(t, Seconds(1.0));
    let (id, timing) = hit(perf.press(&song, n(64), t));
    assert_eq!((id, timing), (NoteId(1), Timing::Good));
    // The stall doesn't leak into the next note.
    let t = perf.advance(&song, PlayMode::Wait, t, Seconds(2.0)).time;
    assert_eq!(hit(perf.press(&song, n(67), t)).1, Timing::Perfect);
}

#[test]
fn wait_mode_waits_for_every_note_of_a_chord() {
    let song = song_from("C4+E4 G4");
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
    let a = perf.advance(&song, PlayMode::Play, Seconds(0.1), t);
    assert_eq!(a.missed, vec![NoteId(0)]);
    assert_eq!(perf.status(NoteId(0)), NoteStatus::Missed);
    assert_eq!(perf.press(&song, n(60), t), PressOutcome::Wrong, "too late");
    let score = perf.score();
    assert_eq!((score.missed, score.wrong), (1, 1));
}

#[test]
fn presses_are_judged_by_pitch_and_window() {
    let song = song();
    let mut perf = Performance::new(&song);
    assert_eq!(perf.press(&song, n(62), Seconds(0.0)), PressOutcome::Wrong);
    assert_eq!(perf.press(&song, n(64), Seconds(0.0)), PressOutcome::Wrong);
    let judged = match perf.press(&song, n(64), Seconds(0.88)) {
        PressOutcome::Hit(j) => j,
        PressOutcome::Wrong => panic!("should hit"),
    };
    assert_eq!(judged.note, NoteId(1));
    assert_eq!(judged.timing, Timing::Great);
    assert!((judged.offset.0 + 0.12).abs() < 1e-5, "12ms early");
    assert_eq!(perf.press(&song, n(64), Seconds(0.9)), PressOutcome::Wrong);
    assert_eq!(perf.status(NoteId(1)), NoteStatus::Hit(Timing::Great));
}

#[test]
fn combos_grow_multiply_points_and_break() {
    let song = song_from(&"C4 ".repeat(25));
    let mut perf = Performance::new(&song);
    (0..12).for_each(|i| {
        hit(perf.press(&song, n(60), Seconds(i as f32)));
    });
    let score = perf.score().clone();
    assert_eq!((score.combo, score.max_combo, score.perfect), (12, 12, 12));
    // 9 hits at x1, then x2 from the 10th hit on.
    assert_eq!(score.points, 9 * 300 + 3 * 600);
    assert_eq!(score.multiplier(), 2);

    perf.press(&song, n(61), Seconds(12.0));
    assert_eq!(perf.score().combo, 0, "wrong notes break the combo");
    assert_eq!(perf.score().max_combo, 12);

    hit(perf.press(&song, n(60), Seconds(12.0)));
    perf.advance(&song, PlayMode::Play, Seconds(12.0), Seconds(13.5));
    assert_eq!(perf.score().combo, 0, "misses break the combo");
}

#[test]
fn multiplier_is_capped() {
    let score = Score {
        combo: 500,
        ..Score::default()
    };
    assert_eq!(score.multiplier(), 4);
}

#[test]
fn accuracy_and_grades() {
    assert_eq!(Score::default().accuracy(), None);
    assert_eq!(Score::default().grade(), None);
    let score = |perfect, great, good, missed| Score {
        perfect,
        great,
        good,
        missed,
        ..Score::default()
    };
    assert_eq!(score(10, 0, 0, 0).accuracy(), Some(1.0));
    assert_eq!(score(10, 0, 0, 0).grade(), Some(Grade::S));
    assert_eq!(score(9, 0, 0, 1).grade(), Some(Grade::A));
    assert_eq!(score(0, 10, 0, 0).grade(), Some(Grade::B));
    assert_eq!(score(0, 0, 10, 0).grade(), Some(Grade::D));
    assert_eq!(score(5, 0, 0, 5).grade(), Some(Grade::C));
    assert!(score(10, 0, 0, 0).is_full_combo());
    assert!(!score(9, 0, 0, 1).is_full_combo());
}

#[test]
fn seeking_skips_unplayed_notes_and_rearms_later_ones() {
    let song = song();
    let mut perf = Performance::new(&song);
    perf.press(&song, n(60), Seconds(0.0));
    perf.seek(&song, Seconds(1.5));
    assert_eq!(perf.status(NoteId(0)), NoteStatus::Hit(Timing::Perfect));
    assert_eq!(perf.status(NoteId(1)), NoteStatus::Skipped);
    assert_eq!(perf.status(NoteId(2)), NoteStatus::Pending);
    let a = perf.advance(&song, PlayMode::Wait, Seconds(1.5), Seconds(1.8));
    assert_eq!(a.time, Seconds(1.8), "wait mode ignores skipped notes");

    perf.seek(&song, Seconds(0.0));
    assert_eq!(perf.status(NoteId(0)), NoteStatus::Pending);
    perf.reset(&song);
    assert_eq!(perf.score(), &Score::default());
}

#[test]
fn modes_cycle() {
    assert_eq!(PlayMode::Listen.next(), PlayMode::Wait);
    assert_eq!(PlayMode::Wait.next(), PlayMode::Play);
    assert_eq!(PlayMode::Play.next(), PlayMode::Listen);
}

/// Right hand C4 E4 at 0s and 1s; left hand C3 at 0s and G2 at 1s.
fn two_hands() -> Song {
    let tempo = Tempo::from_bpm(60.0, 4);
    let notes = parse_voice("C4 E4", Hand::Right, tempo)
        .unwrap()
        .into_iter()
        .chain(parse_voice("C3 G2", Hand::Left, tempo).unwrap())
        .collect();
    Song::new("hands", tempo, notes).unwrap()
}

#[test]
fn hands_cycle_and_include() {
    assert!(Hands::Both.includes(Hand::Left) && Hands::Both.includes(Hand::Right));
    assert!(Hands::Right.includes(Hand::Right) && !Hands::Right.includes(Hand::Left));
    assert_eq!(Hands::Both.next(), Hands::Right);
    assert_eq!(Hands::Right.next(), Hands::Left);
    assert_eq!(Hands::Left.next(), Hands::Both);
}

#[test]
fn the_other_hand_plays_itself_in_wait_mode() {
    let song = two_hands();
    let mut perf = Performance::new(&song);
    perf.set_hands(Hands::Right);
    let a = perf.advance(&song, PlayMode::Wait, Seconds(-1.0), Seconds(0.5));
    assert_eq!(a.time, Seconds(0.0), "still waits for the right hand");
    let c3 = song.iter().find(|(_, note)| note.pitch == n(48)).unwrap().0;
    assert_eq!(a.auto_played, vec![c3], "left hand sounded by the computer");
    assert_eq!(perf.status(c3), NoteStatus::AutoPlayed);

    // Pressing the left-hand note is not a hit: it isn't yours.
    assert_eq!(perf.press(&song, n(48), Seconds(0.0)), PressOutcome::Wrong);
    hit(perf.press(&song, n(60), Seconds(0.0)));
    let b = perf.advance(&song, PlayMode::Wait, Seconds(0.0), Seconds(0.7));
    assert_eq!(b.time, Seconds(0.7), "left hand never gates");
}

#[test]
fn the_other_hand_is_never_missed_in_play_mode() {
    let song = two_hands();
    let mut perf = Performance::new(&song);
    perf.set_hands(Hands::Left);
    let a = perf.advance(&song, PlayMode::Play, Seconds(-1.0), Seconds(3.0));
    let score = perf.score();
    assert_eq!(score.missed, 2, "only the two left-hand notes count");
    assert_eq!(a.auto_played.len(), 2, "right hand auto-played");
    assert_eq!(a.missed.len(), 2);
}

#[test]
fn listen_mode_plays_everything_whatever_the_hands() {
    let song = two_hands();
    let mut perf = Performance::new(&song);
    perf.set_hands(Hands::Left);
    let a = perf.advance(&song, PlayMode::Listen, Seconds(-1.0), Seconds(3.0));
    assert_eq!(a.auto_played.len(), 4);
}

#[test]
fn heat_follows_the_multiplier() {
    use keybard::performance::Heat;
    let heat = |combo| {
        Score {
            combo,
            ..Score::default()
        }
        .heat()
    };
    assert_eq!(heat(0), Heat::Cold);
    assert_eq!(heat(9), Heat::Cold);
    assert_eq!(heat(10), Heat::Warm);
    assert_eq!(heat(20), Heat::Hot);
    assert_eq!(heat(30), Heat::OnFire);
    assert_eq!(heat(300), Heat::OnFire);
    assert!(Heat::Cold < Heat::Warm && Heat::Hot < Heat::OnFire);
    assert_eq!(Heat::Cold.intensity(), 0.0);
    assert_eq!(Heat::OnFire.intensity(), 1.0);
}

#[test]
fn per_bar_accuracy_scores_each_bar_separately() {
    // 60bpm 2/4: 2s bars. Bar 1: C4 D4, bar 2: E4 F4, bar 3: G4 (unplayed so far).
    let tempo = Tempo::from_bpm(60.0, 2);
    let song = Song::new(
        "bars",
        tempo,
        parse_voice("C4 D4 E4 F4 G4", Hand::Right, tempo).unwrap(),
    )
    .unwrap();
    let mut perf = Performance::new(&song);
    hit(perf.press(&song, n(60), Seconds(0.0)));
    hit(perf.press(&song, n(62), Seconds(1.0)));
    hit(perf.press(&song, n(64), Seconds(2.15))); // Good
    perf.advance(&song, PlayMode::Play, Seconds(0.0), Seconds(3.5)); // F4 missed
    let bars = perf.bar_accuracy(&song);
    assert_eq!(bars.len(), 3);
    assert_eq!(bars[0], Some(1.0));
    assert!(
        (bars[1].unwrap() - 0.2).abs() < 1e-5,
        "good (0.4) + miss (0) over 2"
    );
    assert_eq!(bars[2], None, "nothing judged yet");
}
