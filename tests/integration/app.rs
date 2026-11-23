//! Drive the real Bevy app headlessly: no window, GPU or audio device. Each
//! `App::update` is one frame with a fixed 1/60s clock.

use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use keybard::app::effects::{KeyFlash, Particle, Popup};
use keybard::app::falling::BarLabel;
use keybard::app::falling::FallingBar;
use keybard::app::hud::ProgressTick;
use keybard::app::input::{HeldKeys, InputSource, KeyEdge, NoteInput};
use keybard::app::notation::NoteHead;
use keybard::app::piano::PianoKey;
use keybard::app::results::{LastResult, ResultsCard};
use keybard::app::transport::{PlayState, Transport};
use keybard::app::{
    self, ActiveSong, Backend, Judge, Keys, Logging, MidiInput, Options, PlayTone, Player,
    RecordStore, palette,
};
use keybard::layout;
use keybard::music::{KeyColor, MidiNote};
use keybard::performance::{Hands, NoteStatus, PlayMode, Score, Timing};
use keybard::records::{RecordKey, Records, Submission};
use keybard::song::{Hand, NoteId, Seconds, Song, Tempo, parse_voice};

const FRAME: Duration = Duration::from_micros(16_667);

struct Harness {
    app: App,
}

impl Harness {
    fn new(song: Song, mode: PlayMode) -> Self {
        Self::with(song, mode, Player::Human, RecordStore::Memory)
    }

    fn with(song: Song, mode: PlayMode, player: Player, records: RecordStore) -> Self {
        Self::build(Options {
            song,
            mode,
            midi_input: MidiInput::Disabled,
            backend: Backend::Headless,
            logging: Logging::Disabled,
            player,
            records,
            hands: Hands::Both,
            loop_from: None,
            playlist: Default::default(),
            practice_log: RecordStore::Memory,
        })
    }

    fn build(options: Options) -> Self {
        let mut app = app::build(options);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(FRAME));
        app.finish();
        app.cleanup();
        app.update();
        Self { app }
    }

    fn world(&mut self) -> &mut World {
        self.app.world_mut()
    }

    fn frames(&mut self, n: usize) {
        (0..n).for_each(|_| self.app.update());
    }

    fn time(&self) -> Seconds {
        self.app.world().resource::<Transport>().time
    }

    fn transport(&self) -> &Transport {
        self.app.world().resource::<Transport>()
    }

    /// Step until the song clock reaches `t` (panics if it never does).
    fn run_until(&mut self, t: f32) {
        let limit = 60 * 120;
        let reached = (0..limit).any(|_| {
            self.app.update();
            self.time().0 >= t
        });
        assert!(reached, "clock stuck at {:?}", self.time());
    }

    fn key(&mut self, code: KeyCode, state: ButtonState) {
        self.world().write_message(KeyboardInput {
            key_code: code,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        self.app.update();
    }

    fn tap(&mut self, code: KeyCode) {
        self.key(code, ButtonState::Pressed);
        self.key(code, ButtonState::Released);
    }

    fn midi(&mut self, note: u8, edge: KeyEdge) {
        self.world().write_message(NoteInput {
            note: n(note),
            edge,
            source: InputSource::Midi,
        });
        self.app.update();
    }

    fn tones(&mut self) -> Vec<PlayTone> {
        let messages = self.world().resource::<Messages<PlayTone>>();
        messages.iter_current_update_messages().copied().collect()
    }

    fn count<C: Component>(&mut self) -> usize {
        self.world().query::<&C>().iter(self.app.world()).count()
    }

    fn texts(&mut self) -> Vec<String> {
        let mut q = self.world().query::<&Text2d>();
        q.iter(self.app.world()).map(|t| t.0.clone()).collect()
    }

    fn score(&self) -> Score {
        self.app.world().resource::<Judge>().0.score().clone()
    }

    fn status(&self, id: usize) -> NoteStatus {
        self.app.world().resource::<Judge>().0.status(NoteId(id))
    }

    fn key_color(&mut self, note: u8) -> Color {
        let mut q = self.world().query::<(&PianoKey, &Sprite)>();
        q.iter(self.app.world())
            .find(|(k, _)| k.note == n(note))
            .map(|(_, s)| s.color)
            .expect("key exists")
    }

    fn bar(&mut self, id: usize) -> (Vec3, Visibility) {
        let mut q = self
            .world()
            .query::<(&FallingBar, &Transform, &Visibility)>();
        q.iter(self.app.world())
            .find(|(b, ..)| b.0 == NoteId(id))
            .map(|(_, t, v)| (t.translation, *v))
            .expect("bar exists")
    }

    fn head(&mut self, id: usize) -> (Vec3, Visibility) {
        let mut q = self.world().query::<(&NoteHead, &Transform, &Visibility)>();
        q.iter(self.app.world())
            .find(|(h, ..)| h.0 == NoteId(id))
            .map(|(_, t, v)| (t.translation, *v))
            .expect("head exists")
    }
}

fn n(v: u8) -> MidiNote {
    MidiNote::new(v).expect("valid note")
}

/// Right hand C4 E4 G4 one second apart; left hand C3 for two seconds.
fn song() -> Song {
    let tempo = Tempo::from_bpm(60.0, 4);
    let notes = parse_voice("C4 E4 G4", Hand::Right, tempo)
        .unwrap()
        .into_iter()
        .chain(parse_voice("C3/2", Hand::Left, tempo).unwrap())
        .collect();
    Song::new("test", tempo, notes).unwrap()
}

/// Note ids after sorting: C3(0) C4(1) E4(2) G4(3).
const C3: usize = 0;
const C4: usize = 1;
const E4: usize = 2;
const G4: usize = 3;

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
    let mut h = Harness::with(song, PlayMode::Play, Player::Autopilot, RecordStore::Memory);
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
fn personal_bests_are_saved_to_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bests.tsv");
    let mut h = Harness::with(
        song(),
        PlayMode::Wait,
        Player::Autopilot,
        RecordStore::File(path.clone()),
    );
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
    let mut h = Harness::with(
        song(),
        PlayMode::Listen,
        Player::Autopilot,
        RecordStore::Memory,
    );
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
fn a_hot_streak_sets_the_screen_on_fire_and_breaking_it_cools_down() {
    use keybard::app::heat::{Ember, HeatGlow};
    use keybard::performance::Heat;
    let tempo = Tempo::from_bpm(240.0, 4);
    let notes = parse_voice(&"C4 E4 G4 E4 ".repeat(10), Hand::Right, tempo).unwrap();
    let song = Song::new("streak", tempo, notes).unwrap();
    let mut h = Harness::with(song, PlayMode::Play, Player::Autopilot, RecordStore::Memory);

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
    let mut h = Harness::with(song, PlayMode::Play, Player::Human, RecordStore::Memory);
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

fn playlist_harness(mode: PlayMode) -> Harness {
    use keybard::app::songs::{Playlist, SongSource};
    use keybard::builtin::BuiltinSong;
    let playlist = Playlist::new(
        vec![
            SongSource::Builtin(BuiltinSong::Twinkle),
            SongSource::Builtin(BuiltinSong::FurElise),
        ],
        0,
    );
    Harness::build(Options {
        song: BuiltinSong::Twinkle.load().unwrap(),
        mode,
        midi_input: MidiInput::Disabled,
        backend: Backend::Headless,
        logging: Logging::Disabled,
        player: Player::Human,
        records: RecordStore::Memory,
        hands: Hands::Both,
        loop_from: None,
        playlist,
        practice_log: RecordStore::Memory,
    })
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
