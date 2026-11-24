use keybard::chord_keys::ChordKey;
use keybard::music::{MidiNote, PitchClass};

fn notes(values: &[u8]) -> Vec<MidiNote> {
    values.iter().map(|v| MidiNote::new(*v).unwrap()).collect()
}

#[test]
fn a_chord_matches_in_any_octave_or_inversion() {
    let c = ChordKey::major(PitchClass::C);
    assert!(c.matches(&notes(&[60, 64, 67])));
    assert!(
        c.matches(&notes(&[52, 55, 60])),
        "first inversion, lower down"
    );
    assert!(c.matches(&notes(&[48, 60, 64, 67, 72])), "doubled roots");
}

#[test]
fn a_chord_needs_exactly_its_notes() {
    let c = ChordKey::major(PitchClass::C);
    assert!(!c.matches(&notes(&[60, 64])), "missing the fifth");
    assert!(!c.matches(&notes(&[60, 64, 67, 70])), "C7 is not C");
    assert!(!c.matches(&notes(&[60, 63, 67])), "C minor is not C");
    assert!(ChordKey::minor(PitchClass::C).matches(&notes(&[60, 63, 67])));
}

#[test]
fn chords_are_labelled_as_players_name_them() {
    let label = |k: ChordKey| k.label();
    assert_eq!(label(ChordKey::major(PitchClass::C)), "C");
    assert_eq!(label(ChordKey::minor(PitchClass::A)), "Am");
    assert_eq!(label(ChordKey::major(PitchClass::ASharp)), "Bb");
    assert_eq!(label(ChordKey::minor(PitchClass::FSharp)), "F#m");
}
