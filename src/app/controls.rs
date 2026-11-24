//! Every user command as one message, whatever it came from: a key
//! shortcut, an on-screen button or the menu bar.
//!
//! Systems react to [`Control`] rather than to raw keys, so a mouse click and
//! its shortcut always do the same thing.

use std::collections::HashSet;

use bevy::picking::events::PointerClick;
use bevy::prelude::*;
use tracing::info;

use super::input::{KeyEdge, NoteInput};
use crate::chord_keys::ChordKey;
use crate::music::{MidiNote, PitchClass};
use crate::performance::{Hands, PlayMode};

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    TogglePlay,
    Restart,
    SeekBack,
    SeekForward,
    Slower,
    Faster,
    CycleMode,
    SetMode(PlayMode),
    CycleHands,
    SetHands(Hands),
    ToggleLoop,
    ToggleMetronome,
    ToggleAchievements,
    ToggleSightReading,
    CycleLabels,
    PreviousSong,
    NextSong,
    SelectSong(usize),
    /// Leave the start screen and play the selected song.
    StartSong,
    /// Back to the start screen.
    OpenMenu,
    /// Escape: from a song to the start screen, from there out of the app.
    Back,
    Quit,
}

impl Control {
    /// The keyboard shortcut for this command, if it has one.
    pub fn shortcut(self) -> Option<KeyCode> {
        Some(match self {
            Self::TogglePlay => KeyCode::Space,
            Self::Restart => KeyCode::Backspace,
            Self::SeekBack => KeyCode::ArrowLeft,
            Self::SeekForward => KeyCode::ArrowRight,
            Self::Slower => KeyCode::ArrowDown,
            Self::Faster => KeyCode::ArrowUp,
            Self::CycleMode => KeyCode::Tab,
            Self::CycleHands => KeyCode::F2,
            Self::ToggleLoop => KeyCode::F3,
            Self::ToggleMetronome => KeyCode::F4,
            Self::ToggleAchievements => KeyCode::F5,
            Self::ToggleSightReading => KeyCode::F6,
            Self::CycleLabels => KeyCode::F1,
            Self::PreviousSong => KeyCode::PageUp,
            Self::NextSong => KeyCode::PageDown,
            Self::StartSong => KeyCode::Enter,
            Self::Back => KeyCode::Escape,
            Self::SetMode(_)
            | Self::SetHands(_)
            | Self::SelectSong(_)
            | Self::OpenMenu
            | Self::Quit => return None,
        })
    }

    /// The chord that presses this command's button, if it has one. Buttons
    /// shown together never share a chord. Quit has none, so it can't be
    /// played by accident.
    pub fn chord(self) -> Option<ChordKey> {
        use PitchClass as P;
        Some(match self {
            // Navigation: the C major family.
            Self::StartSong | Self::TogglePlay => ChordKey::major(P::C),
            Self::NextSong => ChordKey::major(P::G),
            Self::ToggleLoop => ChordKey::major(P::F),
            Self::OpenMenu => ChordKey::minor(P::A),
            Self::SetMode(PlayMode::Listen) => ChordKey::major(P::D),
            Self::SetMode(PlayMode::Wait) => ChordKey::major(P::E),
            Self::SetMode(PlayMode::Play) => ChordKey::major(P::A),
            Self::SetHands(Hands::Both) => ChordKey::minor(P::D),
            Self::SetHands(Hands::Right) => ChordKey::minor(P::E),
            Self::SetHands(Hands::Left) => ChordKey::minor(P::B),
            Self::ToggleSightReading => ChordKey::major(P::ASharp),
            Self::ToggleMetronome => ChordKey::major(P::DSharp),
            Self::CycleLabels => ChordKey::major(P::GSharp),
            Self::ToggleAchievements => ChordKey::major(P::CSharp),
            Self::SelectSong(i) => return SONG_CHORDS.get(i).copied(),
            _ => return None,
        })
    }

    /// A stable text id, e.g. for menu-bar items.
    pub fn id(self) -> String {
        match self {
            Self::SetMode(mode) => format!("mode:{}", mode.label()),
            Self::SetHands(hands) => format!("hands:{}", hands.label()),
            Self::SelectSong(i) => format!("song:{i}"),
            other => format!("{other:?}"),
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        if let Some(label) = id.strip_prefix("mode:") {
            return PlayMode::ALL
                .into_iter()
                .find(|m| m.label() == label)
                .map(Self::SetMode);
        }
        if let Some(label) = id.strip_prefix("hands:") {
            return Hands::ALL
                .into_iter()
                .find(|h| h.label() == label)
                .map(Self::SetHands);
        }
        if let Some(index) = id.strip_prefix("song:") {
            return index.parse().ok().map(Self::SelectSong);
        }
        Self::PLAIN.into_iter().find(|c| c.id() == id)
    }

    /// Every command without a parameter.
    const PLAIN: [Self; 19] = [
        Self::TogglePlay,
        Self::Restart,
        Self::SeekBack,
        Self::SeekForward,
        Self::Slower,
        Self::Faster,
        Self::CycleMode,
        Self::CycleHands,
        Self::ToggleLoop,
        Self::ToggleMetronome,
        Self::ToggleAchievements,
        Self::ToggleSightReading,
        Self::CycleLabels,
        Self::PreviousSong,
        Self::NextSong,
        Self::StartSong,
        Self::OpenMenu,
        Self::Back,
        Self::Quit,
    ];

    /// Commands bound to a key, for mapping key presses.
    const SHORTCUTS: [Self; 17] = [
        Self::TogglePlay,
        Self::Restart,
        Self::SeekBack,
        Self::SeekForward,
        Self::Slower,
        Self::Faster,
        Self::CycleMode,
        Self::CycleHands,
        Self::ToggleLoop,
        Self::ToggleMetronome,
        Self::ToggleAchievements,
        Self::ToggleSightReading,
        Self::CycleLabels,
        Self::PreviousSong,
        Self::NextSong,
        Self::StartSong,
        Self::Back,
    ];
}

/// Song buttons, in playlist order: minor chords not used elsewhere.
const SONG_CHORDS: [ChordKey; 8] = [
    ChordKey::minor(PitchClass::C),
    ChordKey::minor(PitchClass::G),
    ChordKey::minor(PitchClass::F),
    ChordKey::minor(PitchClass::FSharp),
    ChordKey::minor(PitchClass::CSharp),
    ChordKey::minor(PitchClass::GSharp),
    ChordKey::minor(PitchClass::DSharp),
    ChordKey::minor(PitchClass::ASharp),
];

/// Extra keys that also trigger a command.
const ALIASES: [(KeyCode, Control); 2] = [
    (KeyCode::BracketLeft, Control::Slower),
    (KeyCode::BracketRight, Control::Faster),
];

/// A clickable UI node that sends a command.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlButton(pub Control);

/// Turning input into [`Control`]s, ahead of everything that reacts to them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReadControls;

pub fn plugin(app: &mut App) {
    app.add_message::<Control>()
        .init_resource::<ChordHand>()
        .add_systems(Update, (keyboard, chords).in_set(ReadControls))
        .add_observer(clicked);
}

/// Notes held for chord shortcuts, and whether the current chord has
/// already pressed something.
#[derive(Resource, Debug, Default)]
struct ChordHand {
    held: HashSet<MidiNote>,
    fired: bool,
}

/// Playing a visible button's chord presses it, once per chord: let go of
/// every key before it can fire again. Hidden buttons (e.g. everything
/// while a song plays) never match, so chords mid-song are just notes.
fn chords(
    mut inputs: MessageReader<NoteInput>,
    buttons: Query<(&ControlButton, &InheritedVisibility)>,
    mut hand: ResMut<ChordHand>,
    mut out: MessageWriter<Control>,
) {
    let mut struck = false;
    inputs.read().for_each(|input| match input.edge {
        KeyEdge::Down => {
            hand.held.insert(input.note);
            struck = true;
        }
        KeyEdge::Up => {
            hand.held.remove(&input.note);
        }
    });
    if hand.held.is_empty() {
        hand.fired = false;
    }
    if !struck || hand.fired {
        return;
    }
    let held: Vec<MidiNote> = hand.held.iter().copied().collect();
    let pressed = buttons
        .iter()
        .filter(|(_, visible)| visible.get())
        .map(|(button, _)| button.0)
        .find(|control| control.chord().is_some_and(|c| c.matches(&held)));
    if let Some(control) = pressed {
        info!(?control, "chord pressed a button");
        out.write(control);
        hand.fired = true;
    }
}

fn keyboard(keys: Res<ButtonInput<KeyCode>>, mut out: MessageWriter<Control>) {
    let shortcuts = Control::SHORTCUTS
        .into_iter()
        .filter_map(|c| c.shortcut().map(|k| (k, c)));
    out.write_batch(
        shortcuts
            .chain(ALIASES)
            .filter(|(key, _)| keys.just_pressed(*key))
            .map(|(_, control)| control),
    );
}

/// Clicks on a button (or anything inside it) send its command once.
fn clicked(
    mut click: On<PointerClick>,
    buttons: Query<&ControlButton>,
    mut out: MessageWriter<Control>,
) {
    if let Ok(button) = buttons.get(click.entity) {
        out.write(button.0);
        click.propagate(false);
    }
}
