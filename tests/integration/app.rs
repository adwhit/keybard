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
            view: Default::default(),
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
        self.midi_at(note, edge, keybard::dynamics::Velocity::DEFAULT);
    }

    fn midi_at(&mut self, note: u8, edge: KeyEdge, velocity: keybard::dynamics::Velocity) {
        self.world().write_message(NoteInput {
            note: n(note),
            edge,
            source: InputSource::Midi,
            velocity,
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
        view: Default::default(),
    })
}

mod playback;
mod practice;
mod scoring;
