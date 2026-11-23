use keybard::builtin::BuiltinSong;
use keybard::key::KeySignature;
use keybard::music::{Accidental, Clef, Letter, MidiNote, StaffPosition};
use keybard::song::{Hand, Song, Tempo, parse_voice};

fn n(v: u8) -> MidiNote {
    MidiNote::new(v).unwrap()
}

fn key_of(voice: &str) -> KeySignature {
    let tempo = Tempo::from_bpm(60.0, 4);
    Song::new("k", tempo, parse_voice(voice, Hand::Right, tempo).unwrap())
        .unwrap()
        .key()
}

#[test]
fn infers_keys_from_the_notes() {
    assert_eq!(key_of("C4 D4 E4 F4 G4 A4 B4 C5"), KeySignature::new(0));
    assert_eq!(key_of("G4 A4 B4 C5 D5 E5 F#5 G5"), KeySignature::new(1));
    assert_eq!(key_of("F4 G4 A4 A#4 C5 D5 E5 F5"), KeySignature::new(-1));
    assert_eq!(key_of("D4 E4 F#4 G4 A4 B4 C#5 D5"), KeySignature::new(2));
    assert_eq!(key_of("D#4 F4 G4 G#4 A#4 C5 D5 D#5"), KeySignature::new(-3));
}

#[test]
fn builtin_songs_get_sensible_keys() {
    let key = |b: BuiltinSong| b.load().unwrap().key();
    assert_eq!(key(BuiltinSong::OdeToJoy), KeySignature::new(0));
    assert_eq!(key(BuiltinSong::Twinkle), KeySignature::new(0));
    assert_eq!(key(BuiltinSong::Minuet), KeySignature::new(1), "G major");
    assert_eq!(key(BuiltinSong::FurElise), KeySignature::new(0), "A minor");
}

#[test]
fn black_keys_are_spelled_to_suit_the_key() {
    let f_major = KeySignature::new(-1);
    let bb = f_major.spell(n(70));
    assert_eq!((bb.letter, bb.accidental), (Letter::B, Accidental::Flat));
    let c_major = KeySignature::new(0);
    let a_sharp = c_major.spell(n(70));
    assert_eq!(
        (a_sharp.letter, a_sharp.accidental),
        (Letter::A, Accidental::Sharp)
    );
    // Bb sits on the middle line; A# in the space below it.
    assert_eq!(Clef::Treble.position_of(bb), StaffPosition::MIDDLE_LINE);
    assert_eq!(Clef::Treble.position_of(a_sharp), StaffPosition(3));
}

#[test]
fn only_notes_outside_the_key_get_accidentals() {
    let g_major = KeySignature::new(1);
    assert_eq!(
        g_major.accidental_to_show(g_major.spell(n(66))),
        None,
        "F# is in G"
    );
    assert_eq!(
        g_major.accidental_to_show(g_major.spell(n(65))),
        Some(Accidental::Natural),
        "F natural needs a natural sign"
    );
    assert_eq!(
        g_major.accidental_to_show(g_major.spell(n(68))),
        Some(Accidental::Sharp),
        "G# is foreign"
    );
    let c = KeySignature::new(0);
    assert_eq!(c.accidental_to_show(c.spell(n(60))), None);
}

#[test]
fn signature_letters_follow_the_circle_of_fifths() {
    assert_eq!(
        KeySignature::new(2).altered_letters(),
        vec![Letter::F, Letter::C]
    );
    assert_eq!(
        KeySignature::new(-3).altered_letters(),
        vec![Letter::B, Letter::E, Letter::A]
    );
    assert!(KeySignature::new(0).altered_letters().is_empty());
}
