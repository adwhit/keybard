use keybard::builtin::BuiltinSong;
use keybard::notation::{RestShape, rests};
use keybard::song::{Hand, Song, Tempo, parse_voice};

/// 60bpm, so seconds == beats.
fn song(right: &str, left: &str, beats_per_bar: u8) -> Song {
    let tempo = Tempo::from_bpm(60.0, beats_per_bar);
    let notes = parse_voice(right, Hand::Right, tempo)
        .unwrap()
        .into_iter()
        .chain(parse_voice(left, Hand::Left, tempo).unwrap())
        .collect();
    Song::new("t", tempo, notes).unwrap()
}

fn summary(song: &Song, hand: Hand) -> Vec<(f32, RestShape)> {
    rests(song, hand)
        .iter()
        .map(|r| {
            (
                (song.tempo.to_beats(r.start).0 * 1000.0).round() / 1000.0,
                r.shape,
            )
        })
        .collect()
}

#[test]
fn a_gap_in_the_middle_becomes_a_rest() {
    let s = song("C4 R E4/2", "C3/4", 4);
    assert_eq!(summary(&s, Hand::Right), vec![(1.0, RestShape::Quarter)]);
    assert!(summary(&s, Hand::Left).is_empty());
}

#[test]
fn an_empty_bar_gets_a_whole_bar_rest() {
    let s = song("C4/4 C4/4", "R/4 C3/4", 4);
    assert_eq!(summary(&s, Hand::Left), vec![(0.0, RestShape::WholeBar)]);
}

#[test]
fn longer_gaps_split_into_standard_values_within_bars() {
    // Right hand rests for 3 beats: a half then a quarter.
    let s = song("C4 R/3 | C4/4", "C3/4 | C3/4", 4);
    assert_eq!(
        summary(&s, Hand::Right),
        vec![(1.0, RestShape::Half), (3.0, RestShape::Quarter)]
    );
    // A gap across a bar line is split at the bar line.
    let s = song("C4/3 R/2 C4/3", "C3/4 | C3/4", 4);
    assert_eq!(
        summary(&s, Hand::Right),
        vec![(3.0, RestShape::Quarter), (4.0, RestShape::Quarter)]
    );
}

#[test]
fn eighth_rests_and_tiny_gaps() {
    let s = song("C4/0.5 R/0.5 C4/3", "C3/4", 4);
    assert_eq!(summary(&s, Hand::Right), vec![(0.5, RestShape::Eighth)]);
    let s = song("C4/0.9 R/0.1 C4/3", "C3/4", 4);
    assert!(
        summary(&s, Hand::Right).is_empty(),
        "articulation gaps aren't rests"
    );
}

#[test]
fn fur_elise_left_hand_waits_two_bars() {
    let s = BuiltinSong::FurElise.load().unwrap();
    let left = summary(&s, Hand::Left);
    assert_eq!(left[0], (0.0, RestShape::WholeBar));
    assert_eq!(left[1], (3.0, RestShape::WholeBar));
    // After the first arpeggio (1.5 beats) the bar ends with a quarter and an eighth.
    assert_eq!(left[2], (7.5, RestShape::Quarter));
    assert_eq!(left[3], (8.5, RestShape::Eighth));
}
