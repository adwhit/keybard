//! Note input from the computer keyboard and from MIDI devices.

use std::collections::HashSet;
use std::thread;
use std::time::Duration;

use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender};
use tracing::{debug, info, warn};

use super::audio::PlayTone;
use super::transport::{PlayState, Transport};
use super::{ActiveSong, Feedback, Judge, MidiInput, Phase};
use crate::dynamics::{self, Touch, Velocity};
use crate::midi_port::{PortAction, PortChoice, next_action};
use crate::music::MidiNote;
use crate::performance::PressOutcome;
use crate::song::Seconds;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEdge {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSource {
    ComputerKeyboard,
    Midi,
    Autopilot,
}

/// A piano key went down or up.
#[derive(Message, Debug, Clone, Copy)]
pub struct NoteInput {
    pub note: MidiNote,
    pub edge: KeyEdge,
    pub source: InputSource,
    pub velocity: Velocity,
}

/// Keys the player is currently holding.
#[derive(Resource, Default)]
pub struct HeldKeys(pub HashSet<MidiNote>);

/// Note that the bottom row of the computer keyboard maps to (`Z` key).
#[derive(Resource)]
struct ComputerKeyboardBase(MidiNote);

#[derive(Resource)]
struct MidiReceiver(Receiver<(MidiNote, KeyEdge, Velocity)>);

/// Two-row "tracker" layout: Z-row is one octave, Q-row the next.
const COMPUTER_KEYS: [(KeyCode, i16); 32] = [
    (KeyCode::KeyZ, 0),
    (KeyCode::KeyS, 1),
    (KeyCode::KeyX, 2),
    (KeyCode::KeyD, 3),
    (KeyCode::KeyC, 4),
    (KeyCode::KeyV, 5),
    (KeyCode::KeyG, 6),
    (KeyCode::KeyB, 7),
    (KeyCode::KeyH, 8),
    (KeyCode::KeyN, 9),
    (KeyCode::KeyJ, 10),
    (KeyCode::KeyM, 11),
    (KeyCode::Comma, 12),
    (KeyCode::KeyL, 13),
    (KeyCode::Period, 14),
    (KeyCode::Semicolon, 15),
    (KeyCode::KeyQ, 12),
    (KeyCode::Digit2, 13),
    (KeyCode::KeyW, 14),
    (KeyCode::Digit3, 15),
    (KeyCode::KeyE, 16),
    (KeyCode::KeyR, 17),
    (KeyCode::Digit5, 18),
    (KeyCode::KeyT, 19),
    (KeyCode::Digit6, 20),
    (KeyCode::KeyY, 21),
    (KeyCode::Digit7, 22),
    (KeyCode::KeyU, 23),
    (KeyCode::KeyI, 24),
    (KeyCode::Digit9, 25),
    (KeyCode::KeyO, 26),
    (KeyCode::KeyP, 28),
];

/// Fixed length for tones triggered by key presses (we can't know how long
/// the key will be held when the sound starts).
const PRESS_TONE: Seconds = Seconds(0.6);

pub fn plugin(midi_input: MidiInput) -> impl Fn(&mut App) {
    move |app: &mut App| {
        app.add_message::<NoteInput>()
            .init_resource::<HeldKeys>()
            .insert_resource(ComputerKeyboardBase(
                MidiNote::new(48).unwrap_or(MidiNote::MIDDLE_C),
            ))
            .add_systems(
                Update,
                (computer_keyboard, shift_octave).in_set(Phase::Input),
            )
            .add_systems(
                Update,
                judge_presses
                    .in_set(Phase::Simulate)
                    .before(super::transport::advance),
            );
        match &midi_input {
            MidiInput::Enabled(choice) => {
                let (tx, rx) = crossbeam_channel::unbounded();
                let (status_tx, status_rx) = crossbeam_channel::unbounded();
                spawn_midi_listener(tx, status_tx, choice.clone());
                app.insert_resource(MidiReceiver(rx))
                    .insert_resource(MidiStatusReceiver(status_rx))
                    .insert_resource(MidiStatus::Connecting)
                    .add_systems(Update, (midi_keyboard, midi_status).in_set(Phase::Input));
            }
            MidiInput::Disabled => {
                app.insert_resource(MidiStatus::Off);
            }
        }
    }
}

fn shift_octave(keys: Res<ButtonInput<KeyCode>>, mut base: ResMut<ComputerKeyboardBase>) {
    let shift = [(KeyCode::Minus, -12), (KeyCode::Equal, 12)]
        .into_iter()
        .filter(|(k, _)| keys.just_pressed(*k))
        .map(|(_, d)| d)
        .sum::<i16>();
    if shift != 0
        && let Some(note) = base.0.offset(shift).filter(|n| n.offset(28).is_some())
    {
        base.0 = note;
        info!(base = %note, "computer keyboard octave shifted");
    }
}

fn computer_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    base: Res<ComputerKeyboardBase>,
    mut out: MessageWriter<NoteInput>,
) {
    let events = COMPUTER_KEYS.iter().filter_map(|&(code, offset)| {
        let edge = if keys.just_pressed(code) {
            KeyEdge::Down
        } else if keys.just_released(code) {
            KeyEdge::Up
        } else {
            return None;
        };
        base.0.offset(offset).map(|note| NoteInput {
            note,
            edge,
            source: InputSource::ComputerKeyboard,
            velocity: Velocity::DEFAULT,
        })
    });
    out.write_batch(events);
}

/// What the MIDI listener is up to, for the HUD.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub enum MidiStatus {
    Off,
    Connecting,
    Connected(String),
    NoDevices,
    NoMatch(String),
    Unplugged(String),
    Failed(String),
}

impl MidiStatus {
    pub fn label(&self) -> String {
        match self {
            Self::Off => "MIDI off".to_owned(),
            Self::Connecting => "MIDI: looking for a keyboard".to_owned(),
            Self::Connected(name) => format!("MIDI: {name}"),
            Self::NoDevices => "MIDI: no keyboard yet (plug one in any time)".to_owned(),
            Self::NoMatch(wanted) => format!("MIDI: no port matching \"{wanted}\""),
            Self::Unplugged(name) => format!("MIDI: {name} unplugged, waiting for it"),
            Self::Failed(err) => format!("MIDI error: {err}"),
        }
    }
}

#[derive(Resource)]
struct MidiStatusReceiver(Receiver<MidiStatus>);

fn midi_status(rx: Res<MidiStatusReceiver>, mut status: ResMut<MidiStatus>) {
    if let Some(latest) = rx.0.try_iter().last() {
        *status = latest;
    }
}

fn midi_keyboard(rx: Res<MidiReceiver>, mut out: MessageWriter<NoteInput>) {
    out.write_batch(rx.0.try_iter().map(|(note, edge, velocity)| NoteInput {
        note,
        edge,
        source: InputSource::Midi,
        velocity,
    }));
}

fn judge_presses(
    mut inputs: MessageReader<NoteInput>,
    mut held: ResMut<HeldKeys>,
    mut judge: ResMut<Judge>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    mut tones: MessageWriter<PlayTone>,
    mut feedback: MessageWriter<Feedback>,
) {
    inputs.read().for_each(|input| match input.edge {
        KeyEdge::Down => {
            held.0.insert(input.note);
            tones.write(PlayTone {
                note: input.note,
                duration: PRESS_TONE,
                velocity: input.velocity,
            });
            let judging = transport.mode.is_scored() && transport.state == PlayState::Playing;
            if judging {
                let outcome = judge.0.press(&song.0, input.note, transport.time);
                debug!(note = %input.note, source = ?input.source, ?outcome, "judged press");
                // Only a MIDI keyboard knows how hard the key went down.
                if let (PressOutcome::Hit(judgement), InputSource::Midi) = (outcome, input.source) {
                    judge.0.touch(judgement.note, input.velocity);
                    let marked = song.0.note(judgement.note).velocity;
                    match dynamics::compare(input.velocity, marked) {
                        Touch::AsMarked => {}
                        touch => {
                            feedback.write(Feedback::Touch {
                                note: judgement.note,
                                touch,
                            });
                        }
                    }
                }
                feedback.write(match outcome {
                    PressOutcome::Hit(judgement) => Feedback::Hit(judgement),
                    PressOutcome::Wrong => Feedback::Wrong {
                        pressed: input.note,
                        expected: judge
                            .0
                            .expected_near(&song.0, input.note, transport.time)
                            .map(|id| song.0.note(id).pitch),
                    },
                });
            }
        }
        KeyEdge::Up => {
            held.0.remove(&input.note);
        }
    });
}

fn parse_midi_message(bytes: &[u8]) -> Option<(MidiNote, KeyEdge, Velocity)> {
    match *bytes {
        [status, key, velocity, ..] => {
            let note = MidiNote::new(key).ok()?;
            let velocity_of = Velocity::new(velocity);
            match status & 0xF0 {
                0x90 if velocity > 0 => Some((note, KeyEdge::Down, velocity_of)),
                0x90 | 0x80 => Some((note, KeyEdge::Up, velocity_of)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// How often the watcher re-checks the port list for keyboards coming and
/// going.
const PORT_POLL: Duration = Duration::from_secs(1);

/// Watch MIDI input ports on a background thread: connect to the chosen
/// keyboard whenever it's plugged in, and let go when it's unplugged. The
/// thread owns the connection for the lifetime of the app.
fn spawn_midi_listener(
    tx: Sender<(MidiNote, KeyEdge, Velocity)>,
    status: Sender<MidiStatus>,
    choice: PortChoice,
) {
    let spawned = thread::Builder::new()
        .name("midi-input".into())
        .spawn(move || {
            let mut connection: Option<(String, midir::MidiInputConnection<()>)> = None;
            let mut reported: Option<MidiStatus> = None;
            // The app may already be gone; status updates are best-effort.
            let mut report = |s: MidiStatus| {
                if reported.as_ref() != Some(&s) {
                    info!(status = s.label(), "MIDI");
                    let _ = status.send(s.clone());
                    reported = Some(s);
                }
            };
            loop {
                let input = match midir::MidiInput::new("keybard") {
                    Ok(input) => input,
                    Err(err) => {
                        warn!(%err, "MIDI unavailable");
                        report(MidiStatus::Failed(err.to_string()));
                        return;
                    }
                };
                let ports = input.ports();
                let names: Vec<String> = ports
                    .iter()
                    .map(|p| input.port_name(p).unwrap_or_else(|_| "unknown".into()))
                    .collect();
                let current = connection.as_ref().map(|(name, _)| name.as_str());
                match next_action(current, &names, &choice) {
                    PortAction::Stay => {}
                    PortAction::Wait => report(match &choice {
                        PortChoice::First => MidiStatus::NoDevices,
                        PortChoice::Named(wanted) => MidiStatus::NoMatch(wanted.clone()),
                    }),
                    PortAction::Disconnect => {
                        if let Some((name, open)) = connection.take() {
                            open.close();
                            report(MidiStatus::Unplugged(name));
                        }
                    }
                    PortAction::Connect(index) => {
                        let name = names[index].clone();
                        let tx = tx.clone();
                        let opened = input.connect(
                            &ports[index],
                            "keybard-input",
                            move |_stamp, bytes, _| {
                                if let Some(event) = parse_midi_message(bytes) {
                                    // The receiver only disappears when the app exits.
                                    let _ = tx.send(event);
                                }
                            },
                            (),
                        );
                        match opened {
                            Ok(open) => {
                                report(MidiStatus::Connected(name.clone()));
                                connection = Some((name, open));
                            }
                            Err(err) => {
                                warn!(port = %name, %err, "could not open MIDI input");
                                report(MidiStatus::Failed(err.to_string()));
                            }
                        }
                    }
                }
                thread::sleep(PORT_POLL);
            }
        });
    if let Err(err) = spawned {
        warn!(%err, "could not start MIDI thread");
    }
}
