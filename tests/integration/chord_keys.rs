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

mod book {
    use keybard::chord_keys::{BOOK, ChordKey, Inversion};
    use keybard::music::PitchClass;

    use super::notes;

    #[test]
    fn the_book_has_every_major_and_minor_triad_once_easiest_first() {
        assert_eq!(BOOK.len(), 24);
        let mut labels: Vec<String> = BOOK.iter().map(|c| c.label()).collect();
        assert_eq!(&labels[..4], ["C", "Am", "G", "Em"]);
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), 24);
    }

    #[test]
    fn inversions_are_told_by_the_lowest_note() {
        let c = ChordKey::major(PitchClass::C);
        assert_eq!(c.inversion(&notes(&[60, 64, 67])), Some(Inversion::Root));
        assert_eq!(c.inversion(&notes(&[64, 67, 72])), Some(Inversion::First));
        assert_eq!(c.inversion(&notes(&[55, 60, 64])), Some(Inversion::Second));
        assert_eq!(c.inversion(&notes(&[60, 63, 67])), None, "not C major");
    }

    #[test]
    fn notes_are_spelled_for_the_key() {
        let spell = |c: ChordKey| c.note_names().join(" ");
        assert_eq!(spell(ChordKey::major(PitchClass::C)), "C E G");
        assert_eq!(spell(ChordKey::minor(PitchClass::C)), "C Eb G");
        assert_eq!(spell(ChordKey::major(PitchClass::ASharp)), "Bb D F");
        assert_eq!(spell(ChordKey::major(PitchClass::E)), "E G# B");
        assert_eq!(spell(ChordKey::minor(PitchClass::FSharp)), "F# A C#");
    }

    #[test]
    fn inversions_are_named_with_their_notes() {
        let c = ChordKey::major(PitchClass::C);
        assert_eq!(c.voicing(Inversion::Root).join(" "), "C E G");
        assert_eq!(c.voicing(Inversion::First).join(" "), "E G C");
        assert_eq!(c.voicing(Inversion::Second).join(" "), "G C E");
        assert_eq!(Inversion::First.label(), "first inversion");
    }
}

#[test]
fn three_neighbouring_keys_make_the_pause_cluster() {
    use keybard::chord_keys::pause_cluster;
    assert_eq!(
        pause_cluster(&notes(&[61, 60, 62])),
        Some([60, 61, 62].map(|v| MidiNote::new(v).unwrap()))
    );
    assert!(
        pause_cluster(&notes(&[48, 72, 73, 74])).is_some(),
        "the other hand can hold a note"
    );
    assert_eq!(pause_cluster(&notes(&[60, 61])), None, "two isn't enough");
    assert_eq!(pause_cluster(&notes(&[60, 61, 63])), None, "a gap");
    assert_eq!(pause_cluster(&notes(&[60, 64, 67])), None, "a chord");
    assert_eq!(pause_cluster(&notes(&[126, 127])), None, "top of the range");
}
