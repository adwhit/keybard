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
