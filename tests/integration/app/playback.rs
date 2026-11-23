//! Headless app tests: playback.

use super::*;

#[test]
fn startup_spawns_keyboard_bars_and_noteheads() {
    let mut h = Harness::new(song(), PlayMode::Listen);
    let range = h.world().resource::<Keys>().0;
    let keys = h.world().query::<&PianoKey>().iter(h.app.world()).count();
    assert_eq!(keys, range.notes().count());
    let bars = h.world().query::<&FallingBar>().iter(h.app.world()).count();
    let heads = h.world().query::<&NoteHead>().iter(h.app.world()).count();
    assert_eq!((bars, heads), (4, 4));
    assert!(h.time().0 < 0.0, "starts with a lead-in");
}

#[test]
fn bars_fall_onto_the_hit_line() {
    let mut h = Harness::new(song(), PlayMode::Listen);
    h.run_until(-1.0);
    let (early, vis) = h.bar(E4);
    assert_eq!(vis, Visibility::Inherited);
    h.run_until(0.0);
    let (later, _) = h.bar(E4);
    assert!(later.y < early.y, "bars move down");
    let expected = layout::fall_y(Seconds(1.0) - h.time());
    assert!((later.y - expected).abs() < 0.01);

    let range = h.world().resource::<Keys>().0;
    let x = range.geometry(n(64)).unwrap().center_x;
    assert!((later.x - x).abs() < 0.01, "bar lines up with its key");

    h.run_until(1.05);
    let (c3, vis) = h.bar(C3);
    assert!(
        c3.y < layout::HIT_LINE,
        "started bars sink into the keyboard"
    );
    assert_eq!(vis, Visibility::Inherited, "C3 is still sounding");
    assert_eq!(h.bar(C4).1, Visibility::Hidden, "finished notes are hidden");
}

#[test]
fn noteheads_scroll_to_the_playhead_on_the_right_staff() {
    let mut h = Harness::new(song(), PlayMode::Listen);
    h.run_until(0.5);
    let (e4, vis) = h.head(E4);
    assert_eq!(vis, Visibility::Inherited);
    let expected_x = layout::staff::x(Seconds(1.0) - h.time());
    assert!((e4.x - expected_x).abs() < 0.01);
    assert!(e4.x > layout::staff::PLAYHEAD_X);
    let (c3, _) = h.head(C3);
    assert!(c3.y < e4.y, "left hand sits on the lower (bass) staff");
    assert!(c3.y < layout::staff::TREBLE_BOTTOM_LINE);
}

#[test]
fn listen_mode_plays_tones_and_lights_keys() {
    let mut h = Harness::new(song(), PlayMode::Listen);
    let played: Vec<u8> = (0..400)
        .flat_map(|_| {
            h.app.update();
            h.tones()
        })
        .map(|t| t.note.value())
        .filter(|v| *v < 88) // skip the count-in clicks
        .collect();
    assert!(played.starts_with(&[48, 60]), "got {played:?}");
    assert!(played.contains(&64) && played.contains(&67));

    let mut h = Harness::new(song(), PlayMode::Listen);
    h.run_until(0.5);
    assert_eq!(h.key_color(60), palette::hand(Hand::Right));
    assert_eq!(h.key_color(48), palette::hand(Hand::Left));
    assert_eq!(h.key_color(62), palette::WHITE_KEY);
    assert_eq!(h.status(C4), NoteStatus::AutoPlayed);
}

#[test]
fn transport_controls() {
    let mut h = Harness::new(song(), PlayMode::Listen);
    h.tap(KeyCode::Space);
    assert_eq!(h.transport().state, PlayState::Paused);
    let t = h.time();
    h.frames(20);
    assert_eq!(h.time(), t, "paused clock doesn't move");
    h.tap(KeyCode::Space);
    assert_eq!(h.transport().state, PlayState::Playing);

    h.tap(KeyCode::Tab);
    assert_eq!(h.transport().mode, PlayMode::Wait);

    let before = h.transport().speed.get();
    h.tap(KeyCode::ArrowUp);
    assert!(h.transport().speed.get() > before);

    h.tap(KeyCode::ArrowRight);
    assert!(h.time().0 > 1.0, "seeked forward to {:?}", h.time());
    assert_eq!(h.status(C4), NoteStatus::Skipped);

    h.tap(KeyCode::Backspace);
    assert!(h.time().0 < 0.0, "restart rewinds to the lead-in");
    assert_eq!(h.status(C4), NoteStatus::Pending);
}

#[test]
fn faster_speed_advances_the_clock_faster() {
    let mut slow = Harness::new(song(), PlayMode::Listen);
    let mut fast = Harness::new(song(), PlayMode::Listen);
    (0..5).for_each(|_| fast.tap(KeyCode::ArrowUp));
    slow.frames(10);
    let (s0, f0) = (slow.time(), fast.time());
    slow.frames(60);
    fast.frames(60);
    assert!((fast.time().0 - f0.0) > (slow.time().0 - s0.0) * 1.2);
}

#[test]
fn headless_app_exits_when_the_song_finishes() {
    let mut h = Harness::new(song(), PlayMode::Listen);
    let end = h.world().resource::<ActiveSong>().0.duration().0;
    h.run_until(end + 1.4);
    h.frames(30);
    assert_eq!(h.transport().state, PlayState::Finished);
    assert!(h.app.should_exit().is_some());
}

#[test]
fn f1_cycles_bar_labels_through_fingers_and_names() {
    use keybard::app::falling::{BarFinger, BarLabels};
    let mut h = Harness::new(song(), PlayMode::Listen);
    let shown = |h: &mut Harness| {
        let names = {
            let mut q = h.world().query_filtered::<&Visibility, With<BarLabel>>();
            q.iter(h.app.world())
                .filter(|v| **v != Visibility::Hidden)
                .count()
        };
        let fingers = {
            let mut q = h.world().query_filtered::<&Visibility, With<BarFinger>>();
            q.iter(h.app.world())
                .filter(|v| **v != Visibility::Hidden)
                .count()
        };
        (names, fingers)
    };
    assert_eq!(*h.world().resource::<BarLabels>(), BarLabels::Both);
    assert_eq!(shown(&mut h), (4, 4));
    h.tap(KeyCode::F1);
    assert_eq!(shown(&mut h), (0, 4), "fingers only");
    h.tap(KeyCode::F1);
    assert_eq!(shown(&mut h), (4, 0), "names only");
    h.tap(KeyCode::F1);
    assert_eq!(shown(&mut h), (0, 0), "off");
    h.tap(KeyCode::F1);
    assert_eq!(shown(&mut h), (4, 4), "back to both");
}

#[test]
fn bars_and_noteheads_show_the_recommended_finger() {
    use keybard::app::falling::BarFinger;
    use keybard::app::notation::HeadFinger;
    let mut h = Harness::new(song(), PlayMode::Listen);
    let song = h.world().resource::<ActiveSong>().0.clone();
    let expected = |id: NoteId| song.finger(id).number().to_string();
    let mut bars = h.world().query::<(&FallingBar, &Children)>();
    let mut texts = h.world().query_filtered::<&Text2d, With<BarFinger>>();
    let bar_fingers: Vec<(NoteId, String)> = bars
        .iter(h.app.world())
        .map(|(bar, children)| {
            let text = children
                .iter()
                .find_map(|c| texts.get(h.app.world(), c).ok())
                .map(|t| t.0.clone())
                .expect("bar has a finger label");
            (bar.0, text)
        })
        .collect();
    assert_eq!(bar_fingers.len(), 4);
    bar_fingers
        .iter()
        .for_each(|(id, text)| assert_eq!(*text, expected(*id)));

    let mut heads = h.world().query::<(&NoteHead, &Children)>();
    let mut head_texts = h.world().query_filtered::<&Text2d, With<HeadFinger>>();
    let head_fingers: Vec<(NoteId, String)> = heads
        .iter(h.app.world())
        .filter_map(|(head, children)| {
            children
                .iter()
                .find_map(|c| head_texts.get(h.app.world(), c).ok())
                .map(|t| (head.0, t.0.clone()))
        })
        .collect();
    assert_eq!(head_fingers.len(), 4);
    head_fingers
        .iter()
        .for_each(|(id, text)| assert_eq!(*text, expected(*id)));
}

#[test]
fn keys_show_which_finger_to_use_as_notes_approach() {
    use keybard::app::piano::{APPROACH, KeyFinger};
    let mut h = Harness::new(song(), PlayMode::Wait);
    let finger_on = |h: &mut Harness, note: u8| {
        let mut keys = h.world().query::<(&PianoKey, &Children)>();
        let mut texts = h.world().query_filtered::<&Text2d, With<KeyFinger>>();
        keys.iter(h.app.world())
            .find(|(k, _)| k.note == n(note))
            .and_then(|(_, children)| {
                children
                    .iter()
                    .find_map(|c| texts.get(h.app.world(), c).ok())
            })
            .map(|t| t.0.clone())
            .unwrap_or_default()
    };
    let song = h.world().resource::<ActiveSong>().0.clone();
    let e4 = song.finger(NoteId(E4)).number().to_string();
    h.run_until(0.0);
    assert_eq!(
        finger_on(&mut h, 60),
        song.finger(NoteId(C4)).number().to_string()
    );
    h.midi(60, KeyEdge::Down);
    h.midi(48, KeyEdge::Down);
    h.midi(60, KeyEdge::Up);
    h.midi(48, KeyEdge::Up);
    h.run_until(0.3);
    assert_eq!(finger_on(&mut h, 64), "", "too early");
    h.run_until(1.0 - APPROACH.0 / 2.0);
    assert_eq!(finger_on(&mut h, 64), e4);
}

#[test]
fn help_shows_before_the_music_and_while_paused() {
    use keybard::app::hud::HelpText;
    let mut h = Harness::new(song(), PlayMode::Listen);
    let shown = |h: &mut Harness| {
        let mut q = h.world().query_filtered::<&Visibility, With<HelpText>>();
        *q.single(h.app.world()).unwrap() != Visibility::Hidden
    };
    assert!(shown(&mut h), "lead-in");
    h.run_until(0.5);
    assert!(!shown(&mut h), "playing");
    h.tap(KeyCode::Space);
    assert!(shown(&mut h), "paused");
}

#[test]
fn snapshots_past_the_end_are_taken_when_the_song_finishes() {
    use keybard::app::SnapshotPlan;
    assert!(!SnapshotPlan::is_due(Seconds(40.0), Seconds(39.9), false));
    assert!(SnapshotPlan::is_due(Seconds(40.0), Seconds(39.9), true));
    assert!(SnapshotPlan::is_due(Seconds(5.0), Seconds(5.0), false));
}

#[test]
fn a_bar_of_count_in_clicks_and_numbers_before_the_music() {
    use keybard::app::transport::CountInTick;
    let mut h = Harness::new(song(), PlayMode::Listen);
    // 60bpm 4/4: a bar is 4s, longer than the fall, so it sets the lead-in.
    assert!(
        (h.time().0 + 4.0).abs() < 0.05,
        "starts a bar early: {:?}",
        h.time()
    );

    let mut beats = Vec::new();
    let mut clicks = Vec::new();
    while h.time().0 < 0.0 {
        h.app.update();
        let ticks = h.world().resource::<Messages<CountInTick>>();
        let new: Vec<u8> = ticks
            .iter_current_update_messages()
            .map(|t| t.0.remaining)
            .collect();
        clicks.extend(
            h.tones()
                .iter()
                .map(|t| t.note.value())
                .filter(|v| *v >= 88),
        );
        new.iter().for_each(|beat| {
            assert!(
                h.texts().contains(&beat.to_string()),
                "countdown {beat} shown"
            );
        });
        beats.extend(new);
    }
    assert_eq!(
        beats,
        vec![3, 2, 1],
        "beat 4 is the starting instant itself"
    );
    assert_eq!(clicks.len(), 3);

    h.tap(KeyCode::Backspace);
    assert!(
        (h.time().0 + 4.0).abs() < 0.05,
        "restart goes back to the count-in"
    );
}

#[test]
fn keys_hint_just_before_their_notes_arrive() {
    use keybard::app::piano::APPROACH;
    let mut h = Harness::new(song(), PlayMode::Wait);
    // E4 starts at 1.0s; C4 is gated at 0.0 so press it to let time run.
    h.run_until(0.0);
    h.midi(60, KeyEdge::Down);
    h.midi(48, KeyEdge::Down);
    h.midi(60, KeyEdge::Up);
    h.midi(48, KeyEdge::Up);
    h.run_until(1.0 - APPROACH.0 - 0.1);
    assert_eq!(h.key_color(64), palette::WHITE_KEY, "too early to hint");
    h.run_until(1.0 - APPROACH.0 / 2.0);
    assert_eq!(
        h.key_color(64),
        palette::hand_hint(Hand::Right, KeyColor::White)
    );
    assert_ne!(h.key_color(64), palette::WHITE_KEY);
}

#[test]
fn bars_brighten_as_they_approach() {
    let mut h = Harness::new(song(), PlayMode::Listen);
    let lightness = |h: &mut Harness| {
        let mut q = h.world().query::<(&FallingBar, &Sprite)>();
        let color = q
            .iter(h.app.world())
            .find(|(b, _)| b.0 == NoteId(E4))
            .map(|(_, s)| s.color.to_srgba())
            .unwrap();
        color.red + color.green + color.blue
    };
    h.run_until(0.0);
    let far = lightness(&mut h);
    h.run_until(0.95);
    let near = lightness(&mut h);
    assert!(near > far + 0.05, "{far} -> {near}");
}

#[test]
fn bars_are_shaded_by_their_dynamics() {
    // Quiet, medium and loud notes, a quarter-second apart.
    let tempo = Tempo::from_bpm(240.0, 4);
    let notes = parse_voice("!pp C4 !mf D4 !ff E4", Hand::Right, tempo).unwrap();
    let mut h = Harness::new(
        Song::new("dynamics", tempo, notes).unwrap(),
        PlayMode::Listen,
    );
    // All three are on screen but not yet close enough to start glowing.
    h.run_until(-1.0);
    let mut q = h.world().query::<(&FallingBar, &Sprite)>();
    let mut shades: Vec<(usize, f32)> = q
        .iter(h.app.world())
        .map(|(b, s)| {
            let c = s.color.to_srgba();
            (b.0.0, c.red + c.green + c.blue)
        })
        .collect();
    shades.sort_by_key(|(id, _)| *id);
    let [pp, mf, ff] = [shades[0].1, shades[1].1, shades[2].1];
    assert!(pp + 0.1 < mf && mf + 0.05 < ff, "pp {pp}, mf {mf}, ff {ff}");
}

#[test]
fn page_down_switches_to_the_next_song_and_rebuilds_everything() {
    use keybard::builtin::BuiltinSong;
    let mut h = playlist_harness(PlayMode::Play);
    h.run_until(1.0);
    h.midi(61, KeyEdge::Down); // leave some score behind
    assert!(h.score().wrong > 0);

    h.tap(KeyCode::PageDown);
    let fur_elise = BuiltinSong::FurElise.load().unwrap();
    assert_eq!(h.world().resource::<ActiveSong>().0.title, fur_elise.title);
    assert_eq!(
        h.count::<FallingBar>(),
        fur_elise.notes().len(),
        "old bars gone, new ones spawned"
    );
    assert_eq!(h.count::<NoteHead>(), fur_elise.notes().len());
    let range = h.world().resource::<Keys>().0;
    assert!(
        range.contains(n(40)) && range.contains(n(76)),
        "keyboard covers E2..E5"
    );
    assert_eq!(h.count::<PianoKey>(), range.notes().count());
    assert_eq!(h.score(), Score::default(), "fresh score");
    assert!(h.time().0 < 0.0, "starts with a lead-in");
    assert_eq!(h.transport().mode, PlayMode::Play, "mode is kept");

    // Wraps around both ways.
    h.tap(KeyCode::PageDown);
    assert!(
        h.world()
            .resource::<ActiveSong>()
            .0
            .title
            .starts_with("Twinkle")
    );
    h.tap(KeyCode::PageUp);
    assert_eq!(h.world().resource::<ActiveSong>().0.title, fur_elise.title);
}

#[test]
fn the_song_list_shows_during_the_lead_in_with_the_current_song_marked() {
    use keybard::app::songs::SongList;
    let mut h = playlist_harness(PlayMode::Play);
    let list = |h: &mut Harness| {
        let mut q = h
            .world()
            .query_filtered::<(&Text, &Visibility), With<SongList>>();
        let (text, vis) = q.single(h.app.world()).unwrap();
        (text.0.clone(), *vis != Visibility::Hidden)
    };
    let (text, shown) = list(&mut h);
    assert!(shown);
    assert!(text.contains("> Twinkle"), "{text}");
    assert!(text.contains("Fur Elise"), "{text}");
    h.run_until(0.5);
    assert!(!list(&mut h).1, "hidden while playing");
}

#[test]
fn switching_songs_mid_performance_is_safe() {
    let mut h = playlist_harness(PlayMode::Play);
    // Build up effects, ticks and held keys from the first song...
    h.run_until(0.0);
    h.midi(60, KeyEdge::Down);
    h.run_until(2.0);
    assert!(h.count::<ProgressTick>() > 0);
    // ...then switch and keep going for a while.
    h.tap(KeyCode::PageDown);
    assert_eq!(h.count::<ProgressTick>(), 0, "old ticks cleared");
    assert_eq!(h.count::<ResultsCard>(), 0);
    h.run_until(3.0);
    h.tap(KeyCode::PageDown);
    h.run_until(1.0);
}

#[test]
fn bars_mark_where_the_hand_shifts_position() {
    use keybard::app::falling::ShiftMarker;
    let tempo = Tempo::from_bpm(120.0, 4);
    let notes = parse_voice("C4 D4 E4 F4 G4 A4 B4 C5", Hand::Right, tempo).unwrap();
    let song = Song::new("scale", tempo, notes).unwrap();
    let shifted = song
        .iter()
        .filter(|(id, _)| song.starts_new_position(*id))
        .count();
    assert_eq!(shifted, 1);
    let mut h = Harness::new(song, PlayMode::Listen);
    assert_eq!(h.count::<ShiftMarker>(), 1);
    let mut q = h.world().query::<(&FallingBar, &Children)>();
    let mut markers = h.world().query::<&ShiftMarker>();
    let marked: Vec<NoteId> = q
        .iter(h.app.world())
        .filter(|(_, children)| {
            children
                .iter()
                .any(|c| markers.get(h.app.world(), c).is_ok())
        })
        .map(|(bar, _)| bar.0)
        .collect();
    assert_eq!(
        marked,
        vec![NoteId(3)],
        "the F, where the thumb passes under"
    );
}

#[test]
fn rests_are_found_for_the_loaded_song_and_rebuilt_on_switch() {
    use keybard::app::notation::SongRests;
    use keybard::notation::rests;
    let mut h = playlist_harness(PlayMode::Listen);
    let expected = |song: &Song| rests(song, Hand::Right).len() + rests(song, Hand::Left).len();
    let twinkle = h.world().resource::<ActiveSong>().0.clone();
    assert_eq!(
        h.world().resource::<SongRests>().0.len(),
        expected(&twinkle)
    );
    h.tap(KeyCode::PageDown);
    let fur_elise = h.world().resource::<ActiveSong>().0.clone();
    assert!(expected(&fur_elise) > 0);
    assert_eq!(
        h.world().resource::<SongRests>().0.len(),
        expected(&fur_elise)
    );
}

#[test]
fn notes_sit_where_the_key_spells_them() {
    use keybard::music::{Clef, StaffPosition};
    let tempo = Tempo::from_bpm(60.0, 4);
    let notes = parse_voice("F4 G4 A4 A#4 C5 D5 E5 F5", Hand::Right, tempo).unwrap();
    let song = Song::new("f major", tempo, notes).unwrap();
    assert_eq!(song.key().accidentals(), -1);
    let mut h = Harness::new(song, PlayMode::Listen);
    h.run_until(2.5);
    let (bb, _) = h.head(3);
    let middle = layout::staff::y(Clef::Treble, StaffPosition::MIDDLE_LINE);
    assert!(
        (bb.y - middle).abs() < 0.01,
        "Bb on the B line: {} vs {middle}",
        bb.y
    );
}

#[test]
fn beam_groups_are_built_for_the_song() {
    use keybard::app::notation::SongBeams;
    use keybard::music::StemDirection;
    let tempo = Tempo::from_bpm(60.0, 4);
    let notes = parse_voice("C4/0.5 D4/0.5 E5/0.5 F5/0.5 G4/2", Hand::Right, tempo).unwrap();
    let song = Song::new("beams", tempo, notes).unwrap();
    let h = Harness::new(song, PlayMode::Listen);
    let beams = h.app.world().resource::<SongBeams>();
    assert_eq!(beams.groups.len(), 2);
    assert_eq!(
        beams.groups[0].direction,
        StemDirection::Up,
        "low notes: stems up"
    );
    assert_eq!(
        beams.groups[1].direction,
        StemDirection::Down,
        "high notes: stems down"
    );
}

#[test]
fn f4_toggles_a_metronome_that_clicks_every_beat() {
    use keybard::app::transport::Metronome;
    let clicks_between = |h: &mut Harness, until: f32| {
        let mut clicks = 0;
        while h.time().0 < until {
            h.app.update();
            clicks += h.tones().iter().filter(|t| t.note.value() >= 88).count();
        }
        clicks
    };
    let mut h = Harness::new(song(), PlayMode::Listen);
    h.run_until(0.1);
    assert_eq!(clicks_between(&mut h, 2.05), 0, "off by default");
    h.tap(KeyCode::F4);
    assert_eq!(*h.world().resource::<Metronome>(), Metronome::On);
    // 60bpm: beats at 3.0 and 4.0... from 2.05 to 4.05 crosses two.
    assert_eq!(clicks_between(&mut h, 4.05), 2);
    h.tap(KeyCode::F4);
    assert_eq!(*h.world().resource::<Metronome>(), Metronome::Off);
}

#[test]
fn chord_symbols_scroll_above_the_staff() {
    use keybard::app::notation::ChordSymbol;
    use keybard::chords::progression;
    let song = keybard::builtin::BuiltinSong::JingleBells.load().unwrap();
    let expected = progression(&song);
    let mut h = Harness::new(song, PlayMode::Listen);
    assert_eq!(h.count::<ChordSymbol>(), expected.len());
    h.run_until(0.0);
    let mut q = h
        .world()
        .query::<(&ChordSymbol, &Text2d, &Transform, &Visibility)>();
    let first = q
        .iter(h.app.world())
        .find(|(c, ..)| c.at == expected[0].0)
        .map(|(_, t, tr, v)| (t.0.clone(), tr.translation, *v))
        .unwrap();
    assert_eq!(first.0, "C");
    assert_eq!(first.2, Visibility::Inherited);
    assert!(
        first.1.y
            > layout::staff::y(
                keybard::music::Clef::Treble,
                keybard::music::StaffPosition::TOP_LINE
            )
    );
}

#[test]
fn sight_reading_hides_the_bars_and_the_keyboard_cues() {
    use keybard::app::falling::View;
    let mut h = Harness::new(song(), PlayMode::Wait);
    h.run_until(0.0);
    assert_eq!(h.bar(C4).1, Visibility::Inherited);
    assert_eq!(h.key_color(60), palette::hand(Hand::Right), "cue before");

    h.tap(KeyCode::F6);
    assert_eq!(*h.world().resource::<View>(), View::SightReading);
    h.frames(2);
    assert_eq!(h.bar(C4).1, Visibility::Hidden, "no bars to follow");
    assert_eq!(h.key_color(60), palette::WHITE_KEY, "no cue on the key");

    // Your own presses still show.
    h.midi(60, KeyEdge::Down);
    assert_eq!(h.key_color(60), palette::hand_lit(Hand::Right));

    h.tap(KeyCode::F6);
    assert_eq!(*h.world().resource::<View>(), View::Bars);
}

#[test]
fn dynamics_are_marked_and_heard() {
    use keybard::app::notation::DynamicMark;
    use keybard::dynamics::{Velocity, markings};
    use keybard::song::Note;
    let tempo = Tempo::from_bpm(60.0, 4);
    let note = |pitch: u8, start: f32, velocity: u8| Note {
        pitch: n(pitch),
        start: Seconds(start),
        duration: Seconds(4.0),
        hand: Hand::Right,
        finger: None,
        velocity: Velocity::new(velocity),
    };
    let song = Song::new("dyn", tempo, vec![note(60, 0.0, 40), note(64, 4.0, 110)]).unwrap();
    let expected = markings(&song).len();
    let mut h = Harness::new(song, PlayMode::Listen);
    assert_eq!(expected, 2);
    assert_eq!(h.count::<DynamicMark>(), 2);
    let texts = h.texts();
    assert!(
        texts.contains(&"p".to_owned()) && texts.contains(&"f".to_owned()),
        "{texts:?}"
    );

    // The computer plays each note at its own velocity.
    let mut heard = Vec::new();
    while h.time().0 < 4.5 {
        h.app.update();
        heard.extend(
            h.tones()
                .iter()
                .filter(|t| t.note.value() < 88)
                .map(|t| t.velocity.value()),
        );
    }
    assert_eq!(heard, vec![40, 110]);
}
