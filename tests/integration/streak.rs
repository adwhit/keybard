use keybard::song::Seconds;
use keybard::streak::{Day, MIN_DAILY_PRACTICE, PracticeLog};

fn log(days: &[(u32, f32)]) -> PracticeLog {
    let mut log = PracticeLog::default();
    days.iter().for_each(|(d, s)| log.add(Day(*d), Seconds(*s)));
    log
}

#[test]
fn time_adds_up_per_day() {
    let mut l = log(&[(10, 30.0), (10, 45.0), (11, 5.0)]);
    assert_eq!(l.on(Day(10)), Seconds(75.0));
    assert_eq!(l.on(Day(12)), Seconds(0.0));
    l.add(Day(12), Seconds(1.5));
    assert_eq!(l.on(Day(12)), Seconds(1.5));
}

#[test]
fn streaks_count_consecutive_days_of_real_practice() {
    let enough = MIN_DAILY_PRACTICE.0;
    let l = log(&[(7, enough), (8, enough), (9, enough), (10, enough)]);
    assert_eq!(l.streak(Day(10)), 4);
    // Not practised yet today: yesterday's streak is still alive.
    assert_eq!(l.streak(Day(11)), 4);
    // Missed a whole day: gone.
    assert_eq!(l.streak(Day(12)), 0);
    // A token few seconds don't count.
    let l = log(&[(9, enough), (10, 3.0)]);
    assert_eq!(
        l.streak(Day(10)),
        1,
        "today too short, but yesterday counts"
    );
    let l = log(&[(8, enough), (9, 3.0), (10, enough)]);
    assert_eq!(l.streak(Day(10)), 1, "a too-short day breaks the chain");
}

#[test]
fn days_come_from_unix_time() {
    assert_eq!(Day::from_unix_seconds(0), Day(0));
    assert_eq!(Day::from_unix_seconds(86_399), Day(0));
    assert_eq!(Day::from_unix_seconds(86_400), Day(1));
    assert!(Day::today().0 > 20_000, "after 2024");
}

#[test]
fn round_trips_through_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sub").join("practice.tsv");
    assert_eq!(PracticeLog::load(&path).unwrap(), PracticeLog::default());
    let l = log(&[(100, 61.5), (101, 120.0)]);
    l.save(&path).unwrap();
    assert_eq!(PracticeLog::load(&path).unwrap(), l);
}

#[test]
fn days_follow_the_local_clock() {
    // 2026-11-23 23:30 in UTC-8 is 07:30 the next morning in UTC.
    let utc = 1_795_995_000_u64; // some instant
    let utc_day = Day::from_unix_seconds(utc);
    let behind = Day::local(utc, -8 * 3600);
    let ahead = Day::local(utc, 9 * 3600);
    assert!(behind.0 <= utc_day.0 && utc_day.0 <= ahead.0);
    // Exactly: an evening in California is still "yesterday" in UTC terms.
    let eleven_pm_pacific = 1_796_025_600 - 3600; // 07:00Z next day = 23:00 PST
    assert_eq!(
        Day::local(eleven_pm_pacific, -8 * 3600),
        Day(Day::from_unix_seconds(eleven_pm_pacific).0 - 1)
    );
    assert_eq!(Day::local(utc, 0), utc_day);
}
