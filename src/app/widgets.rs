//! Clickable buttons: how they look, and which ones show as selected.

use bevy::picking::hover::PickingInteraction;
use bevy::prelude::*;

use super::controls::{Control, ControlButton};
use super::falling::View;
use super::input::MidiEcho;
use super::songs::Playlist;
use super::transport::{Metronome, Transport};
use super::{Judge, Phase, palette};

/// A button sending `control`, with a text label and, if the command has a
/// chord shortcut, a badge naming it.
pub fn button(control: Control, label: impl Into<String>, size: f32) -> impl Bundle {
    (
        ControlButton(control),
        ChordBadges(control.chord().map(|c| c.label())),
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

/// The chord to badge a button with, added once the button exists.
#[derive(Component, Debug, Clone)]
struct ChordBadges(Option<String>);

/// The little chord name in a button's corner.
#[derive(Component, Debug)]
pub struct ChordBadge;

/// Badges go on as soon as a button exists, so no frame shows it bare.
fn add_badge(added: On<Add<ChordBadges>>, badges: Query<&ChordBadges>, mut commands: Commands) {
    let button = added.entity;
    if let Ok(ChordBadges(Some(chord))) = badges.get(button) {
        commands.entity(button).with_child((
            ChordBadge,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(-7.0),
                right: Val::Px(-7.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(0.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(palette::CHORD_BADGE),
            children![(
                Text::new(chord.clone()),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(palette::DARK_INK),
            )],
        ));
    }
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
    midi_muted: bool,
}

impl Settings {
    fn selects(self, control: Control) -> bool {
        match control {
            Control::SetMode(mode) => mode == self.mode,
            Control::SetHands(hands) => hands == self.hands,
            Control::SelectSong(i) => i == self.song,
            Control::ToggleSightReading => self.sight_reading,
            Control::ToggleMetronome => self.metronome,
            Control::ToggleMidiEcho => self.midi_muted,
            // The main action always stands out.
            Control::StartSong => true,
            _ => false,
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_observer(add_badge)
        .add_systems(Update, style_buttons.in_set(Phase::Present));
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
    echo: Res<MidiEcho>,
) {
    let settings = Settings {
        mode: transport.mode,
        hands: judge.0.hands(),
        song: playlist.current(),
        sight_reading: *view == View::SightReading,
        metronome: *metronome == Metronome::On,
        midi_muted: *echo == MidiEcho::Mute,
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
