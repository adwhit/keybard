//! Every user command as one message, whatever it came from: a key
//! shortcut, an on-screen button or the menu bar.
//!
//! Systems react to [`Control`] rather than to raw keys, so a mouse click and
//! its shortcut always do the same thing.

use bevy::picking::events::PointerClick;
use bevy::prelude::*;

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
        .add_systems(Update, keyboard.in_set(ReadControls))
        .add_observer(clicked);
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
