//! Players, and their settings kept between runs.

use super::*;
use keybard::app::book::ChordBook;
use keybard::app::falling::{BarLabels, View};
use keybard::app::input::MidiEcho;
use keybard::app::menu::Screen;
use keybard::app::songs::{Playlist, SongSource};
use keybard::app::transport::Metronome;
use keybard::builtin::BuiltinSong;
use keybard::chord_keys::Inversion;

const SONGS: [BuiltinSong; 3] = [
    BuiltinSong::Twinkle,
    BuiltinSong::Scale,
    BuiltinSong::FurElise,
];

fn start(saves: Saves, settings: Settings) -> Harness {
    Harness::build(Options {
        song: SONGS[0].load().unwrap(),
        midi_input: MidiInput::Disabled,
        backend: Backend::Headless,
        logging: Logging::Disabled,
        player: Player::Human,
        loop_from: None,
        playlist: Playlist::new(SONGS.iter().map(|b| SongSource::Builtin(*b)).collect(), 0),
        screen: Screen::Menu,
        settings,
        saves,
    })
}

#[test]
fn changed_settings_are_saved_for_the_player() {
    let dir = tempfile::tempdir().unwrap();
    let saves = Saves::at(dir.path());
    let mut h = start(saves.clone(), Settings::default());
    h.frames(2);
    h.click(Control::SetMode(PlayMode::Wait));
    h.click(Control::SetHands(Hands::Left));
    h.click(Control::ToggleMetronome);
    h.click(Control::ToggleMidiEcho);
    h.click(Control::SelectSong(2));
    h.tap(KeyCode::F1);
    h.frames(2);
    let saved = saves.load_settings(&saves.current());
    assert_eq!(saved.mode, PlayMode::Wait);
    assert_eq!(saved.hands, Hands::Left);
    assert!(saved.metronome && saved.midi_muted);
    assert_eq!(saved.labels, BarLabels::Fingers);
    assert!(
        saved
            .song
            .as_deref()
            .is_some_and(|s| s.starts_with("Fur Elise"))
    );
}

#[test]
fn saved_settings_take_effect_at_startup() {
    let settings = Settings {
        mode: PlayMode::Listen,
        hands: Hands::Right,
        sight_reading: true,
        metronome: true,
        labels: BarLabels::Off,
        midi_muted: true,
        song: None,
        chord_book: [(0, Inversion::Root)].into_iter().collect(),
    };
    let mut h = start(Saves::memory(), settings);
    assert_eq!(h.transport().mode, PlayMode::Listen);
    assert_eq!(h.world().resource::<Judge>().0.hands(), Hands::Right);
    assert_eq!(*h.world().resource::<View>(), View::SightReading);
    assert_eq!(*h.world().resource::<Metronome>(), Metronome::On);
    assert_eq!(*h.world().resource::<BarLabels>(), BarLabels::Off);
    assert_eq!(*h.world().resource::<MidiEcho>(), MidiEcho::Mute);
    assert_eq!(
        h.world().resource::<ChordBook>().progress(),
        [(0, Inversion::Root)].into_iter().collect()
    );
}

#[test]
fn nothing_is_written_before_anything_changes() {
    let dir = tempfile::tempdir().unwrap();
    let saves = Saves::at(dir.path());
    let mut h = start(saves.clone(), Settings::default());
    h.frames(10);
    assert!(
        !saves
            .file(&saves.current(), SaveFile::Settings)
            .unwrap()
            .exists(),
        "just starting up saves nothing"
    );
}

fn screen(h: &mut Harness) -> Screen {
    *h.world().resource::<Screen>()
}

fn playing_as(h: &mut Harness) -> String {
    h.world()
        .resource::<keybard::app::players::Profile>()
        .player
        .as_str()
        .to_owned()
}

#[test]
fn the_start_screen_says_who_is_playing() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = start(Saves::at(dir.path()), Settings::default());
    h.frames(2);
    assert!(
        h.ui_texts().iter().any(|t| t == "Playing as Player 1"),
        "{:?}",
        h.ui_texts()
    );
    assert!(h.has_button(Control::OpenPlayers));
}

#[test]
fn a_new_player_is_typed_in_and_starts_fresh() {
    let dir = tempfile::tempdir().unwrap();
    let saves = Saves::at(dir.path());
    let mut h = start(saves.clone(), Settings::default());
    h.click(Control::SetMode(PlayMode::Wait));
    h.frames(2);

    h.click(Control::OpenPlayers);
    assert_eq!(screen(&mut h), Screen::Players);
    assert!(h.has_button(Control::SelectPlayer(0)), "Player 1 listed");
    h.click(Control::NewPlayer);
    // Typing types: no notes, no shortcuts (Space doesn't play, Z isn't C3).
    h.type_text("Alex Z");
    assert!(h.world().resource::<HeldKeys>().0.is_empty());
    h.tap(KeyCode::Backspace);
    h.tap(KeyCode::Backspace);
    assert!(
        h.ui_texts().iter().any(|t| t.contains("Alex")),
        "{:?}",
        h.ui_texts()
    );
    h.tap(KeyCode::Enter);

    assert_eq!(playing_as(&mut h), "Alex");
    assert_eq!(screen(&mut h), Screen::Menu, "back to the start screen");
    assert_eq!(saves.current().as_str(), "Alex", "remembered");
    assert_eq!(
        h.transport().mode,
        PlayMode::Play,
        "Alex starts with defaults"
    );
    assert!(h.ui_texts().iter().any(|t| t == "Playing as Alex"));
}

#[test]
fn switching_back_restores_that_players_settings_and_bests() {
    let dir = tempfile::tempdir().unwrap();
    let saves = Saves::at(dir.path());
    let one = saves.current();
    saves.save_settings(
        &one,
        &Settings {
            mode: PlayMode::Wait,
            metronome: true,
            ..Settings::default()
        },
    );
    std::fs::write(
        saves.file(&one, SaveFile::Bests).unwrap(),
        format!(
            "{}\t{}\t900\t0.9\t5\tTwinkle, Twinkle, Little Star\n",
            PlayMode::Wait.label(),
            Hands::Both.label()
        ),
    )
    .unwrap();
    saves
        .create(&keybard::saves::PlayerName::new("Two").unwrap())
        .unwrap();
    // Start as Two (just created, so current), with defaults.
    let mut h = start(saves.clone(), Settings::default());
    assert_eq!(playing_as(&mut h), "Two");
    h.click(Control::OpenPlayers);
    let index = saves.players().iter().position(|p| *p == one).unwrap();
    h.click(Control::SelectPlayer(index));
    h.frames(2);
    assert_eq!(playing_as(&mut h), "Player 1");
    assert_eq!(h.transport().mode, PlayMode::Wait);
    assert_eq!(
        *h.world().resource::<keybard::app::transport::Metronome>(),
        keybard::app::transport::Metronome::On
    );
    let best = h
        .world()
        .resource::<keybard::app::results::Bests>()
        .records
        .best(&RecordKey {
            title: "Twinkle, Twinkle, Little Star".to_owned(),
            mode: PlayMode::Wait,
            hands: Hands::Both,
        });
    assert_eq!(best.map(|b| b.points), Some(900), "their bests are loaded");
}

#[test]
fn escape_leaves_the_players_screen_and_cancels_typing() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = start(Saves::at(dir.path()), Settings::default());
    h.click(Control::OpenPlayers);
    h.click(Control::NewPlayer);
    h.type_text("Zed");
    h.tap(KeyCode::Escape);
    assert_eq!(
        screen(&mut h),
        Screen::Players,
        "Esc cancels the name first"
    );
    assert_eq!(playing_as(&mut h), "Player 1");
    h.tap(KeyCode::Escape);
    assert_eq!(screen(&mut h), Screen::Menu);
}
