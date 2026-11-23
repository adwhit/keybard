//! Drive the real Bevy app headlessly: no window, GPU or audio device. Each
//! `App::update` is one frame with a fixed 1/60s clock.

use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use keybard::app::falling::FallingBar;
use keybard::app::input::{HeldKeys, InputSource, KeyEdge, NoteInput};
use keybard::app::notation::NoteHead;
use keybard::app::piano::PianoKey;
use keybard::app::transport::{PlayState, Transport};
use keybard::app::{
    self, ActiveSong, Backend, Judge, Keys, Logging, MidiInput, Options, PlayTone, palette,
};
use keybard::layout;
use keybard::music::MidiNote;
use keybard::performance::{NoteStatus, PlayMode};
use keybard::song::{Hand, NoteId, Seconds, Song, Tempo, parse_voice};

const FRAME: Duration = Duration::from_micros(16_667);

struct Harness {
    app: App,
}

impl Harness {
    fn new(song: Song, mode: PlayMode) -> Self {
        let mut app = app::build(Options {
            song,
            mode,
            midi_input: MidiInput::Disabled,
            backend: Backend::Headless,
            logging: Logging::Disabled,
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
    assert_eq!(h.status(C4), NoteStatus::Hit);
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
    assert_eq!(h.status(C4), NoteStatus::Hit);
    assert!(h.world().resource::<HeldKeys>().0.contains(&n(60)));
    assert_eq!(h.key_color(60), palette::PRESSED);

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
    assert_eq!(
        (h.status(C3), h.status(C4)),
        (NoteStatus::Hit, NoteStatus::Hit)
    );

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
    assert_eq!((score.hit, score.missed, score.wrong), (1, 3, 1));
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
