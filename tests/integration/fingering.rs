use keybard::builtin::BuiltinSong;
use keybard::fingering::Finger;
use keybard::song::{Hand, Song, Tempo, parse_voice};

fn fingers(voice: &str, hand: Hand) -> Vec<u8> {
    let tempo = Tempo::from_bpm(120.0, 4);
    let song = Song::new("f", tempo, parse_voice(voice, hand, tempo).unwrap()).unwrap();
    song.iter()
        .map(|(id, _)| song.finger(id).number())
        .collect()
}

#[test]
fn five_finger_position() {
    assert_eq!(fingers("C4 D4 E4 F4 G4", Hand::Right), vec![1, 2, 3, 4, 5]);
    assert_eq!(fingers("G4 F4 E4 D4 C4", Hand::Right), vec![5, 4, 3, 2, 1]);
}

#[test]
fn the_left_hand_is_mirrored() {
    assert_eq!(fingers("C3 D3 E3 F3 G3", Hand::Left), vec![5, 4, 3, 2, 1]);
}

#[test]
fn thumb_passes_under_on_a_scale() {
    assert_eq!(
        fingers("C4 D4 E4 F4 G4 A4 B4 C5", Hand::Right),
        vec![1, 2, 3, 1, 2, 3, 4, 5]
    );
}

#[test]
fn fingers_cross_over_the_thumb_coming_down() {
    assert_eq!(
        fingers("C5 B4 A4 G4 F4 E4 D4 C4", Hand::Right),
        vec![5, 4, 3, 2, 1, 3, 2, 1]
    );
}

#[test]
fn ode_to_joy_sits_in_c_position() {
    assert_eq!(
        fingers("E4 E4 F4 G4 G4 F4 E4 D4 C4 C4 D4 E4 E4 D4 D4", Hand::Right),
        vec![3, 3, 4, 5, 5, 4, 3, 2, 1, 1, 2, 3, 3, 2, 2]
    );
}

#[test]
fn twinkle_reaches_up_with_four_then_five() {
    assert_eq!(
        fingers("C4 C4 G4 G4 A4 A4 G4", Hand::Right),
        vec![1, 1, 4, 4, 5, 5, 4]
    );
}

#[test]
fn chords_spread_across_the_hand() {
    // The triad (sorted by pitch) takes 1-3-5.
    assert_eq!(fingers("C4+E4+G4", Hand::Right), vec![1, 3, 5]);
    // Left hand: lowest note gets the pinky.
    assert_eq!(fingers("C3+G3", Hand::Left), vec![5, 1]);
}

#[test]
fn every_note_of_every_builtin_gets_a_finger() {
    use clap::ValueEnum;
    BuiltinSong::value_variants().iter().for_each(|b| {
        let song = b.load().unwrap();
        song.iter().for_each(|(id, _)| {
            let n = song.finger(id).number();
            assert!((1..=5).contains(&n), "{b:?}");
        });
    });
}

#[test]
fn finger_numbers_round_trip() {
    assert_eq!(Finger::Thumb.number(), 1);
    assert_eq!(Finger::Pinky.number(), 5);
    assert_eq!(Finger::ALL.len(), 5);
}

#[test]
fn annotations_pin_fingers_and_the_rest_adapts() {
    // Unannotated, C D E F G takes 1-2-3-4-5. Pin E to 1 and the hand must
    // shift: the notes after it follow on from the thumb.
    let pinned = fingers("C4 D4 E4@1 F4 G4", Hand::Right);
    assert_eq!(pinned[2], 1, "the pin holds");
    assert_eq!(pinned[3..], [2, 3], "the hand carries on from the thumb");
    // ...and the thumb passes under from the middle finger, the classic pivot.
    assert_eq!(pinned[..2], [2, 3]);
    // Chord members can be pinned individually.
    assert_eq!(fingers("C4@1+E4@2+G4", Hand::Right)[..2], [1, 2]);
}

#[test]
fn annotations_parse_with_durations_and_reject_bad_fingers() {
    use keybard::song::parse_voice;
    let tempo = Tempo::from_bpm(60.0, 4);
    let notes = parse_voice("E4@3/2 C4", Hand::Right, tempo).unwrap();
    assert_eq!(notes[0].finger, Some(Finger::Middle));
    assert!((notes[0].duration.0 - 2.0).abs() < 1e-6);
    assert_eq!(notes[1].finger, None);
    assert!(parse_voice("E4@6", Hand::Right, tempo).is_err());
    assert!(parse_voice("E4@x", Hand::Right, tempo).is_err());
}
