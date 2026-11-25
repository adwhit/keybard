//! Pausing without leaving the piano. Three neighbouring keys held together
//! (C, C#, D, say) pause the song, and do it again to carry on. While
//! paused, a panel of settings appears over the song, each button with the
//! chord that presses it, so speed, mode, hands and the rest can all be
//! changed from the keys.
//!
//! The cluster's own notes would otherwise count as wrong notes. Before
//! each key goes down the judge is copied, and pausing puts back the copy
//! from just before the cluster's first key.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use tracing::info;

use super::controls::Control;
use super::input::{KeyEdge, NoteInput};
use super::menu::Screen;
use super::songs::Playlist;
use super::transport::{PlayState, Transport};
use super::widgets::{button, capitalise, panel_title, section_label};
use super::{Judge, Phase, fonts, palette};
use crate::chord_keys::pause_cluster;
use crate::music::MidiNote;
use crate::performance::{Hands, Performance, PlayMode, Timing};

pub fn plugin(app: &mut App) {
    app.init_resource::<PauseHand>()
        .add_systems(
            Update,
            pause_by_cluster
                .in_set(Phase::Simulate)
                .before(super::transport::controls),
        )
        .add_systems(Startup, spawn_panel)
        .add_systems(Update, (show, show_speed).in_set(Phase::Present));
}

/// Keys held for the pause cluster, with the judge as it stood before each
/// went down.
#[derive(Resource, Debug, Default)]
struct PauseHand {
    held: HashSet<MidiNote>,
    /// Per held key: when it went down (in key presses) and the judge just
    /// before it was judged.
    before: HashMap<MidiNote, (u64, Performance)>,
    presses: u64,
    /// The current cluster has already paused or resumed: let go first.
    fired: bool,
}

/// Runs before the press is judged, so the copy is taken in time and the
/// last key of the cluster, arriving once the song is paused, isn't judged
/// at all.
fn pause_by_cluster(
    mut inputs: MessageReader<NoteInput>,
    mut hand: ResMut<PauseHand>,
    mut judge: ResMut<Judge>,
    transport: Res<Transport>,
    screen: Res<Screen>,
    mut out: MessageWriter<Control>,
) {
    let playing = transport.state == PlayState::Playing;
    let mut struck = false;
    inputs.read().for_each(|input| match input.edge {
        KeyEdge::Down => {
            struck = true;
            hand.held.insert(input.note);
            hand.presses += 1;
            if playing {
                let entry = (hand.presses, judge.0.clone());
                hand.before.entry(input.note).or_insert(entry);
            }
        }
        KeyEdge::Up => {
            hand.held.remove(&input.note);
            hand.before.remove(&input.note);
        }
    });
    let held: Vec<MidiNote> = hand.held.iter().copied().collect();
    let Some(cluster) = pause_cluster(&held) else {
        hand.fired = false;
        return;
    };
    let in_song = *screen == Screen::Song && transport.state != PlayState::Finished;
    if !struck || hand.fired || !in_song {
        return;
    }
    hand.fired = true;
    if playing
        && let Some((_, before)) = cluster
            .iter()
            .filter_map(|key| hand.before.get(key))
            .min_by_key(|(at, _)| *at)
    {
        judge.0 = before.clone();
    }
    info!(?cluster, "pause cluster");
    out.write(Control::TogglePlay);
}

/// The panel over the song while it's paused.
#[derive(Component, Debug)]
pub struct PausePanel;

/// The speed, between the Slower and Faster buttons.
#[derive(Component, Debug)]
struct SpeedReadout;

/// How to get going again.
const HINT: &str =
    "Hold three neighbouring keys (C C# D) again to carry on, or play a button's chord";

fn row(children: impl Bundle) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        },
        children,
    )
}

/// A labelled row of buttons: "MODE  [Play] [Listen] [Wait]".
fn section(label: &str, buttons: impl Bundle) -> impl Bundle {
    row((Children::spawn((
        Spawn(section_label(label)),
        Spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                ..default()
            },
            buttons,
        )),
    )),))
}

fn spawn_panel(mut commands: Commands, playlist: Res<Playlist>) {
    let several_songs = playlist.entries().len() > 1;
    let modes = PlayMode::ALL.map(|m| button(Control::SetMode(m), capitalise(m.label()), 15.0));
    let hands = Hands::ALL.map(|h| button(Control::SetHands(h), capitalise(h.label()), 15.0));
    commands
        .spawn((
            PausePanel,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.55)),
            GlobalZIndex(20),
            Visibility::Hidden,
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Start,
                        row_gap: Val::Px(14.0),
                        padding: UiRect::axes(Val::Px(28.0), Val::Px(22.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(palette::PANEL.with_alpha(1.0)),
                    BorderColor::all(palette::timing(Timing::Perfect).with_alpha(0.7)),
                ))
                .with_children(|card| {
                    card.spawn(panel_title("PAUSED"));
                    card.spawn((
                        Text::new(HINT),
                        fonts::regular(13.0),
                        TextColor(palette::HUD_DIM),
                    ));
                    card.spawn(row(())).with_children(|actions| {
                        actions.spawn(button(Control::TogglePlay, "Resume", 20.0));
                        actions.spawn(button(Control::Restart, "Restart", 16.0));
                        if several_songs {
                            actions.spawn(button(Control::NextSong, "Next song", 16.0));
                        }
                        actions.spawn(button(Control::OpenMenu, "Start screen", 16.0));
                    });
                    card.spawn(section(
                        "SPEED",
                        Children::spawn((
                            Spawn(button(Control::Slower, "Slower", 15.0)),
                            Spawn((
                                SpeedReadout,
                                Text::new("100%"),
                                fonts::numbers(22.0),
                                TextColor(palette::HUD_TEXT),
                                Node {
                                    min_width: Val::Px(70.0),
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                                TextLayout::justify(Justify::Center),
                            )),
                            Spawn(button(Control::Faster, "Faster", 15.0)),
                        )),
                    ));
                    card.spawn(section(
                        "MODE",
                        Children::spawn(SpawnIter(modes.into_iter())),
                    ));
                    card.spawn(section(
                        "HANDS",
                        Children::spawn(SpawnIter(hands.into_iter())),
                    ));
                    card.spawn(section(
                        "OPTIONS",
                        children![
                            button(Control::ToggleLoop, "Loop 4 bars", 15.0),
                            button(Control::ToggleMetronome, "Metronome", 15.0),
                            button(Control::ToggleSightReading, "Sight-reading", 15.0),
                            button(Control::CycleLabels, "Bar labels", 15.0),
                            button(Control::ToggleMidiEcho, "Mute MIDI", 15.0),
                        ],
                    ));
                });
        });
}

fn show(
    screen: Res<Screen>,
    transport: Res<Transport>,
    mut panel: Single<&mut Visibility, With<PausePanel>>,
) {
    let paused = *screen == Screen::Song && transport.state == PlayState::Paused;
    panel.set_if_neq(if paused {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
}

fn show_speed(transport: Res<Transport>, mut readout: Single<&mut Text, With<SpeedReadout>>) {
    let wanted = format!("{:.0}%", transport.speed.get() * 100.0);
    if readout.0 != wanted {
        readout.0 = wanted;
    }
}
