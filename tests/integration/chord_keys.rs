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

mod controls {
    use keybard::chord_keys::{BOOK, ChordKey};
    use keybard::music::PitchClass;

    use super::notes;

    #[test]
    fn sus4_and_diminished_chords_match_their_own_notes() {
        let csus4 = ChordKey::sus4(PitchClass::C);
        assert!(csus4.matches(&notes(&[60, 65, 67])), "C F G");
        assert!(csus4.matches(&notes(&[53, 55, 60])), "any inversion");
        assert!(
            !csus4.matches(&notes(&[60, 64, 67])),
            "C major is not Csus4"
        );
        let bdim = ChordKey::diminished(PitchClass::B);
        assert!(bdim.matches(&notes(&[59, 62, 65])), "B D F");
        assert_eq!(csus4.label(), "Csus4");
        assert_eq!(bdim.label(), "Bdim");
    }

    #[test]
    fn no_book_page_is_a_sus4_or_diminished_chord() {
        let others = [
            ChordKey::sus4(PitchClass::C),
            ChordKey::sus4(PitchClass::G),
            ChordKey::sus4(PitchClass::D),
            ChordKey::diminished(PitchClass::B),
        ];
        others.iter().for_each(|other| {
            let held = other
                .pitch_classes()
                .map(|pc| keybard::music::MidiNote::new(60 + pc.semitone()).unwrap());
            assert!(
                BOOK.iter().all(|page| !page.matches(&held)),
                "{} is in the book",
                other.label()
            );
        });
    }
}

mod names {
    use keybard::chord_keys::{BOOK, ChordKey, Inversion};
    use keybard::music::PitchClass;

    use super::notes;

    #[test]
    fn chords_have_full_names() {
        assert_eq!(ChordKey::major(PitchClass::C).name(), "C major");
        assert_eq!(ChordKey::minor(PitchClass::FSharp).name(), "F# minor");
        assert_eq!(ChordKey::diminished(PitchClass::B).name(), "B diminished");
    }

    #[test]
    fn keys_are_the_voicing_around_middle_c() {
        let c = ChordKey::major(PitchClass::C);
        assert_eq!(c.keys(Inversion::Root), notes(&[60, 64, 67]));
        assert_eq!(c.keys(Inversion::First), notes(&[64, 67, 72]));
        assert_eq!(c.keys(Inversion::Second), notes(&[55, 60, 64]));
        let am = ChordKey::minor(PitchClass::A);
        assert_eq!(am.keys(Inversion::Root), notes(&[57, 60, 64]));
        BOOK.iter().for_each(|chord| {
            Inversion::ALL.iter().for_each(|inversion| {
                let keys = chord.keys(*inversion);
                assert_eq!(
                    chord.inversion(&keys),
                    Some(*inversion),
                    "{}",
                    chord.label()
                );
                assert!(keys.windows(2).all(|w| w[0] < w[1]));
            });
        });
    }

    #[test]
    fn the_book_page_of_a_chord_is_found() {
        assert_eq!(ChordKey::in_book(&notes(&[57, 60, 64])), Some(BOOK[1]));
        assert_eq!(ChordKey::in_book(&notes(&[59, 62, 65])), None, "Bdim");
        assert_eq!(ChordKey::in_book(&notes(&[60, 64])), None);
    }
}
