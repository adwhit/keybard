//! The flash-card drill: chords you've cleared in the chord book, one card
//! at a time, each against the clock.

use std::collections::BTreeSet;

use keybard::chord_keys::{BOOK, Inversion};
use keybard::drill::{
    Answer, Card, Drill, Mistake, NothingCleared, RETRY_AFTER, ROUND, Stage, TIME_LIMIT,
};
use keybard::music::MidiNote;
use keybard::song::Seconds;

fn notes(values: &[u8]) -> Vec<MidiNote> {
    values.iter().map(|v| MidiNote::new(*v).unwrap()).collect()
}

/// C major in root position and first inversion, A minor in root position.
fn cleared() -> Vec<(usize, Inversion)> {
    vec![
        (0, Inversion::Root),
        (0, Inversion::First),
        (1, Inversion::Root),
    ]
}

fn started(cleared: Vec<(usize, Inversion)>, seed: u64) -> Drill {
    let mut drill = Drill::new(seed);
    drill.start(cleared).expect("something to drill");
    drill
}

fn card(drill: &Drill) -> Card {
    match drill.round().expect("a round").stage() {
        Stage::Asking { card, .. } => card,
        other => panic!("not asking: {other:?}"),
    }
}

/// Let go of everything, then play the card's keys.
fn play(drill: &mut Drill, keys: &[MidiNote]) -> Option<Answer> {
    drill.play(&[]);
    drill.play(keys)
}

/// Wait out the pause after an answer, so the next card is dealt.
fn next(drill: &mut Drill) {
    drill.play(&[]);
    drill.tick(Seconds(2.0));
}

/// Play a whole round: miss the cards `miss` picks (by letting time run
/// out), answer the rest. Returns the cards dealt.
fn round(drill: &mut Drill, miss: impl Fn(Card) -> bool) -> Vec<Card> {
    (0..ROUND)
        .map(|_| {
            let card = card(drill);
            if miss(card) {
                assert_eq!(drill.tick(TIME_LIMIT), Some(Answer::TimedOut));
            } else {
                assert!(matches!(play(drill, &card.keys()), Some(Answer::Right(_))));
            }
            next(drill);
            card
        })
        .collect()
}

#[test]
fn nothing_cleared_means_nothing_to_drill() {
    let mut drill = Drill::new(1);
    assert_eq!(drill.start(Vec::new()), Err(NothingCleared));
    assert!(drill.round().is_none());
}

#[test]
fn cards_are_the_cleared_challenges() {
    let drill = started(cleared(), 7);
    let deck: BTreeSet<(usize, Inversion)> = drill
        .deck()
        .iter()
        .map(|c| (c.page(), c.inversion()))
        .collect();
    assert_eq!(deck, cleared().into_iter().collect());
    let first = Card::new(0, Inversion::First).unwrap();
    assert_eq!(first.chord(), BOOK[0]);
    assert_eq!(first.label(), "C major, first inversion");
    assert_eq!(first.keys(), notes(&[64, 67, 72]));
    assert!(Card::new(24, Inversion::Root).is_none(), "no such page");
}

#[test]
fn a_round_deals_its_cards_then_ends_with_a_summary() {
    let mut drill = started(cleared(), 3);
    assert_eq!(drill.round().unwrap().number(), 1);
    let dealt = round(&mut drill, |_| false);
    let deck: BTreeSet<Card> = drill.deck().iter().copied().collect();
    assert!(dealt.iter().all(|c| deck.contains(c)));
    assert!(
        dealt.windows(2).all(|w| w[0] != w[1]),
        "never the same card twice running: {dealt:?}"
    );
    let round = drill.round().unwrap();
    assert_eq!(round.stage(), Stage::Over);
    let summary = round.summary();
    assert_eq!(
        (summary.right, summary.wrong, summary.timed_out),
        (ROUND, 0, 0)
    );
    assert!(summary.to_work_on.is_empty());
}

#[test]
fn the_right_chord_in_the_right_position_is_right() {
    let mut drill = started(vec![(1, Inversion::Root)], 1);
    assert_eq!(drill.tick(Seconds(1.5)), None);
    assert_eq!(play(&mut drill, &notes(&[57])), None, "one note so far");
    assert_eq!(
        play(&mut drill, &notes(&[57, 60])),
        None,
        "two notes so far"
    );
    assert_eq!(
        play(&mut drill, &notes(&[45, 57, 60, 64])),
        Some(Answer::Right(Seconds(1.5))),
        "doubling the root is fine"
    );
    let round = drill.round().unwrap();
    assert!(matches!(round.stage(), Stage::Marked { .. }));
    assert_eq!(drill.tick(Seconds(10.0)), None, "no clock while marked");
}

#[test]
fn another_position_or_chord_is_wrong() {
    let wrong = |keys: &[u8]| {
        let mut drill = started(vec![(0, Inversion::Root)], 1);
        play(&mut drill, &notes(keys))
    };
    assert_eq!(
        wrong(&[64, 67, 72]),
        Some(Answer::Wrong(Mistake::Inversion(Inversion::First)))
    );
    assert_eq!(
        wrong(&[57, 60, 64]),
        Some(Answer::Wrong(Mistake::Chord(BOOK[1])))
    );
    assert_eq!(
        wrong(&[60, 62, 64]),
        Some(Answer::Wrong(Mistake::NotAChord))
    );
}

#[test]
fn time_runs_out() {
    let mut drill = started(cleared(), 1);
    assert_eq!(drill.tick(Seconds(TIME_LIMIT.0 - 0.1)), None);
    assert!((drill.round().unwrap().left().0 - 0.1).abs() < 1e-4);
    assert_eq!(drill.tick(Seconds(0.2)), Some(Answer::TimedOut));
    assert_eq!(drill.round().unwrap().left(), Seconds(0.0));
}

#[test]
fn keys_held_from_before_a_card_do_not_answer_it() {
    let mut drill = started(vec![(0, Inversion::Root)], 1);
    let keys = card(&drill).keys();
    assert_eq!(drill.play(&keys), None, "still held from before");
    drill.play(&keys[..1]);
    assert_eq!(drill.play(&keys), None, "not every key let go yet");
    drill.play(&[]);
    assert!(matches!(drill.play(&keys), Some(Answer::Right(_))));
}

#[test]
fn a_missed_card_comes_back_soon() {
    let everything: Vec<(usize, Inversion)> = (0..BOOK.len())
        .flat_map(|page| Inversion::ALL.map(|i| (page, i)))
        .collect();
    let mut drill = started(everything, 11);
    let missed = card(&drill);
    drill.tick(TIME_LIMIT);
    next(&mut drill);
    let following: Vec<Card> = (0..=RETRY_AFTER)
        .map(|_| {
            let card = card(&drill);
            play(&mut drill, &card.keys());
            next(&mut drill);
            card
        })
        .collect();
    assert!(following.contains(&missed), "{following:?}");
}

#[test]
fn chords_you_miss_are_dealt_more_often() {
    // Six cards: C major and A minor in every position.
    let six: Vec<(usize, Inversion)> = (0..2)
        .flat_map(|page| Inversion::ALL.map(|i| (page, i)))
        .collect();
    let mut drill = Drill::new(5);
    let hard = Card::new(1, Inversion::Root).unwrap();
    let dealt: Vec<Card> = (0..6)
        .flat_map(|_| {
            drill.start(six.clone()).unwrap();
            round(&mut drill, |c| c == hard)
        })
        .collect();
    let hard_count = dealt.iter().filter(|c| **c == hard).count();
    let fair_share = dealt.len() / six.len();
    assert!(
        hard_count > fair_share * 2,
        "{hard_count} of {} dealt",
        dealt.len()
    );
}

#[test]
fn the_summary_counts_answers_and_lists_what_to_work_on() {
    let mut drill = started(cleared(), 9);
    let first = card(&drill);
    drill.tick(TIME_LIMIT);
    next(&mut drill);
    let second = card(&drill);
    play(&mut drill, &notes(&[60, 62, 64]));
    next(&mut drill);
    (2..ROUND).for_each(|_| {
        let card = card(&drill);
        drill.tick(Seconds(1.0));
        play(&mut drill, &card.keys());
        next(&mut drill);
    });
    let summary = drill.round().unwrap().summary();
    assert_eq!(
        (summary.right, summary.wrong, summary.timed_out),
        (ROUND - 2, 1, 1)
    );
    assert_eq!(summary.average, Some(Seconds(1.0)));
    assert_eq!(summary.to_work_on, vec![first, second]);
}

#[test]
fn stopping_ends_the_round() {
    let mut drill = started(cleared(), 1);
    drill.stop();
    assert!(drill.round().is_none());
    assert_eq!(drill.tick(TIME_LIMIT), None);
}
