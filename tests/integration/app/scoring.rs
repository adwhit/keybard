//! Headless app tests: scoring.

use super::*;
use keybard::app::results::ChallengeOutcome;

#[test]
fn wait_mode_holds_until_the_right_key_is_played() {
    let mut h = Harness::new(song(), PlayMode::Wait);
    h.run_until(0.0);
    h.frames(30);
    assert_eq!(h.time(), Seconds(0.0), "waiting for C3+C4");

    h.midi(60, KeyEdge::Down);
    h.frames(10);
    assert_eq!(h.time(), Seconds(0.0), "still waiting for the left hand");
    assert!(matches!(h.status(C4), NoteStatus::Hit(_)));
    assert!(h.world().resource::<HeldKeys>().0.contains(&n(60)));
    assert_eq!(
        h.key_color(60),
        palette::hand_lit(Hand::Right),
        "held and correct"
    );

    h.midi(48, KeyEdge::Down);
    h.midi(60, KeyEdge::Up);
    assert!(!h.world().resource::<HeldKeys>().0.contains(&n(60)));
    h.frames(10);
    assert!(h.time().0 > 0.0, "released");
    h.run_until(1.0);
    h.frames(10);
    assert_eq!(h.time(), Seconds(1.0), "gated at E4");
}

#[test]
fn computer_keyboard_plays_notes() {
    let mut h = Harness::new(song(), PlayMode::Wait);
    h.run_until(0.0);
    // Z is C3, Q is C4 in the default octave.
    h.key(KeyCode::KeyZ, ButtonState::Pressed);
    h.key(KeyCode::KeyQ, ButtonState::Pressed);
    let held = h.world().resource::<HeldKeys>().0.clone();
    assert!(held.contains(&n(48)) && held.contains(&n(60)));
    h.frames(5);
    assert!(h.time().0 > 0.0);
    assert!(matches!(h.status(C3), NoteStatus::Hit(_)));
    assert!(matches!(h.status(C4), NoteStatus::Hit(_)));

    // Shift up an octave: Z now plays C4.
    h.key(KeyCode::KeyZ, ButtonState::Released);
    h.tap(KeyCode::Equal);
    h.key(KeyCode::KeyZ, ButtonState::Pressed);
    assert!(h.world().resource::<HeldKeys>().0.contains(&n(60)));
}

#[test]
fn play_mode_counts_misses_and_wrong_notes() {
    let mut h = Harness::new(song(), PlayMode::Play);
    h.run_until(-0.05);
    h.midi(60, KeyEdge::Down);
    h.midi(61, KeyEdge::Down);
    h.run_until(3.5);
    let score = h.world().resource::<Judge>().0.score();
    assert_eq!((score.hits(), score.missed, score.wrong), (1, 3, 1));
    assert_eq!(score.combo, 0);
    assert_eq!(
        h.key_color(61),
        palette::PRESSED,
        "held stray key stays lit"
    );
    assert_eq!(h.status(G4), NoteStatus::Missed);
}

#[test]
fn midi_touch_is_judged_against_the_dynamics() {
    use keybard::dynamics::Touch;
    use keybard::dynamics::Velocity;
    let mut h = Harness::new(song(), PlayMode::Play);
    h.run_until(-0.02);
    // The test song is unmarked (mf); slam the first note.
    h.midi_at(60, KeyEdge::Down, Velocity::new(127));
    let dynamics = h.world().resource::<Judge>().0.insights(&song()).dynamics;
    let dynamics = dynamics.expect("MIDI velocity reaches the judge");
    assert_eq!(dynamics.lean, Touch::TooLoud);
}

#[test]
fn far_off_touch_gets_a_softer_or_louder_cue() {
    use keybard::dynamics::Velocity;
    // The test song is unmarked, i.e. mezzo-forte.
    let mut h = Harness::new(song(), PlayMode::Play);
    h.run_until(-0.02);
    h.midi_at(60, KeyEdge::Down, Velocity::new(127));
    assert!(h.texts().contains(&"softer".to_owned()), "{:?}", h.texts());
    // Close to the marking: no nagging.
    let mut h = Harness::new(song(), PlayMode::Play);
    h.run_until(-0.02);
    h.midi_at(60, KeyEdge::Down, Velocity::new(90));
    assert!(!h.texts().iter().any(|t| t == "softer" || t == "louder"));
    // Too soft, on the next note.
    h.run_until(0.98);
    h.midi_at(64, KeyEdge::Down, Velocity::new(30));
    assert!(h.texts().contains(&"louder".to_owned()), "{:?}", h.texts());
}

#[test]
fn touch_cues_do_not_nag_on_every_note() {
    use keybard::dynamics::Velocity;
    let mut h = Harness::new(song(), PlayMode::Play);
    h.run_until(-0.02);
    h.midi_at(60, KeyEdge::Down, Velocity::new(127));
    h.run_until(0.3);
    let cues = |h: &mut Harness| h.texts().iter().filter(|t| *t == "softer").count();
    assert_eq!(cues(&mut h), 1);
    // A second slam a beat later is too soon for another cue.
    h.run_until(0.98);
    h.midi_at(64, KeyEdge::Down, Velocity::new(127));
    assert_eq!(cues(&mut h), 0, "first cue faded, no new one yet");
}

#[test]
fn computer_keys_have_no_touch_to_judge() {
    let mut h = Harness::new(song(), PlayMode::Play);
    h.run_until(-0.02);
    h.tap(KeyCode::Equal);
    h.key(KeyCode::KeyZ, ButtonState::Pressed);
    let insights = h.world().resource::<Judge>().0.insights(&song());
    assert_eq!(insights.right_accuracy, Some(1.0), "the key hit C4");
    assert_eq!(insights.dynamics, None);
}

#[test]
fn hits_spawn_particles_popups_and_progress_ticks() {
    let mut h = Harness::new(song(), PlayMode::Play);
    h.run_until(-0.01);
    h.midi(60, KeyEdge::Down);
    assert!(h.count::<Particle>() > 10, "a perfect hit bursts");
    assert!(h.count::<KeyFlash>() >= 1);
    assert_eq!(h.count::<Popup>(), 1);
    assert!(h.texts().contains(&"PERFECT".to_owned()));
    assert_eq!(h.count::<ProgressTick>(), 1);
    // Effects are short-lived, and keep animating while paused.
    h.tap(KeyCode::Space);
    h.frames(90);
    assert_eq!(h.count::<Particle>(), 0);
    assert_eq!(h.count::<Popup>(), 0);
}

#[test]
fn misses_announce_themselves() {
    let mut h = Harness::new(song(), PlayMode::Play);
    h.run_until(0.25);
    assert!(h.texts().contains(&"MISS".to_owned()));
}

#[test]
fn autopilot_plays_a_whole_song_and_earns_a_result() {
    // Short and quick, but with chords and both hands.
    let tempo = Tempo::from_bpm(240.0, 4);
    let notes = parse_voice("C4 E4 G4 C5 B4 G4 E4+G4 D4 C4/2", Hand::Right, tempo)
        .unwrap()
        .into_iter()
        .chain(parse_voice("C3/2 G2/2 C3+E3/2 G2/2", Hand::Left, tempo).unwrap())
        .collect();
    let song = Song::new("quick", tempo, notes).unwrap();
    let notes = song.notes().len();
    let mut h = Harness::with(song, PlayMode::Play, Player::Autopilot, Saves::memory());
    let end = h.world().resource::<ActiveSong>().0.duration().0;
    h.run_until(end + 1.4);
    h.frames(30);
    let score = h.score();
    assert_eq!(score.hits(), notes, "{score:?}");
    assert_eq!((score.missed, score.wrong), (0, 0));
    assert!(
        score.perfect > 0 && score.great > 0,
        "jitter gives a mix: {score:?}"
    );
    assert_eq!(score.max_combo as usize, notes);
    assert!(score.is_full_combo());

    let result = h.world().resource::<LastResult>().clone();
    assert_eq!(result.submission, Submission::NewBest { previous: None });
    assert_eq!(h.count::<ResultsCard>(), 1);

    // Replaying clears the card and resets the score.
    h.tap(KeyCode::Space);
    assert_eq!(h.count::<ResultsCard>(), 0);
    assert_eq!(h.score(), Score::default());
}

#[test]
fn the_hud_chases_your_personal_best_live() {
    let dir = tempfile::tempdir().unwrap();
    let saves = Saves::at(dir.path());
    let path = saves.file(&saves.current(), SaveFile::Bests).unwrap();
    // A best of 400 points in Play mode, both hands.
    std::fs::write(
        &path,
        format!(
            "{}\t{}\t400\t0.5\t1\ttest\n",
            PlayMode::Play.label(),
            Hands::Both.label()
        ),
    )
    .unwrap();
    let mut h = Harness::with(song(), PlayMode::Play, Player::Human, saves);
    h.run_until(-0.5);
    assert!(
        h.ui_texts()
            .iter()
            .any(|t| t.contains("best 400 | 400 to go")),
        "{:?}",
        h.ui_texts()
    );
    h.run_until(-0.01);
    h.midi(60, KeyEdge::Down);
    h.frames(2);
    assert!(h.ui_texts().iter().any(|t| t.contains("100 to go")));
    assert!(!h.texts().contains(&"BEST BEATEN!".to_owned()));
    h.run_until(0.99);
    h.midi(64, KeyEdge::Down);
    h.frames(2);
    assert!(
        h.texts().contains(&"BEST BEATEN!".to_owned()),
        "{:?}",
        h.texts()
    );
    assert!(h.ui_texts().iter().any(|t| t.contains("best beaten by")));
    // Only once per run.
    h.run_until(1.99);
    h.midi(67, KeyEdge::Down);
    h.frames(2);
    // The first pop (at 1s) has faded; passing further doesn't pop again.
    assert!(
        !h.texts().contains(&"BEST BEATEN!".to_owned()),
        "no second pop"
    );
}

#[test]
fn personal_bests_are_saved_to_disk() {
    let dir = tempfile::tempdir().unwrap();
    let saves = Saves::at(dir.path());
    let path = saves.file(&saves.current(), SaveFile::Bests).unwrap();
    let mut h = Harness::with(song(), PlayMode::Wait, Player::Autopilot, saves);
    h.run_until(4.4);
    h.frames(30);
    assert_eq!(h.transport().state, PlayState::Finished);

    let records = Records::load(&path).unwrap();
    let best = records
        .best(&RecordKey {
            title: "test".to_owned(),
            mode: PlayMode::Wait,
            hands: Hands::Both,
        })
        .expect("best recorded");
    assert_eq!(best.points, h.score().points);
    assert!(best.points > 0);

    // A second run of the same app sees the earlier best.
    h.tap(KeyCode::Space);
    h.run_until(4.4);
    h.frames(30);
    let result = h.world().resource::<LastResult>().clone();
    assert!(matches!(result.submission, Submission::NotBest { .. }));
}

#[test]
fn listen_mode_is_not_scored_and_shows_no_results() {
    let mut h = Harness::with(song(), PlayMode::Listen, Player::Autopilot, Saves::memory());
    h.run_until(4.4);
    h.frames(30);
    assert_eq!(h.score(), Score::default());
    assert!(h.world().get_resource::<LastResult>().is_none());
    assert_eq!(h.count::<ResultsCard>(), 0);
}

#[test]
fn autopilot_jitter_is_bounded_and_varied() {
    let offsets: Vec<f32> = (0..200)
        .map(|i| keybard::app::autopilot_jitter(NoteId(i)).0)
        .collect();
    assert!(offsets.iter().all(|o| o.abs() <= 0.0701));
    let timings: std::collections::HashSet<_> = offsets
        .iter()
        .filter_map(|o| Timing::from_offset(Seconds(*o)))
        .collect();
    assert!(timings.len() >= 2);
}

#[test]
fn a_hot_streak_sets_the_screen_on_fire_and_breaking_it_cools_down() {
    use keybard::app::heat::{Ember, HeatGlow};
    use keybard::performance::Heat;
    let tempo = Tempo::from_bpm(240.0, 4);
    let notes = parse_voice(&"C4 E4 G4 E4 ".repeat(10), Hand::Right, tempo).unwrap();
    let song = Song::new("streak", tempo, notes).unwrap();
    let mut h = Harness::with(song, PlayMode::Play, Player::Autopilot, Saves::memory());

    let mut saw_on_fire_banner = false;
    while h.score().heat() != Heat::OnFire {
        h.app.update();
        assert!(h.time().0 < 30.0, "never got hot");
    }
    // Let the glow catch up and the embers start.
    (0..60).for_each(|_| {
        h.app.update();
        saw_on_fire_banner |= h.texts().contains(&"ON FIRE!".to_owned());
    });
    assert!(saw_on_fire_banner);
    assert!(h.world().resource::<HeatGlow>().0 > 0.9);
    assert!(h.count::<Ember>() > 10, "embers drift up");

    // A wrong note breaks the streak.
    h.midi(30, KeyEdge::Down);
    assert_eq!(h.score().heat(), Heat::Cold);
    assert!(h.texts().contains(&"STREAK LOST".to_owned()));
    h.tap(KeyCode::Space); // pause so the autopilot can't rebuild it
    h.frames(90);
    assert!(h.world().resource::<HeatGlow>().0 < 0.1, "cools down");
    assert_eq!(h.count::<Ember>(), 0);
}

#[test]
fn results_show_per_bar_accuracy_and_f3_drills_the_weakest_bars() {
    use keybard::app::results::BarCell;
    use keybard::app::section::SectionLoop;
    // 240bpm 4/4: six one-second bars.
    let tempo = Tempo::from_bpm(240.0, 4);
    let notes = parse_voice(&"C4 E4 G4 E4 ".repeat(6), Hand::Right, tempo).unwrap();
    let song = Song::new("bars", tempo, notes).unwrap();
    let mut h = Harness::with(song, PlayMode::Play, Player::Human, Saves::memory());
    // Play nothing at all: every bar is missed.
    while h.transport().state != PlayState::Finished {
        h.app.update();
    }
    h.frames(2);
    let result = h.world().resource::<LastResult>().clone();
    assert_eq!(result.bars, vec![Some(0.0); 6]);
    assert_eq!(result.weakest, Some(1));
    assert_eq!(h.count::<BarCell>(), 6);
    let texts: Vec<String> = {
        let mut q = h.world().query::<&Text>();
        q.iter(h.app.world()).map(|t| t.0.clone()).collect()
    };
    assert!(texts.iter().any(|t| t.contains("bars 1-4")), "{texts:?}");
    assert!(
        texts
            .iter()
            .any(|t| t == "Most missed: E4 x12, C4 x6, G4 x6"),
        "{texts:?}"
    );
    assert!(
        texts.iter().any(|t| t == "Keep practising!"),
        "no fanfare for zero hits: {texts:?}"
    );
    assert!(texts.iter().any(|t| t == "Right hand 0%"), "{texts:?}");

    h.tap(KeyCode::F3);
    let region = h
        .world()
        .resource::<SectionLoop>()
        .0
        .as_ref()
        .expect("drilling")
        .region;
    assert_eq!(region.start, Seconds(0.0));
    assert_eq!(region.end, Seconds(4.0));
    assert_eq!(h.transport().state, PlayState::Playing);
    assert!(h.time() < region.start, "starts with a lead-in");
    assert_eq!(h.score(), Score::default(), "fresh score for the drill");
    assert!(
        (h.transport().speed.get() - 0.7).abs() < 1e-5,
        "drills start slow"
    );
    assert_eq!(h.count::<ResultsCard>(), 0);
}

#[test]
fn the_hud_says_when_midi_is_off() {
    use keybard::app::input::MidiStatus;
    let mut h = Harness::new(song(), PlayMode::Listen);
    assert_eq!(*h.world().resource::<MidiStatus>(), MidiStatus::Off);
    let texts: Vec<String> = {
        let mut q = h.world().query::<&Text>();
        q.iter(h.app.world()).map(|t| t.0.clone()).collect()
    };
    assert!(texts.iter().any(|t| t.contains("MIDI off")), "{texts:?}");
    assert_eq!(
        MidiStatus::Connected("Yamaha P-125".into()).label(),
        "MIDI: Yamaha P-125"
    );
    // Unplugging isn't fatal: the HUD says it's waiting for the keyboard.
    assert_eq!(
        MidiStatus::Unplugged("Yamaha P-125".into()).label(),
        "MIDI: Yamaha P-125 unplugged, waiting for it"
    );
    assert!(MidiStatus::NoDevices.label().contains("plug one in"));
}

#[test]
fn finishing_well_unlocks_achievements_once() {
    use keybard::achievements::Achievement;
    use keybard::app::results::Unlocked;
    let mut h = Harness::with(song(), PlayMode::Play, Player::Autopilot, Saves::memory());
    while h.transport().state != PlayState::Finished {
        h.app.update();
    }
    h.frames(2);
    let first = h.world().resource::<LastResult>().new_achievements.clone();
    assert!(first.contains(&Achievement::FirstClear), "{first:?}");
    assert!(first.contains(&Achievement::FullCombo), "{first:?}");
    let texts: Vec<String> = {
        let mut q = h.world().query::<&Text>();
        q.iter(h.app.world()).map(|t| t.0.clone()).collect()
    };
    assert!(
        texts.iter().any(|t| t.starts_with("UNLOCKED  Unbroken")),
        "{texts:?}"
    );

    h.tap(KeyCode::Space);
    while h.transport().state != PlayState::Finished {
        h.app.update();
    }
    h.frames(2);
    assert!(
        h.world()
            .resource::<LastResult>()
            .new_achievements
            .is_empty(),
        "nothing new the second time"
    );
    assert_eq!(
        h.world().resource::<Unlocked>().achievements.count(),
        first.len()
    );
}

#[test]
fn achievements_are_saved_next_to_the_bests() {
    let dir = tempfile::tempdir().unwrap();
    let saves = Saves::at(dir.path());
    let achievements = saves
        .file(&saves.current(), SaveFile::Achievements)
        .unwrap();
    let mut h = Harness::with(song(), PlayMode::Wait, Player::Autopilot, saves);
    while h.transport().state != PlayState::Finished {
        h.app.update();
    }
    h.frames(2);
    let saved = std::fs::read_to_string(achievements).unwrap();
    assert!(saved.contains("first-clear"), "{saved}");
}

#[test]
fn f5_shows_the_achievement_gallery() {
    use keybard::achievements::Achievement;
    use keybard::app::results::{AchievementGallery, Unlocked};
    let mut h = Harness::new(song(), PlayMode::Play);
    let gallery = |h: &mut Harness| {
        let mut q = h
            .world()
            .query_filtered::<(&Text, &Visibility), With<AchievementGallery>>();
        let (text, vis) = q.single(h.app.world()).unwrap();
        (text.0.clone(), *vis != Visibility::Hidden)
    };
    assert!(!gallery(&mut h).1, "hidden until asked for");
    h.world()
        .resource_mut::<Unlocked>()
        .achievements
        .grant(Achievement::FullCombo);
    h.tap(KeyCode::F5);
    let (text, shown) = gallery(&mut h);
    assert!(shown);
    assert!(text.contains("ACHIEVEMENTS 1/13"), "{text}");
    assert!(text.contains("[x] Unbroken"), "{text}");
    assert!(text.contains("[ ] S Rank"), "{text}");
    assert_eq!(text.lines().count(), 1 + Achievement::ALL.len());
    h.tap(KeyCode::F5);
    assert!(!gallery(&mut h).1);
}

#[test]
fn wrong_notes_suggest_the_right_one() {
    let mut h = Harness::new(song(), PlayMode::Play);
    h.run_until(0.95);
    h.midi(62, KeyEdge::Down); // D4 while E4 is due
    assert!(h.texts().contains(&"try E4".to_owned()), "{:?}", h.texts());
    // Two flashes: the wrong key and the wanted one.
    assert!(h.count::<KeyFlash>() >= 2);
}

#[test]
fn sight_reading_a_song_well_unlocks_sight_reader() {
    use keybard::achievements::Achievement;
    use keybard::app::falling::View;
    let mut h = Harness::with(song(), PlayMode::Play, Player::Autopilot, Saves::memory());
    h.tap(KeyCode::F6);
    assert_eq!(*h.world().resource::<View>(), View::SightReading);
    while h.transport().state != PlayState::Finished {
        h.app.update();
    }
    h.frames(2);
    let unlocked = h.world().resource::<LastResult>().new_achievements.clone();
    assert!(unlocked.contains(&Achievement::SightReader), "{unlocked:?}");
}

#[test]
fn a_bar_of_perfect_hits_is_celebrated() {
    use keybard::app::effects::PerfectBarFlash;
    let mut h = Harness::new(song(), PlayMode::Play);
    // Every note of the one bar (C3+C4 at 0, E4 at 1, G4 at 2), on the beat.
    [(0.0, vec![48, 60]), (1.0, vec![64]), (2.0, vec![67])]
        .into_iter()
        .for_each(|(t, notes)| {
            h.run_until(t);
            notes.iter().for_each(|n| h.midi(*n, KeyEdge::Down));
        });
    let mut celebrated = false;
    while h.time().0 < 4.4 && h.transport().state == PlayState::Playing {
        h.app.update();
        celebrated |= h.texts().contains(&"PERFECT BAR".to_owned());
    }
    assert!(celebrated, "score: {:?}", h.score());
    assert!(h.count::<PerfectBarFlash>() <= 1);
}

#[test]
fn imperfect_bars_pass_quietly() {
    let mut h = Harness::new(song(), PlayMode::Play);
    h.run_until(0.15);
    h.midi(60, KeyEdge::Down); // late: Good, not Perfect
    let mut celebrated = false;
    while h.time().0 < 4.4 && h.transport().state == PlayState::Playing {
        h.app.update();
        celebrated |= h.texts().contains(&"PERFECT BAR".to_owned());
    }
    assert!(!celebrated);
}

#[test]
fn todays_challenge_is_listed_and_completing_it_is_announced() {
    use keybard::app::practice::Practice;
    use keybard::app::songs::{Playlist, SongSource};
    use keybard::builtin::BuiltinSong;
    use keybard::challenge::for_day;
    let songs = [BuiltinSong::Twinkle, BuiltinSong::Scale];
    let today = keybard::streak::Day::today();
    let pick = for_day(today, songs.len()).song;
    let mut h = Harness::build(Options {
        song: songs[pick].load().unwrap(),
        settings: Settings {
            mode: PlayMode::Play,
            ..Settings::default()
        },
        midi_input: MidiInput::Disabled,
        backend: Backend::Headless,
        logging: Logging::Disabled,
        player: Player::Autopilot,
        saves: Saves::memory(),
        loop_from: None,
        playlist: Playlist::new(
            songs.iter().map(|b| SongSource::Builtin(*b)).collect(),
            pick,
        ),
        screen: keybard::app::menu::Screen::Song,
    });
    assert_eq!(h.world().resource::<Practice>().today, today);
    while h.transport().state != PlayState::Finished {
        h.app.update();
    }
    h.frames(2);
    assert!(matches!(
        h.world().resource::<LastResult>().challenge,
        ChallengeOutcome::Completed { .. }
    ));
    let texts: Vec<String> = {
        let mut q = h.world().query::<&Text>();
        q.iter(h.app.world()).map(|t| t.0.clone()).collect()
    };
    assert!(
        texts.iter().any(|t| t == "DAILY CHALLENGE COMPLETE!"),
        "{texts:?}"
    );

    // Again the same day: remembered, not re-celebrated.
    h.tap(KeyCode::Space);
    while h.transport().state != PlayState::Finished {
        h.app.update();
    }
    h.frames(2);
    assert_eq!(
        h.world().resource::<LastResult>().challenge,
        ChallengeOutcome::AlreadyDone
    );
    // The start screen remembers it too.
    h.tap(KeyCode::Escape);
    assert!(
        h.ui_texts().iter().any(|t| t.contains("(done!)")),
        "{:?}",
        h.ui_texts()
    );
}

#[test]
fn other_songs_dont_complete_the_challenge() {
    let mut h = Harness::with(song(), PlayMode::Play, Player::Autopilot, Saves::memory());
    while h.transport().state != PlayState::Finished {
        h.app.update();
    }
    h.frames(2);
    assert!(
        h.world().resource::<LastResult>().challenge == ChallengeOutcome::NotMet,
        "no playlist, no challenge"
    );
}

/// Every tone requested over `frames` frames.
fn tones_over(h: &mut Harness, frames: usize) -> Vec<PlayTone> {
    (0..frames)
        .flat_map(|_| {
            h.app.update();
            h.tones()
        })
        .collect()
}

#[test]
fn muting_the_midi_keyboard_silences_only_your_midi_notes() {
    use keybard::app::input::MidiEcho;
    let mut h = Harness::new(song(), PlayMode::Play);
    assert_eq!(*h.world().resource::<MidiEcho>(), MidiEcho::Play);
    h.tap(KeyCode::F7);
    assert_eq!(*h.world().resource::<MidiEcho>(), MidiEcho::Mute);
    h.run_until(-0.02);

    // Your MIDI keyboard sounds by itself: no echo...
    h.midi(60, KeyEdge::Down);
    assert!(
        !h.tones().iter().any(|t| t.note == n(60)),
        "MIDI press muted"
    );
    assert!(matches!(h.status(C4), NoteStatus::Hit(_)), "still judged");
    // ...but the computer keys have no sound of their own, so they still do.
    h.key(KeyCode::KeyZ, ButtonState::Pressed);
    assert!(
        h.tones().iter().any(|t| t.note == n(48)),
        "computer key sounds"
    );

    // F7 again unmutes.
    h.tap(KeyCode::F7);
    h.midi(61, KeyEdge::Down);
    assert!(h.tones().iter().any(|t| t.note == n(61)));
}

#[test]
fn muted_midi_still_hears_the_hand_the_computer_plays() {
    use keybard::app::input::MidiEcho;
    let mut h = Harness::new(song(), PlayMode::Play);
    h.tap(KeyCode::F7);
    // Practise the left hand only: the right hand (C4 at 0s) is played for you.
    h.tap(KeyCode::F2);
    h.tap(KeyCode::F2);
    assert_eq!(h.world().resource::<Judge>().0.hands(), Hands::Left);
    assert_eq!(*h.world().resource::<MidiEcho>(), MidiEcho::Mute);
    h.run_until(-0.1);
    let tones = tones_over(&mut h, 20);
    assert!(
        tones.iter().any(|t| t.note == n(60)),
        "right hand still sounds"
    );
}

#[test]
fn the_hud_says_when_midi_notes_are_muted() {
    let mut h = Harness::new(song(), PlayMode::Play);
    assert!(!h.ui_texts().iter().any(|t| t.contains("muted")));
    h.tap(KeyCode::F7);
    h.frames(1);
    assert!(
        h.ui_texts().iter().any(|t| t.contains("MIDI notes muted")),
        "{:?}",
        h.ui_texts()
    );
}
