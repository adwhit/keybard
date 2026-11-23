use keybard::builtin::BuiltinSong;
use keybard::chords::{name, progression};
use keybard::key::KeySignature;
use keybard::music::MidiNote;

fn n(v: u8) -> MidiNote {
    MidiNote::new(v).unwrap()
}

fn chord(notes: &[u8]) -> Option<String> {
    let notes: Vec<MidiNote> = notes.iter().map(|v| n(*v)).collect();
    name(&notes, KeySignature::new(0))
}

#[test]
fn names_common_chords() {
    assert_eq!(chord(&[60, 64, 67]).as_deref(), Some("C"));
    assert_eq!(chord(&[57, 60, 64]).as_deref(), Some("Am"));
    assert_eq!(chord(&[55, 59, 62, 65]).as_deref(), Some("G7"));
    assert_eq!(chord(&[59, 62, 65]).as_deref(), Some("Bdim"));
    assert_eq!(chord(&[60, 64, 68]).as_deref(), Some("Caug"));
    assert_eq!(chord(&[60, 65, 67]).as_deref(), Some("Csus4"));
    assert_eq!(chord(&[48, 55]).as_deref(), Some("C5"));
    assert_eq!(chord(&[60, 64, 67, 71]).as_deref(), Some("Cmaj7"));
    assert_eq!(chord(&[62, 65, 69, 72]).as_deref(), Some("Dm7"));
}

#[test]
fn inversions_and_doublings_find_the_root() {
    assert_eq!(
        chord(&[64, 67, 72]).as_deref(),
        Some("C"),
        "first inversion"
    );
    assert_eq!(
        chord(&[55, 60, 64, 72]).as_deref(),
        Some("C"),
        "second inversion, doubled"
    );
    assert_eq!(chord(&[43, 50, 59, 67]).as_deref(), Some("G"));
}

#[test]
fn single_notes_and_clusters_are_not_named() {
    assert_eq!(chord(&[60]), None);
    assert_eq!(chord(&[60, 72]), None, "octaves");
    assert_eq!(chord(&[60, 61, 62]), None);
}

#[test]
fn roots_are_spelled_for_the_key() {
    let notes = [n(70), n(74), n(77)]; // Bb major
    assert_eq!(name(&notes, KeySignature::new(-1)).as_deref(), Some("Bb"));
    assert_eq!(name(&notes, KeySignature::new(0)).as_deref(), Some("A#"));
}

#[test]
fn ode_to_joy_starts_on_c_then_g() {
    let song = BuiltinSong::OdeToJoy.load().unwrap();
    let symbols: Vec<String> = progression(&song)
        .into_iter()
        .map(|(_, s)| s)
        .take(2)
        .collect();
    assert_eq!(symbols, vec!["C", "G"]);
}

#[test]
fn progressions_only_mark_changes() {
    let song = BuiltinSong::OdeToJoy.load().unwrap();
    let marks = progression(&song);
    assert!(
        marks.windows(2).all(|w| w[0].1 != w[1].1),
        "no repeats in a row"
    );
    assert!(marks.windows(2).all(|w| w[0].0 < w[1].0));
}

fn symbols(song: BuiltinSong) -> Vec<String> {
    progression(&song.load().unwrap())
        .into_iter()
        .map(|(_, s)| s)
        .collect()
}

#[test]
fn passing_tones_dont_make_sus_chords() {
    let ode = symbols(BuiltinSong::OdeToJoy);
    assert!(ode.iter().all(|s| !s.contains("sus")), "{ode:?}");
    assert!(ode.contains(&"Am".to_owned()), "{ode:?}");
}

#[test]
fn single_note_bass_lines_are_harmonised_per_bar() {
    let twinkle = symbols(BuiltinSong::Twinkle);
    assert_eq!(
        twinkle.first().map(String::as_str),
        Some("C"),
        "{twinkle:?}"
    );
    assert!(twinkle.len() >= 4, "{twinkle:?}");
    let fur_elise = symbols(BuiltinSong::FurElise);
    assert!(
        fur_elise.iter().all(|s| !s.contains("aug")),
        "{fur_elise:?}"
    );
    assert!(fur_elise.contains(&"Am".to_owned()) && fur_elise.contains(&"E".to_owned()));
}
