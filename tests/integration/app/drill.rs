//! Flash cards from the chord book, steered from the piano or computer keys.

use super::book::{chord, open_book, open_book_with};
use super::*;
use keybard::app::book::ChordBook;
use keybard::app::drill::FlashCards;
use keybard::app::menu::Screen;
use keybard::app::piano::GuideKeys;
use keybard::chord_keys::Inversion;
use keybard::drill::{Card, ROUND, Stage, TIME_LIMIT};

const DRILL: [u8; 3] = [62, 67, 69]; // Dsus4
const BACK: [u8; 3] = [59, 62, 65]; // Bdim

/// The book, with C major (root position) and A minor (first inversion)
/// cleared.
fn cleared_book() -> Harness {
    open_book_with(Settings {
        chord_book: [(0, Inversion::Root), (1, Inversion::First)]
            .into_iter()
            .collect(),
        ..Settings::default()
    })
}

fn screen(h: &mut Harness) -> Screen {
    *h.world().resource::<Screen>()
}

fn stage(h: &mut Harness) -> Stage {
    h.world()
        .resource::<FlashCards>()
        .drill
        .round()
        .expect("a round")
        .stage()
}

fn card(h: &mut Harness) -> Card {
    match stage(h) {
        Stage::Asking { card, .. } => card,
        other => panic!("not asking: {other:?}"),
    }
}

fn keys(card: Card) -> Vec<u8> {
    card.keys().iter().map(|n| n.value()).collect()
}

fn shows(h: &mut Harness, text: &str) -> bool {
    h.ui_texts().iter().any(|t| t.contains(text))
}

/// Play the card in front of us, then wait for the next one.
fn answer(h: &mut Harness) {
    let card = card(h);
    chord(h, &keys(card));
    h.frames(60);
}

#[test]
fn with_nothing_cleared_the_book_says_how_to_make_cards() {
    let mut h = open_book();
    assert!(shows(&mut h, "Clear a challenge"), "{:?}", h.ui_texts());
    chord(&mut h, &DRILL);
    assert_eq!(screen(&mut h), Screen::ChordBook);
    assert!(shows(&mut h, "Nothing cleared yet"), "{:?}", h.ui_texts());
}

#[test]
fn a_chord_starts_flash_cards_of_the_cleared_chords() {
    let mut h = cleared_book();
    assert!(shows(&mut h, "2 cards"), "{:?}", h.ui_texts());
    chord(&mut h, &DRILL);
    assert_eq!(screen(&mut h), Screen::Drill);
    let card = card(&mut h);
    assert!(
        [(0, Inversion::Root), (1, Inversion::First)].contains(&(card.page(), card.inversion()))
    );
    let name = card.chord().name();
    assert!(shows(&mut h, &name), "{name} in {:?}", h.ui_texts());
    assert!(shows(&mut h, card.inversion().label()));
    assert!(shows(&mut h, "card 1 of 10"), "{:?}", h.ui_texts());
    assert!(
        h.world().resource::<GuideKeys>().0.is_empty(),
        "no keys lit to give it away"
    );
    assert!(h.has_button(Control::Back), "a Stop button");
    assert!(!h.has_button(Control::StartDrill));
}

#[test]
fn a_right_answer_scores_and_the_next_card_follows() {
    let mut h = cleared_book();
    h.tap(KeyCode::Enter);
    assert_eq!(screen(&mut h), Screen::Drill, "Enter starts the drill too");
    let first = card(&mut h);
    chord(&mut h, &keys(first));
    assert!(shows(&mut h, "Right!"), "{:?}", h.ui_texts());
    assert!(shows(&mut h, "1 right"), "{:?}", h.ui_texts());
    h.frames(60);
    let round = h.world().resource::<FlashCards>().drill.round().unwrap();
    assert_eq!(round.number(), 2);
    assert_ne!(card(&mut h), first, "a different card");
}

#[test]
fn answers_never_press_buttons() {
    let mut h = cleared_book();
    chord(&mut h, &DRILL);
    // C (Start), G (Chord book), Am (Start screen) and F (Loop): wrong
    // answers here, never button presses.
    [[60, 64, 67], [55, 59, 62], [57, 60, 64], [53, 57, 60]]
        .iter()
        .for_each(|wrong| {
            chord(&mut h, wrong);
            assert_eq!(screen(&mut h), Screen::Drill);
            h.frames(120);
        });
    assert!(h.app.should_exit().is_none());
    assert_ne!(h.transport().state, PlayState::Playing, "no song started");
}

#[test]
fn time_runs_out_and_the_answer_is_lit() {
    let mut h = cleared_book();
    chord(&mut h, &DRILL);
    let card = card(&mut h);
    assert!(
        shows(&mut h, "5.0s") || shows(&mut h, "4.9s"),
        "{:?}",
        h.ui_texts()
    );
    h.frames((TIME_LIMIT.0 * 60.0) as usize + 2);
    assert!(shows(&mut h, "Time's up"), "{:?}", h.ui_texts());
    assert_eq!(h.world().resource::<GuideKeys>().0, card.keys());
    assert!(shows(&mut h, "1 too slow"), "{:?}", h.ui_texts());
}

#[test]
fn a_round_ends_with_a_summary_and_goes_again_or_back_by_chord() {
    let mut h = cleared_book();
    chord(&mut h, &DRILL);
    (0..ROUND).for_each(|_| answer(&mut h));
    assert_eq!(stage(&mut h), Stage::Over);
    assert!(shows(&mut h, "10 of 10 right"), "{:?}", h.ui_texts());
    assert!(h.has_button(Control::StartDrill), "Again");
    chord(&mut h, &DRILL);
    assert!(matches!(stage(&mut h), Stage::Asking { .. }), "a new round");
    chord(&mut h, &BACK);
    assert_eq!(screen(&mut h), Screen::ChordBook);
    assert!(h.world().resource::<FlashCards>().drill.round().is_none());
    // The book's own progress is untouched by the drill.
    assert_eq!(h.world().resource::<ChordBook>().progress().len(), 2);
}

#[test]
fn esc_stops_the_drill_and_esc_again_leaves_the_book() {
    let mut h = cleared_book();
    h.tap(KeyCode::Enter);
    h.tap(KeyCode::Escape);
    assert_eq!(screen(&mut h), Screen::ChordBook);
    h.tap(KeyCode::Escape);
    assert_eq!(screen(&mut h), Screen::Menu);
    assert!(h.app.should_exit().is_none());
}

#[test]
fn cards_can_be_answered_on_the_computer_keys() {
    let mut h = open_book_with(Settings {
        chord_book: [(0, Inversion::Root)].into_iter().collect(),
        ..Settings::default()
    });
    h.tap(KeyCode::Enter);
    // Z C B is C3 E3 G3: C major in root position.
    [KeyCode::KeyZ, KeyCode::KeyC, KeyCode::KeyB]
        .into_iter()
        .for_each(|k| h.key(k, ButtonState::Pressed));
    assert!(shows(&mut h, "Right!"), "{:?}", h.ui_texts());
}
