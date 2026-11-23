//! Headless app tests: practice.

use super::*;

#[test]
fn practising_one_hand_ghosts_and_autoplays_the_other() {
    let mut h = Harness::with(
        song(),
        PlayMode::Play,
        Player::Autopilot,
        RecordStore::Memory,
    );
    h.tap(KeyCode::F2);
    assert_eq!(h.world().resource::<Judge>().0.hands(), Hands::Right);

    h.run_until(-1.0);
    let alpha = |h: &mut Harness, id: usize| {
        let mut q = h.world().query::<(&FallingBar, &Sprite)>();
        q.iter(h.app.world())
            .find(|(b, _)| b.0 == NoteId(id))
            .map(|(_, s)| s.color.alpha())
            .unwrap()
    };
    assert!(alpha(&mut h, C3) < 0.5, "left hand is a ghost");
    assert_eq!(alpha(&mut h, C4), 1.0, "right hand is yours");

    h.run_until(4.4);
    h.frames(30);
    let score = h.score();
    assert_eq!(score.hits(), 3, "the autopilot only plays the right hand");
    assert_eq!((score.missed, score.wrong), (0, 0));
    assert_eq!(h.status(C3), NoteStatus::AutoPlayed);
}

#[test]
fn looping_a_section_repeats_it_and_counts_passes() {
    use keybard::app::section::SectionLoop;
    let mut h = Harness::with(
        song(),
        PlayMode::Play,
        Player::Autopilot,
        RecordStore::Memory,
    );
    h.run_until(0.5);
    h.tap(KeyCode::F3);
    let region = {
        let section = h.world().resource::<SectionLoop>();
        section.0.as_ref().expect("looping").region
    };
    assert_eq!(region.start, Seconds(0.0));
    assert_eq!(region.end, Seconds(3.0), "clipped to the song");

    let passes = |h: &Harness| {
        h.app
            .world()
            .resource::<SectionLoop>()
            .0
            .as_ref()
            .map(|l| (l.passes, l.clean_passes))
    };
    let mut max_time = 0.0_f32;
    while passes(&h).is_some_and(|(p, _)| p < 2) {
        h.app.update();
        max_time = max_time.max(h.time().0);
        assert!(h.time().0 < 60.0, "loop never wrapped");
    }
    assert!(max_time < 3.05, "never ran past the loop: {max_time}");
    assert_eq!(
        passes(&h),
        Some((2, 2)),
        "the autopilot plays both passes cleanly"
    );
    assert_ne!(h.transport().state, PlayState::Finished);
    let last = h
        .world()
        .resource::<SectionLoop>()
        .0
        .as_ref()
        .unwrap()
        .last_pass
        .unwrap();
    assert_eq!(
        (last.hits, last.missed, last.wrong),
        (4, 0, 0),
        "every note re-armed and hit"
    );

    h.tap(KeyCode::F3);
    assert!(
        h.world().resource::<SectionLoop>().0.is_none(),
        "F3 again clears"
    );
}

#[test]
fn starting_with_a_loop_begins_just_before_that_bar() {
    use keybard::app::section::SectionLoop;
    let song = keybard::builtin::BuiltinSong::OdeToJoy.load().unwrap();
    let bar = song.tempo.bar_length();
    let mut h = Harness::build(Options {
        song,
        mode: PlayMode::Wait,
        midi_input: MidiInput::Disabled,
        backend: Backend::Headless,
        logging: Logging::Disabled,
        player: Player::Human,
        records: RecordStore::Memory,
        hands: Hands::Both,
        loop_from: Some(5),
        playlist: Default::default(),
        practice_log: RecordStore::Memory,
    });
    let region = h
        .world()
        .resource::<SectionLoop>()
        .0
        .as_ref()
        .unwrap()
        .region;
    assert!(
        (region.start.0 - 4.0 * bar.0).abs() < 1e-4,
        "bar 5 starts after 4 bars"
    );
    assert!(h.time() < region.start, "begins with a lead-in");
    assert!(region.start.0 - h.time().0 >= bar.0 - 0.05);
}

#[test]
fn clean_loop_passes_speed_up_towards_full_tempo() {
    use keybard::app::section::SectionLoop;
    let mut h = Harness::with(
        song(),
        PlayMode::Play,
        Player::Autopilot,
        RecordStore::Memory,
    );
    (0..6).for_each(|_| h.tap(KeyCode::ArrowDown));
    assert!((h.transport().speed.get() - 0.7).abs() < 1e-5);
    h.run_until(0.5);
    h.tap(KeyCode::F3);
    let passes = |h: &Harness| {
        h.app
            .world()
            .resource::<SectionLoop>()
            .0
            .as_ref()
            .unwrap()
            .passes
    };
    let mut announced = false;
    while passes(&h) < 2 {
        h.app.update();
        announced |= h.texts().contains(&"SPEED 75%".to_owned());
    }
    assert!(announced, "step-ups are announced");
    assert!(
        (h.transport().speed.get() - 0.8).abs() < 1e-5,
        "two clean passes: +10%"
    );
}

#[test]
fn playing_time_is_logged_and_the_streak_shown() {
    use keybard::app::practice::Practice;
    use keybard::streak::MIN_DAILY_PRACTICE;
    let mut h = Harness::new(song(), PlayMode::Listen);
    let today = h.world().resource::<Practice>().today;
    let yesterday = keybard::streak::Day(today.0 - 1);
    h.world()
        .resource_mut::<Practice>()
        .log
        .add(yesterday, MIN_DAILY_PRACTICE);
    h.run_until(1.0);
    let logged = h.world().resource::<Practice>().log.on(today).0;
    assert!(logged > 4.5 && logged < 5.5, "lead-in + a second: {logged}");
    // Paused time doesn't count.
    h.tap(KeyCode::Space);
    h.frames(120);
    let paused = h.world().resource::<Practice>().log.on(today).0;
    assert!((paused - logged).abs() < 0.1);
    let info: Vec<String> = {
        let mut q = h.world().query::<&Text>();
        q.iter(h.app.world()).map(|t| t.0.clone()).collect()
    };
    assert!(
        info.iter()
            .any(|t| t.contains("1-day streak") && t.contains("0 min today")),
        "{info:?}"
    );
}

#[test]
fn crossing_the_daily_minimum_celebrates_the_streak() {
    use keybard::app::practice::Practice;
    use keybard::song::Seconds;
    use keybard::streak::{Day, MIN_DAILY_PRACTICE};
    let mut h = Harness::new(song(), PlayMode::Listen);
    let today = h.world().resource::<Practice>().today;
    {
        let mut practice = h.world().resource_mut::<Practice>();
        practice.log.add(Day(today.0 - 2), MIN_DAILY_PRACTICE);
        practice.log.add(Day(today.0 - 1), MIN_DAILY_PRACTICE);
        practice.log.add(today, Seconds(MIN_DAILY_PRACTICE.0 - 0.5));
    }
    let mut celebrated = false;
    (0..60).for_each(|_| {
        h.app.update();
        celebrated |= h.texts().contains(&"3-DAY STREAK!".to_owned());
    });
    assert!(celebrated);
    assert_eq!(h.world().resource::<Practice>().streak(), 3);
}
