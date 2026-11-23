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
use keybard::music::MidiNote;
use keybard::performance::{NoteStatus, PlayMode, Score, Timing};
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
        let mut app = app::build(Options {
            song,
            mode,
            midi_input: MidiInput::Disabled,
            backend: Backend::Headless,
            logging: Logging::Disabled,
            player,
            records,
        });
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
    assert!(h.time().0 > 2.0, "seeked forward to {:?}", h.time());
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
fn note_name_labels_toggle() {
    let mut h = Harness::new(song(), PlayMode::Listen);
    let visible = |h: &mut Harness| {
        let mut q = h.world().query_filtered::<&Visibility, With<BarLabel>>();
        q.iter(h.app.world())
            .filter(|v| **v != Visibility::Hidden)
            .count()
    };
    assert_eq!(visible(&mut h), 4);
    h.tap(KeyCode::F1);
    assert_eq!(visible(&mut h), 0);
    h.tap(KeyCode::F1);
    assert_eq!(visible(&mut h), 4);
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
