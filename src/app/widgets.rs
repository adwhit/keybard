//! Clickable buttons: how they look, and which ones show as selected.

use bevy::picking::hover::PickingInteraction;
use bevy::prelude::*;

use super::controls::{Control, ControlButton};
use super::falling::View;
use super::songs::Playlist;
use super::transport::{Metronome, Transport};
use super::{Judge, Phase, palette};

/// A button sending `control`, with a text label.
pub fn button(control: Control, label: impl Into<String>, size: f32) -> impl Bundle {
    (
        ControlButton(control),
        PickingInteraction::default(),
        Node {
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(palette::BUTTON),
        BorderColor::all(palette::BUTTON_BORDER),
        children![label_text(label, size)],
    )
}

pub fn label_text(value: impl Into<String>, size: f32) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(palette::HUD_TEXT),
    )
}

/// "both hands" -> "Both hands".
pub fn capitalise(label: &str) -> String {
    let mut chars = label.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Whether a button's setting is the one currently in effect.
#[derive(Debug, Clone, Copy)]
struct Settings {
    mode: crate::performance::PlayMode,
    hands: crate::performance::Hands,
    song: usize,
    sight_reading: bool,
    metronome: bool,
}

impl Settings {
    fn selects(self, control: Control) -> bool {
        match control {
            Control::SetMode(mode) => mode == self.mode,
            Control::SetHands(hands) => hands == self.hands,
            Control::SelectSong(i) => i == self.song,
            Control::ToggleSightReading => self.sight_reading,
            Control::ToggleMetronome => self.metronome,
            // The main action always stands out.
            Control::StartSong => true,
            _ => false,
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, style_buttons.in_set(Phase::Present));
}

fn style_buttons(
    mut buttons: Query<(
        &ControlButton,
        &PickingInteraction,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    transport: Res<Transport>,
    judge: Res<Judge>,
    playlist: Res<Playlist>,
    view: Res<View>,
    metronome: Res<Metronome>,
) {
    let settings = Settings {
        mode: transport.mode,
        hands: judge.0.hands(),
        song: playlist.current(),
        sight_reading: *view == View::SightReading,
        metronome: *metronome == Metronome::On,
    };
    buttons
        .iter_mut()
        .for_each(|(button, interaction, mut background, mut border)| {
            let selected = settings.selects(button.0);
            let fill = match (interaction, selected) {
                (PickingInteraction::Pressed, _) => palette::BUTTON_PRESSED,
                (PickingInteraction::Hovered, true) => palette::BUTTON_SELECTED.lighter(0.06),
                (PickingInteraction::Hovered, false) => palette::BUTTON_HOVER,
                (PickingInteraction::None, true) => palette::BUTTON_SELECTED,
                (PickingInteraction::None, false) => palette::BUTTON,
            };
            background.set_if_neq(BackgroundColor(fill));
            let edge = if selected {
                palette::timing(crate::performance::Timing::Perfect)
            } else {
                palette::BUTTON_BORDER
            };
            border.set_if_neq(BorderColor::all(edge));
        });
}
