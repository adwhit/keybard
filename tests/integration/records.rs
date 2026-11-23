use keybard::performance::{Hands, PlayMode, Score};
use keybard::records::{Best, RecordKey, Records, Submission};

fn key(title: &str, mode: PlayMode) -> RecordKey {
    RecordKey {
        title: title.to_owned(),
        mode,
        hands: Hands::Both,
    }
}

fn score(points: u32, perfect: usize, missed: usize, max_combo: u32) -> Score {
    Score {
        perfect,
        missed,
        points,
        max_combo,
        ..Score::default()
    }
}

#[test]
fn first_score_is_a_new_best() {
    let mut records = Records::default();
    let result = records.submit(key("Song", PlayMode::Play), &score(1000, 3, 1, 3));
    assert_eq!(result, Submission::NewBest { previous: None });
    let best = records.best(&key("Song", PlayMode::Play)).unwrap();
    assert_eq!(best.points, 1000);
    assert_eq!(best.max_combo, 3);
    assert!((best.accuracy - 0.75).abs() < 1e-6);
}

#[test]
fn only_higher_points_replace_the_best() {
    let mut records = Records::default();
    let k = key("Song", PlayMode::Play);
    records.submit(k.clone(), &score(1000, 4, 0, 4));
    let lower = records.submit(k.clone(), &score(900, 4, 0, 4));
    assert!(matches!(lower, Submission::NotBest { best } if best.points == 1000));
    let higher = records.submit(k.clone(), &score(1500, 4, 0, 4));
    assert!(matches!(
        higher,
        Submission::NewBest {
            previous: Some(Best { points: 1000, .. })
        }
    ));
    assert_eq!(records.best(&k).unwrap().points, 1500);
}

#[test]
fn modes_and_songs_are_tracked_separately() {
    let mut records = Records::default();
    records.submit(key("A", PlayMode::Play), &score(10, 1, 0, 1));
    records.submit(key("A", PlayMode::Wait), &score(20, 1, 0, 1));
    records.submit(key("B", PlayMode::Play), &score(30, 1, 0, 1));
    assert_eq!(records.best(&key("A", PlayMode::Play)).unwrap().points, 10);
    assert_eq!(records.best(&key("A", PlayMode::Wait)).unwrap().points, 20);
    assert_eq!(records.best(&key("B", PlayMode::Play)).unwrap().points, 30);
    assert!(records.best(&key("B", PlayMode::Wait)).is_none());
}

#[test]
fn empty_scores_are_not_recorded() {
    let mut records = Records::default();
    let result = records.submit(key("A", PlayMode::Play), &Score::default());
    assert_eq!(result, Submission::Unscored);
    assert!(records.best(&key("A", PlayMode::Play)).is_none());
}

#[test]
fn round_trips_through_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("best.tsv");
    let mut records = Records::load(&path).unwrap();
    records.submit(
        key("Tabs\tand\nnewlines", PlayMode::Wait),
        &score(42, 2, 1, 2),
    );
    records.submit(key("Plain", PlayMode::Play), &score(7, 1, 0, 1));
    records.save(&path).unwrap();

    let loaded = Records::load(&path).unwrap();
    assert_eq!(loaded, records);
    assert_eq!(
        loaded
            .best(&key("Tabs\tand\nnewlines", PlayMode::Wait))
            .unwrap()
            .points,
        42
    );
}

#[test]
fn missing_files_load_empty_and_bad_lines_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let missing = Records::load(&dir.path().join("none.tsv")).unwrap();
    assert_eq!(missing, Records::default());

    let path = dir.path().join("best.tsv");
    std::fs::write(&path, "garbage\nPlay\tboth hands\t100\t0.5\t3\tGood Song\n").unwrap();
    let loaded = Records::load(&path).unwrap();
    assert_eq!(
        loaded
            .best(&key("Good Song", PlayMode::Play))
            .unwrap()
            .points,
        100
    );
}

#[test]
fn chasing_a_best_counts_down_then_celebrates() {
    use keybard::records::{Chase, chase};
    let best = Best {
        points: 1000,
        accuracy: 0.9,
        max_combo: 10,
    };
    assert_eq!(chase(0, None), Chase::NoBest);
    assert_eq!(
        chase(300, Some(best)),
        Chase::Behind {
            best: 1000,
            to_go: 700
        }
    );
    // Equalling it isn't beating it.
    assert_eq!(
        chase(1000, Some(best)),
        Chase::Behind {
            best: 1000,
            to_go: 0
        }
    );
    assert_eq!(chase(1200, Some(best)), Chase::Ahead { by: 200 });
    assert_eq!(
        chase(300, Some(best)).label(),
        Some("best 1000 | 700 to go".to_owned())
    );
    assert_eq!(
        chase(1200, Some(best)).label(),
        Some("best beaten by 200!".to_owned())
    );
    assert_eq!(chase(5, None).label(), None);
}
