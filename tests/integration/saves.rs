use keybard::chord_keys::Inversion;
use keybard::performance::{Hands, PlayMode};
use keybard::saves::{BarLabels, PlayerName, SaveFile, Saves, Settings};

#[test]
fn settings_round_trip_through_their_file_format() {
    let settings = Settings {
        mode: PlayMode::Wait,
        hands: Hands::Left,
        sight_reading: true,
        metronome: true,
        labels: BarLabels::Names,
        midi_muted: true,
        song: Some("Fur Elise, opening (Beethoven)".to_owned()),
        chord_book: [(0, Inversion::Root), (3, Inversion::Second)]
            .into_iter()
            .collect(),
    };
    assert_eq!(Settings::parse(&settings.to_text()), settings);
}

#[test]
fn missing_or_garbled_settings_fall_back_to_defaults() {
    let defaults = Settings::default();
    assert_eq!(defaults.mode, PlayMode::Play);
    assert_eq!(defaults.labels, BarLabels::Both);
    assert_eq!(Settings::parse(""), defaults);
    let partial = Settings::parse("mode=Listen\nhands=sideways\nnonsense\nmetronome=true\n");
    assert_eq!(partial.mode, PlayMode::Listen);
    assert_eq!(partial.hands, Hands::Both, "unknown value ignored");
    assert!(partial.metronome);
}

#[test]
fn player_names_are_checked() {
    assert_eq!(PlayerName::new("  Alex ").unwrap().as_str(), "Alex");
    assert!(PlayerName::new("Mary-Jane O'Neil 2").is_ok());
    ["", "   ", "../etc", "a/b", ".hidden", &"x".repeat(25)]
        .iter()
        .for_each(|bad| assert!(PlayerName::new(bad).is_err(), "{bad:?}"));
}

#[test]
fn the_first_run_makes_player_1_and_remembers_who_played() {
    let dir = tempfile::tempdir().unwrap();
    let saves = Saves::at(dir.path());
    let first = saves.current();
    assert_eq!(first.as_str(), "Player 1");
    assert_eq!(saves.players(), vec![first.clone()]);
    let alex = saves.create(&PlayerName::new("Alex").unwrap()).unwrap();
    assert_eq!(saves.current(), alex, "a new player becomes current");
    saves.choose(&first);
    assert_eq!(Saves::at(dir.path()).current(), first, "remembered on disk");
    assert_eq!(saves.players().len(), 2);
    // Each player keeps their own files.
    let bests = saves.file(&alex, SaveFile::Bests).unwrap();
    assert!(bests.starts_with(dir.path().join("profiles").join("Alex")));
}

#[test]
fn settings_are_saved_per_player() {
    let dir = tempfile::tempdir().unwrap();
    let saves = Saves::at(dir.path());
    let one = saves.current();
    let two = saves.create(&PlayerName::new("Two").unwrap()).unwrap();
    let wait = Settings {
        mode: PlayMode::Wait,
        ..Settings::default()
    };
    saves.save_settings(&one, &wait);
    assert_eq!(saves.load_settings(&one).mode, PlayMode::Wait);
    assert_eq!(saves.load_settings(&two), Settings::default());
}

#[test]
fn memory_saves_keep_nothing_on_disk() {
    let saves = Saves::memory();
    let player = saves.current();
    assert_eq!(saves.file(&player, SaveFile::Settings), None);
    saves.save_settings(&player, &Settings::default());
    assert_eq!(saves.load_settings(&player), Settings::default());
}
