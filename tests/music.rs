use keybard::music::{Clef, KeyColor, MidiNote, PitchClass, StaffPosition, StemDirection};

fn n(v: u8) -> MidiNote {
    MidiNote::new(v).expect("valid note")
}

#[test]
fn rejects_out_of_range_notes() {
    assert!(MidiNote::new(128).is_err());
    assert!(MidiNote::new(127).is_ok());
}

#[test]
fn builds_from_pitch_class_and_octave() {
    assert_eq!(
        MidiNote::from_parts(PitchClass::C, 4).unwrap(),
        MidiNote::MIDDLE_C
    );
    assert_eq!(MidiNote::from_parts(PitchClass::A, 0).unwrap(), n(21));
    assert_eq!(MidiNote::from_parts(PitchClass::C, -1).unwrap(), n(0));
    assert!(MidiNote::from_parts(PitchClass::A, 9).is_err());
}

#[test]
fn names_and_colors() {
    assert_eq!(n(60).to_string(), "C4");
    assert_eq!(n(61).to_string(), "C#4");
    assert_eq!(n(21).to_string(), "A0");
    assert_eq!(n(61).key_color(), KeyColor::Black);
    assert_eq!(n(64).key_color(), KeyColor::White);
    let blacks = n(60)
        .range_inclusive(n(71))
        .filter(|x| x.key_color() == KeyColor::Black)
        .count();
    assert_eq!(blacks, 5);
}

#[test]
fn equal_temperament_frequencies() {
    assert!((n(69).frequency() - 440.0).abs() < 1e-3);
    assert!((n(81).frequency() - 880.0).abs() < 1e-2);
    assert!((n(60).frequency() - 261.626).abs() < 1e-2);
}

#[test]
fn treble_staff_positions() {
    let treble = Clef::Treble;
    assert_eq!(treble.staff_position(n(64)), StaffPosition::BOTTOM_LINE); // E4
    assert_eq!(treble.staff_position(n(71)), StaffPosition::MIDDLE_LINE); // B4
    assert_eq!(treble.staff_position(n(77)), StaffPosition::TOP_LINE); // F5
    assert_eq!(treble.staff_position(n(60)), StaffPosition(-2)); // middle C
    // Sharps share their natural's position.
    assert_eq!(treble.staff_position(n(66)), treble.staff_position(n(65)));
}

#[test]
fn bass_staff_positions() {
    let bass = Clef::Bass;
    assert_eq!(bass.staff_position(n(43)), StaffPosition::BOTTOM_LINE); // G2
    assert_eq!(bass.staff_position(n(53)), StaffPosition(6)); // F3 on the F line
    assert_eq!(bass.staff_position(n(57)), StaffPosition::TOP_LINE); // A3
    assert_eq!(bass.staff_position(n(60)), StaffPosition(10)); // middle C
}

#[test]
fn ledger_lines() {
    assert!(StaffPosition(4).ledger_lines().is_empty());
    assert!(StaffPosition(-1).ledger_lines().is_empty());
    assert!(StaffPosition(9).ledger_lines().is_empty());
    assert_eq!(StaffPosition(-2).ledger_lines(), vec![StaffPosition(-2)]);
    assert_eq!(StaffPosition(-3).ledger_lines(), vec![StaffPosition(-2)]);
    assert_eq!(
        StaffPosition(-4).ledger_lines(),
        vec![StaffPosition(-2), StaffPosition(-4)]
    );
    assert_eq!(
        StaffPosition(13).ledger_lines(),
        vec![StaffPosition(10), StaffPosition(12)]
    );
}

#[test]
fn stems_flip_at_middle_line() {
    assert_eq!(StaffPosition(3).stem_direction(), StemDirection::Up);
    assert_eq!(StaffPosition(4).stem_direction(), StemDirection::Down);
}

#[test]
fn clef_choice_splits_at_middle_c() {
    assert_eq!(Clef::for_note(n(60)), Clef::Treble);
    assert_eq!(Clef::for_note(n(59)), Clef::Bass);
}
