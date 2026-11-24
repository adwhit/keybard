//! The chord book: flick through triads and play each one three ways.

use super::*;
use keybard::app::book::ChordBook;
use keybard::app::menu::{Screen, StartScreen};
use keybard::app::piano::GuideKeys;

fn open_book() -> Harness {
    let mut h = Harness::build(Options {
        song: song(),
        settings: Settings {
            mode: PlayMode::Play,
            ..Settings::default()
        },
        midi_input: MidiInput::Disabled,
        backend: Backend::Headless,
        logging: Logging::Disabled,
        player: Player::Human,
        saves: Saves::memory(),
        loop_from: None,
        playlist: Default::default(),
        screen: Screen::Menu,
    });
    h.click(Control::OpenChordBook);
    h
}

fn chord(h: &mut Harness, pitches: &[u8]) {
    pitches.iter().for_each(|p| h.midi(*p, KeyEdge::Down));
    h.frames(2);
    pitches.iter().for_each(|p| h.midi(*p, KeyEdge::Up));
    h.frames(2);
}

fn page(h: &mut Harness) -> usize {
    h.world().resource::<ChordBook>().page
}

#[test]
fn the_book_opens_on_c_major_with_its_keys_lit() {
    let mut h = open_book();
    assert_eq!(*h.world().resource::<Screen>(), Screen::ChordBook);
    let mut q = h
        .world()
        .query_filtered::<&InheritedVisibility, With<StartScreen>>();
    assert!(
        !q.single(h.app.world()).unwrap().get(),
        "start screen hidden"
    );
    assert_eq!(page(&mut h), 0);
    let texts = h.ui_texts();
    assert!(texts.iter().any(|t| t == "C major"), "{texts:?}");
    assert!(texts.iter().any(|t| t.contains("C E G")), "{texts:?}");
    assert_eq!(
        h.world().resource::<GuideKeys>().0,
        vec![n(60), n(64), n(67)],
        "C4 E4 G4 shown on the piano"
    );
}

#[test]
fn playing_each_inversion_ticks_off_the_challenges() {
    let mut h = open_book();
    assert_eq!(h.world().resource::<ChordBook>().mastered(), 0);
    chord(&mut h, &[60, 64, 67]);
    assert!(h.ui_texts().iter().any(|t| t.contains("[x] root position")));
    chord(&mut h, &[64, 67, 72]);
    chord(&mut h, &[55, 60, 64]);
    assert_eq!(h.world().resource::<ChordBook>().mastered(), 1);
    assert!(
        h.ui_texts().iter().any(|t| t.contains("C major mastered")),
        "{:?}",
        h.ui_texts()
    );
    // Playing C in the book is practice, not "Start".
    assert_eq!(*h.world().resource::<Screen>(), Screen::ChordBook);
}

#[test]
fn pages_turn_with_buttons_and_arrows_and_esc_closes_the_book() {
    let mut h = open_book();
    h.click(Control::SeekForward);
    assert_eq!(page(&mut h), 1);
    assert!(h.ui_texts().iter().any(|t| t == "A minor"));
    assert_eq!(
        h.world().resource::<GuideKeys>().0,
        vec![n(57), n(60), n(64)]
    );
    // Playing the page's chord is practice: no button in the book has one.
    chord(&mut h, &[57, 60, 64]);
    assert_eq!(*h.world().resource::<Screen>(), Screen::ChordBook);
    assert!(h.ui_texts().iter().any(|t| t.contains("[x] root position")));
    h.tap(KeyCode::ArrowRight);
    assert_eq!(page(&mut h), 2);
    h.tap(KeyCode::ArrowLeft);
    h.tap(KeyCode::ArrowLeft);
    h.tap(KeyCode::ArrowLeft);
    assert_eq!(page(&mut h), 23, "wraps round to the last page");
    h.tap(KeyCode::Escape);
    assert_eq!(*h.world().resource::<Screen>(), Screen::Menu);
    assert!(h.world().resource::<GuideKeys>().0.is_empty(), "keys unlit");
    assert!(h.app.should_exit().is_none());
}
