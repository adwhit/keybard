use keybard::achievements::{Achievement, Achievements, RunSummary};
use keybard::performance::{Hands, PlayMode, Score};

fn run(score: Score) -> RunSummary {
    RunSummary {
        score,
        mode: PlayMode::Play,
        hands: Hands::Both,
        speed: 1.0,
        streak: 1,
        minutes_today: 5,
        songs_with_bests: 1,
        sight_reading: false,
    }
}

fn perfect(n: usize) -> Score {
    Score {
        perfect: n,
        max_combo: n as u32,
        combo: n as u32,
        points: 1000,
        ..Score::default()
    }
}

#[test]
fn a_flawless_run_earns_a_haul() {
    let earned = Achievement::earned_by(&run(perfect(120)));
    [
        Achievement::FirstClear,
        Achievement::FullCombo,
        Achievement::GradeS,
        Achievement::Combo50,
        Achievement::Combo100,
        Achievement::Flawless,
        Achievement::BothHandsFullCombo,
    ]
    .iter()
    .for_each(|a| assert!(earned.contains(a), "missing {a:?} from {earned:?}"));
    assert!(!earned.contains(&Achievement::Streak3));
}

#[test]
fn nothing_for_a_run_with_no_hits() {
    let score = Score {
        missed: 10,
        ..Score::default()
    };
    assert!(Achievement::earned_by(&run(score)).is_empty());
}

#[test]
fn one_hand_full_combos_dont_count_as_both_hands() {
    let mut summary = run(perfect(10));
    summary.hands = Hands::Right;
    let earned = Achievement::earned_by(&summary);
    assert!(earned.contains(&Achievement::FullCombo));
    assert!(!earned.contains(&Achievement::BothHandsFullCombo));
}

#[test]
fn habits_and_range_are_rewarded() {
    let mut summary = run(perfect(5));
    summary.streak = 7;
    summary.minutes_today = 31;
    summary.songs_with_bests = 3;
    summary.speed = 1.25;
    let earned = Achievement::earned_by(&summary);
    [
        Achievement::Streak3,
        Achievement::Streak7,
        Achievement::Marathon,
        Achievement::Explorer,
        Achievement::SpeedDemon,
    ]
    .iter()
    .for_each(|a| assert!(earned.contains(a), "missing {a:?}"));
}

#[test]
fn unlocking_reports_only_new_ones_and_round_trips() {
    let mut owned = Achievements::default();
    let first = owned.unlock(&run(perfect(10)));
    assert!(first.contains(&Achievement::FullCombo));
    let again = owned.unlock(&run(perfect(10)));
    assert!(again.is_empty(), "already owned: {again:?}");
    assert_eq!(owned.count(), first.len());
    assert!(Achievement::ALL.len() >= 12);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a").join("achievements.txt");
    owned.save(&path).unwrap();
    assert_eq!(Achievements::load(&path).unwrap(), owned);
    assert_eq!(
        Achievements::load(&dir.path().join("none")).unwrap(),
        Achievements::default()
    );
}

#[test]
fn every_achievement_has_a_title_and_description() {
    Achievement::ALL.iter().for_each(|a| {
        assert!(!a.title().is_empty() && a.title().is_ascii());
        assert!(!a.description().is_empty());
    });
}

#[test]
fn sight_reading_well_earns_its_own_achievement() {
    let mut summary = run(perfect(10));
    assert!(!Achievement::earned_by(&summary).contains(&Achievement::SightReader));
    summary.sight_reading = true;
    assert!(Achievement::earned_by(&summary).contains(&Achievement::SightReader));
    // Not for a scrappy run.
    let scrappy = Score {
        good: 5,
        missed: 5,
        ..Score::default()
    };
    let mut poor = run(scrappy);
    poor.sight_reading = true;
    assert!(!Achievement::earned_by(&poor).contains(&Achievement::SightReader));
}
