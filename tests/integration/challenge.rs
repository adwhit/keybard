use keybard::challenge::{Challenge, TARGET_GRADE, for_day};
use keybard::performance::{Grade, PlayMode};
use keybard::streak::Day;

#[test]
fn the_challenge_rotates_through_the_songs_daily() {
    let picks: Vec<usize> = (0..6).map(|d| for_day(Day(20_000 + d), 3).song).collect();
    assert!(picks.iter().all(|i| *i < 3));
    assert_ne!(picks[0], picks[1], "changes from one day to the next");
    assert_eq!(picks[0], picks[3], "cycles");
    assert_eq!(for_day(Day(5), 3), for_day(Day(5), 3), "same all day");
}

#[test]
fn completing_needs_the_song_the_mode_and_the_grade() {
    let challenge = Challenge { song: 1 };
    assert!(challenge.is_met_by(1, PlayMode::Play, Some(TARGET_GRADE)));
    assert!(challenge.is_met_by(1, PlayMode::Play, Some(Grade::S)));
    assert!(
        !challenge.is_met_by(0, PlayMode::Play, Some(Grade::S)),
        "wrong song"
    );
    assert!(
        !challenge.is_met_by(1, PlayMode::Wait, Some(Grade::S)),
        "wrong mode"
    );
    assert!(
        !challenge.is_met_by(1, PlayMode::Play, Some(Grade::D)),
        "not good enough"
    );
    assert!(!challenge.is_met_by(1, PlayMode::Play, None));
}
