//! The start screen and driving the app with the mouse.

use super::*;
use keybard::app::menu::{Screen, StartScreen};
use keybard::app::songs::{Playlist, SongSource};
use keybard::builtin::BuiltinSong;

const SONGS: [BuiltinSong; 3] = [
    BuiltinSong::Twinkle,
    BuiltinSong::Scale,
    BuiltinSong::FurElise,
];

fn on_start_screen() -> Harness {
    Harness::build(Options {
        song: SONGS[0].load().unwrap(),
        mode: PlayMode::Listen,
        midi_input: MidiInput::Disabled,
        backend: Backend::Headless,
        logging: Logging::Disabled,
        player: Player::Human,
        records: RecordStore::Memory,
        hands: Hands::Both,
        loop_from: None,
        playlist: Playlist::new(SONGS.iter().map(|b| SongSource::Builtin(*b)).collect(), 0),
        practice_log: RecordStore::Memory,
        view: Default::default(),
        screen: Screen::Menu,
    })
}

fn screen(h: &mut Harness) -> Screen {
    *h.world().resource::<Screen>()
}

fn start_screen_shown(h: &mut Harness) -> bool {
    let mut q = h
        .world()
        .query_filtered::<&InheritedVisibility, With<StartScreen>>();
    q.single(h.app.world()).unwrap().get()
}

#[test]
fn opens_on_the_start_screen_with_the_song_waiting() {
    let mut h = on_start_screen();
    assert_eq!(screen(&mut h), Screen::Menu);
    assert!(start_screen_shown(&mut h));
    let before = h.time();
    h.frames(30);
    assert_eq!(h.time(), before, "nothing plays behind the start screen");
    assert_eq!(h.transport().state, PlayState::Paused);
    // Every song, mode and hand choice is a button.
    (0..SONGS.len()).for_each(|i| assert!(h.has_button(Control::SelectSong(i))));
    PlayMode::ALL
        .into_iter()
        .for_each(|m| assert!(h.has_button(Control::SetMode(m))));
    Hands::ALL
        .into_iter()
        .for_each(|hands| assert!(h.has_button(Control::SetHands(hands))));
    assert!(h.ui_texts().iter().any(|t| t.contains("Fur Elise")));
}

#[test]
fn clicking_picks_song_mode_and_hands_then_starts() {
    let mut h = on_start_screen();
    h.click(Control::SelectSong(2));
    assert!(
        h.world()
            .resource::<ActiveSong>()
            .0
            .title
            .starts_with("Fur Elise")
    );
    h.click(Control::SetMode(PlayMode::Wait));
    assert_eq!(h.transport().mode, PlayMode::Wait);
    h.click(Control::SetHands(Hands::Right));
    assert_eq!(h.world().resource::<Judge>().0.hands(), Hands::Right);
    h.frames(10);
    assert_eq!(h.transport().state, PlayState::Paused, "still choosing");

    h.click(Control::StartSong);
    assert_eq!(screen(&mut h), Screen::Song);
    assert!(!start_screen_shown(&mut h));
    assert_eq!(h.transport().state, PlayState::Playing);
    let started = h.time();
    h.frames(10);
    assert!(h.time() > started);
}

#[test]
fn enter_starts_and_escape_comes_back_then_quits() {
    let mut h = on_start_screen();
    h.tap(KeyCode::Enter);
    assert_eq!(screen(&mut h), Screen::Song);
    h.frames(20);
    h.tap(KeyCode::Escape);
    assert_eq!(screen(&mut h), Screen::Menu);
    assert_eq!(h.transport().state, PlayState::Paused);
    assert_eq!(h.time(), h.transport().start, "rewound for next time");
    assert!(h.app.should_exit().is_none());
    h.tap(KeyCode::Escape);
    assert_eq!(h.app.should_exit(), Some(AppExit::Success));
}

#[test]
fn space_on_the_start_screen_does_not_sneak_the_song_on() {
    let mut h = on_start_screen();
    h.tap(KeyCode::Space);
    h.frames(5);
    assert_eq!(h.transport().state, PlayState::Paused);
    assert_eq!(h.time(), h.transport().start);
}

#[test]
fn option_buttons_show_which_settings_are_on() {
    use keybard::app::falling::View;
    let mut h = on_start_screen();
    h.click(Control::ToggleSightReading);
    assert_eq!(*h.world().resource::<View>(), View::SightReading);
    let colour = |h: &mut Harness, control: Control| {
        let e = h.button(control);
        h.world().get::<BackgroundColor>(e).unwrap().0
    };
    assert_ne!(
        colour(&mut h, Control::ToggleSightReading),
        colour(&mut h, Control::ToggleMetronome),
        "on and off look different"
    );
    assert_eq!(
        colour(&mut h, Control::SetMode(PlayMode::Listen)),
        colour(&mut h, Control::ToggleSightReading),
        "both selected"
    );
}

#[test]
fn the_results_card_has_buttons_for_what_next() {
    let mut h = on_start_screen();
    h.click(Control::SetMode(PlayMode::Play));
    h.click(Control::StartSong);
    // Play nothing: every note is missed, so there are weak bars to drill.
    while h.transport().state != PlayState::Finished {
        h.app.update();
    }
    h.frames(2);
    assert!(h.has_button(Control::TogglePlay));
    assert!(h.has_button(Control::ToggleLoop));
    assert!(h.has_button(Control::NextSong));
    assert!(h.has_button(Control::OpenMenu));

    h.click(Control::TogglePlay);
    assert_eq!(h.transport().state, PlayState::Playing, "playing again");
    assert!(h.time().0 - h.transport().start.0 < 0.05, "from the top");
    while h.transport().state != PlayState::Finished {
        h.app.update();
    }
    h.frames(2);
    h.click(Control::OpenMenu);
    assert_eq!(screen(&mut h), Screen::Menu);
    assert!(!h.has_button(Control::OpenMenu), "card gone");
}

#[test]
fn every_command_has_a_stable_id_for_the_menu_bar() {
    let all = [
        Control::TogglePlay,
        Control::Restart,
        Control::SeekBack,
        Control::SeekForward,
        Control::Slower,
        Control::Faster,
        Control::CycleMode,
        Control::CycleHands,
        Control::ToggleLoop,
        Control::ToggleMetronome,
        Control::ToggleAchievements,
        Control::ToggleSightReading,
        Control::CycleLabels,
        Control::PreviousSong,
        Control::NextSong,
        Control::StartSong,
        Control::OpenMenu,
        Control::Back,
        Control::Quit,
        Control::SelectSong(3),
        Control::SetMode(PlayMode::Wait),
        Control::SetHands(Hands::Left),
    ];
    all.into_iter()
        .for_each(|c| assert_eq!(Control::from_id(&c.id()), Some(c), "{}", c.id()));
    assert_eq!(Control::from_id("nonsense"), None);
}

/// Hold `pitches` down on the MIDI keyboard, then let go.
fn play_chord(h: &mut Harness, pitches: &[u8]) {
    pitches.iter().for_each(|p| h.midi(*p, KeyEdge::Down));
    h.frames(2);
    pitches.iter().for_each(|p| h.midi(*p, KeyEdge::Up));
    h.frames(2);
}

fn visible_chords(h: &mut Harness) -> Vec<String> {
    let mut q = h.world().query::<(&ControlButton, &InheritedVisibility)>();
    q.iter(h.app.world())
        .filter(|(_, v)| v.get())
        .filter_map(|(b, _)| b.0.chord().map(|c| c.label()))
        .collect()
}

#[test]
fn every_visible_button_has_its_own_chord_shown_on_it() {
    let mut h = on_start_screen();
    // Song rows (and so their badges) arrive once the titles are loaded.
    h.frames(2);
    let chords = visible_chords(&mut h);
    assert!(chords.len() > 10, "{chords:?}");
    let mut unique = chords.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), chords.len(), "no two buttons share a chord");
    // Each chord is drawn on its button.
    let texts = h.ui_texts();
    chords
        .iter()
        .for_each(|c| assert!(texts.contains(c), "{c} badge"));
    assert_eq!(Control::StartSong.chord().unwrap().label(), "C");
}

#[test]
fn playing_a_chord_presses_its_button() {
    let mut h = on_start_screen();
    let fur_elise = Control::SelectSong(2).chord().unwrap();
    let pitches: Vec<u8> = fur_elise
        .pitch_classes()
        .iter()
        .map(|pc| 60 + pc.semitone())
        .collect();
    play_chord(&mut h, &pitches);
    assert!(
        h.world()
            .resource::<ActiveSong>()
            .0
            .title
            .starts_with("Fur Elise")
    );
    // C major (in second inversion) starts it.
    play_chord(&mut h, &[55, 60, 64]);
    assert_eq!(screen(&mut h), Screen::Song);
    assert_eq!(h.transport().state, PlayState::Playing);
}

#[test]
fn chords_do_nothing_while_the_music_plays() {
    let mut h = on_start_screen();
    h.click(Control::SetMode(PlayMode::Play));
    h.click(Control::StartSong);
    h.frames(10);
    // A C chord mid-song is just notes, not "Play again" or "Start".
    play_chord(&mut h, &[60, 64, 67]);
    assert_eq!(h.transport().state, PlayState::Playing);
    assert_eq!(screen(&mut h), Screen::Song);
}

#[test]
fn a_held_chord_presses_once() {
    use keybard::app::falling::View;
    let mut h = on_start_screen();
    let toggle = Control::ToggleSightReading.chord().unwrap();
    let pitches: Vec<u8> = toggle
        .pitch_classes()
        .iter()
        .map(|pc| 60 + pc.semitone())
        .collect();
    pitches.iter().for_each(|p| h.midi(*p, KeyEdge::Down));
    h.frames(2);
    assert_eq!(*h.world().resource::<View>(), View::SightReading);
    // Re-strike one note while still holding the rest: no second toggle.
    h.midi(pitches[1], KeyEdge::Up);
    h.midi(pitches[1], KeyEdge::Down);
    h.frames(2);
    assert_eq!(*h.world().resource::<View>(), View::SightReading);
    // Let go completely, and the chord works again.
    pitches.iter().for_each(|p| h.midi(*p, KeyEdge::Up));
    play_chord(&mut h, &pitches);
    assert_eq!(*h.world().resource::<View>(), View::Bars);
}
