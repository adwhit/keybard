//! The song clock: play/pause, seeking, speed and play-mode controls.

use bevy::prelude::*;
use tracing::{debug, info};

use super::audio::PlayTone;
use super::controls::Control;
use super::{ActiveSong, Feedback, Judge, Phase};
use crate::dynamics::Velocity;
use crate::layout;
use crate::music::MidiNote;
use crate::performance::PlayMode;
use crate::song::{CountIn, Seconds, Tempo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Playing,
    Paused,
    Finished,
}

/// Playback speed multiplier, kept within sensible bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Speed(f32);

impl Speed {
    const MIN: f32 = 0.25;
    const MAX: f32 = 2.0;
    const STEP: f32 = 0.05;

    pub fn new(value: f32) -> Self {
        Self(value.clamp(Self::MIN, Self::MAX))
    }

    pub fn get(self) -> f32 {
        self.0
    }

    fn nudge(self, steps: f32) -> Self {
        Self((self.0 + steps * Self::STEP).clamp(Self::MIN, Self::MAX))
    }
}

#[derive(Resource, Debug)]
pub struct Transport {
    pub time: Seconds,
    pub state: PlayState,
    pub speed: Speed,
    pub mode: PlayMode,
    /// Where the clock starts: long enough for the first notes to fall into
    /// view and for a full bar of count-in.
    pub start: Seconds,
}

/// Whether a click plays on every beat of the music (F4).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Metronome {
    #[default]
    Off,
    On,
}

/// A beat of the count-in, for display and the metronome click.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct CountInTick(pub CountIn);

const SEEK_STEP: Seconds = Seconds(5.0);
/// Linger this long after the last note before declaring the song finished.
const OUTRO: Seconds = Seconds(1.5);

impl Transport {
    pub fn new(mode: PlayMode, tempo: Tempo) -> Self {
        let lead_in = layout::fall_lookahead().0.max(tempo.bar_length().0);
        let start = Seconds(-lead_in);
        Self {
            time: start,
            state: PlayState::Playing,
            speed: Speed(1.0),
            mode,
            start,
        }
    }
}

/// Metronome pitches: accented downbeat, then the other beats.
const CLICK_ACCENT: u8 = 93; // A6
const CLICK: u8 = 88; // E6
const CLICK_LENGTH: Seconds = Seconds(0.05);

pub fn plugin(app: &mut App) {
    app.add_message::<CountInTick>()
        .init_resource::<Metronome>()
        .add_systems(Update, (controls, advance).chain().in_set(Phase::Simulate));
}

pub fn controls(
    mut controls: MessageReader<Control>,
    mut transport: ResMut<Transport>,
    song: Res<ActiveSong>,
    mut judge: ResMut<Judge>,
    mut metronome: ResMut<Metronome>,
) {
    let song = &song.0;
    controls.read().for_each(|control| match *control {
        Control::TogglePlay => {
            transport.state = match transport.state {
                PlayState::Playing => PlayState::Paused,
                PlayState::Paused => PlayState::Playing,
                PlayState::Finished => {
                    judge.0.reset(song);
                    transport.time = transport.start;
                    PlayState::Playing
                }
            };
            info!(state = ?transport.state, "toggled playback");
        }
        Control::Restart => {
            judge.0.reset(song);
            transport.time = transport.start;
            transport.state = PlayState::Playing;
            info!("restarted song");
        }
        Control::SeekBack | Control::SeekForward => {
            let step = if *control == Control::SeekBack {
                -SEEK_STEP.0
            } else {
                SEEK_STEP.0
            };
            let target = (transport.time.0 + step).clamp(transport.start.0, song.duration().0);
            transport.time = Seconds(target);
            judge.0.seek(song, transport.time);
            if transport.state == PlayState::Finished {
                transport.state = PlayState::Paused;
            }
            info!(time = target, "seeked");
        }
        Control::Slower | Control::Faster => {
            let steps = if *control == Control::Slower {
                -1.0
            } else {
                1.0
            };
            transport.speed = transport.speed.nudge(steps);
            info!(speed = transport.speed.get(), "changed speed");
        }
        Control::CycleMode | Control::SetMode(_) => {
            transport.mode = match *control {
                Control::SetMode(mode) => mode,
                _ => transport.mode.next(),
            };
            judge.0.seek(song, transport.time);
            // Picking a mode once the song is over starts it again in that
            // mode (e.g. "Play it" after listening).
            if transport.state == PlayState::Finished {
                judge.0.reset(song);
                transport.time = transport.start;
                transport.state = PlayState::Playing;
            }
            info!(mode = transport.mode.label(), "changed play mode");
        }
        Control::ToggleMetronome => {
            *metronome = match *metronome {
                Metronome::Off => Metronome::On,
                Metronome::On => Metronome::Off,
            };
            info!(metronome = ?*metronome, "toggled metronome");
        }
        Control::CycleHands | Control::SetHands(_) => {
            let hands = match *control {
                Control::SetHands(hands) => hands,
                _ => judge.0.hands().next(),
            };
            judge.0.set_hands(hands);
            judge.0.seek(song, transport.time);
            info!(hands = hands.label(), "changed practised hands");
        }
        _ => {}
    });
}

#[expect(clippy::too_many_arguments, reason = "Bevy system parameters")]
pub fn advance(
    time: Res<Time>,
    mut transport: ResMut<Transport>,
    song: Res<ActiveSong>,
    mut judge: ResMut<Judge>,
    mut tones: MessageWriter<PlayTone>,
    mut feedback: MessageWriter<Feedback>,
    mut ticks: MessageWriter<CountInTick>,
    metronome: Res<Metronome>,
) {
    if transport.state != PlayState::Playing {
        return;
    }
    let song = &song.0;
    let from = transport.time;
    let to = Seconds(from.0 + time.delta_secs() * transport.speed.get());
    let advance = judge.0.advance(song, transport.mode, from, to);
    song.tempo.count_in(from, advance.time).for_each(|beat| {
        let accent = beat.remaining == song.tempo.beats_per_bar;
        tones.write(PlayTone {
            note: MidiNote::new(if accent { CLICK_ACCENT } else { CLICK })
                .unwrap_or(MidiNote::MIDDLE_C),
            duration: CLICK_LENGTH,
            velocity: Velocity::DEFAULT,
        });
        ticks.write(CountInTick(beat));
    });
    if *metronome == Metronome::On {
        tones.write_batch(song.tempo.beats_between(from, advance.time).map(|beat| {
            PlayTone {
                note: MidiNote::new(if beat.downbeat { CLICK_ACCENT } else { CLICK })
                    .unwrap_or(MidiNote::MIDDLE_C),
                duration: CLICK_LENGTH,
                velocity: Velocity::DEFAULT,
            }
        }));
    }
    transport.time = advance.time;
    let speed = transport.speed.get();
    tones.write_batch(advance.auto_played.iter().map(|id| {
        let note = song.note(*id);
        PlayTone {
            note: note.pitch,
            duration: Seconds(note.duration.0 / speed),
            velocity: note.velocity,
        }
    }));
    if !advance.auto_played.is_empty() {
        debug!(count = advance.auto_played.len(), "auto-played notes");
    }
    feedback.write_batch(
        advance
            .auto_played
            .iter()
            .map(|id| Feedback::AutoPlayed(*id))
            .chain(advance.missed.iter().map(|id| Feedback::Missed(*id))),
    );
    if transport.time > song.duration() + OUTRO {
        transport.state = PlayState::Finished;
        info!(score = ?judge.0.score(), "song finished");
    }
}
