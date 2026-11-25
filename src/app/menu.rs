//! The start screen: pick a song, a mode and hands, then start. Escape
//! during a song comes back here; Escape here quits.

use bevy::prelude::*;
use tracing::info;

use super::controls::{Control, ControlButton};
use super::input::HeldKeys;
use super::players::Profile;
use super::practice::Practice;
use super::results::{Bests, LastResult};
use super::songs::{Playlist, Titles};
use super::transport::{self, PlayState, Transport};
use super::widgets::{button, capitalise, label_text};
use super::{ActiveSong, Judge, Phase, palette};
use crate::challenge::for_day;
use crate::chords;
use crate::key::KeySignature;
use crate::layout;
use crate::music::MidiNote;
use crate::performance::{Grade, Hands, PlayMode, Timing};
use crate::records::RecordKey;

/// Which screen is showing.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// The start screen; the song waits behind it, paused.
    Menu,
    /// The chord book, opened from the start screen.
    ChordBook,
    /// Choosing or adding a player, from the start screen.
    Players,
    Song,
}

/// The start screen's root node.
#[derive(Component, Debug)]
pub struct StartScreen;

/// Where the song buttons go.
#[derive(Component, Debug)]
struct SongRows;

/// The best-score label on a song's button.
#[derive(Component, Debug)]
struct SongBest(usize);

#[derive(Component, Debug)]
struct ChallengeLine;

/// "Playing as ..." on the start screen.
#[derive(Component, Debug)]
struct PlayingAs;

#[derive(Component, Debug)]
struct ModeHint;

/// Names the notes or chord being held.
#[derive(Component, Debug)]
struct ChordReadout;

pub fn plugin(screen: Screen) -> impl Fn(&mut App) {
    move |app: &mut App| {
        app.insert_resource(screen)
            .add_systems(Startup, spawn_screen)
            .add_systems(Update, navigate.in_set(Phase::Input))
            .add_systems(
                Update,
                hold_on_menu
                    .in_set(Phase::Simulate)
                    .after(transport::controls)
                    .before(transport::advance),
            )
            .add_systems(
                Update,
                (
                    fill_song_rows.run_if(resource_changed::<Titles>),
                    refresh,
                    show_held,
                )
                    .chain()
                    .in_set(Phase::Present),
            );
    }
}

fn navigate(
    mut controls: MessageReader<Control>,
    mut screen: ResMut<Screen>,
    mut transport: ResMut<Transport>,
    song: Res<ActiveSong>,
    mut judge: ResMut<Judge>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    controls.read().for_each(|control| match (*control, *screen) {
        (Control::StartSong, Screen::Menu) => {
            *screen = Screen::Song;
            judge.0.reset(&song.0);
            transport.time = transport.start;
            transport.state = PlayState::Playing;
            info!(title = %song.0.title, mode = transport.mode.label(), "started from the start screen");
        }
        (Control::OpenMenu | Control::Back, Screen::Song) => {
            *screen = Screen::Menu;
            judge.0.reset(&song.0);
            transport.time = transport.start;
            transport.state = PlayState::Paused;
            commands.remove_resource::<LastResult>();
            info!("back to the start screen");
        }
        (Control::OpenChordBook, Screen::Menu) => {
            *screen = Screen::ChordBook;
            info!("opened the chord book");
        }
        (Control::OpenPlayers, Screen::Menu) => {
            *screen = Screen::Players;
        }
        (Control::Back | Control::OpenMenu, Screen::ChordBook | Screen::Players) => {
            *screen = Screen::Menu;
        }
        (Control::Back, Screen::Menu) | (Control::Quit, _) => {
            exit.write(AppExit::Success);
        }
        _ => {}
    });
}

/// Nothing plays behind the start screen, whatever else asks it to.
fn hold_on_menu(screen: Res<Screen>, mut transport: ResMut<Transport>) {
    if *screen != Screen::Song && transport.state == PlayState::Playing {
        transport.state = PlayState::Paused;
    }
}

fn heading(value: &str) -> impl Bundle {
    (
        Text::new(value.to_uppercase()),
        super::fonts::bold(12.0),
        TextColor(palette::HUD_DIM),
        Node {
            margin: UiRect::top(Val::Px(10.0)),
            ..default()
        },
    )
}

fn row() -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(8.0),
        ..default()
    }
}

fn spawn_screen(mut commands: Commands) {
    let modes = PlayMode::ALL.map(|m| button(Control::SetMode(m), m.label(), 16.0));
    let hands = Hands::ALL.map(|h| button(Control::SetHands(h), capitalise(h.label()), 16.0));
    commands.spawn((
        StartScreen,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            // Stop above the piano, so you can see the keys you play.
            height: Val::Percent((1.0 - layout::WHITE_KEY_HEIGHT / layout::CANVAS_HEIGHT) * 100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::top(Val::Percent(4.0)),
            row_gap: Val::Px(6.0),
            ..default()
        },
        BackgroundColor(palette::BACKGROUND),
        GlobalZIndex(30),
        children![
            (
                Text::new("keybard"),
                super::fonts::bold(54.0),
                TextColor(palette::timing(Timing::Perfect)),
            ),
            label_text("Pick a song, choose how to play it, and start.", 16.0),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(12.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![
                    (PlayingAs, label_text("", 15.0)),
                    button(Control::OpenPlayers, "Switch player", 13.0),
                ],
            ),
            (
                ChallengeLine,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(palette::FIRE),
            ),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(48.0),
                    margin: UiRect::top(Val::Px(12.0)),
                    ..default()
                },
                children![
                    (
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            ..default()
                        },
                        children![
                            heading("Songs · best in this mode"),
                            (
                                SongRows,
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(4.0),
                                    ..default()
                                },
                            ),
                        ],
                    ),
                    (
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            ..default()
                        },
                        children![
                            heading("MODE"),
                            (row(), Children::spawn(SpawnIter(modes.into_iter()))),
                            (ModeHint, label_text("", 13.0)),
                            heading("HANDS"),
                            (row(), Children::spawn(SpawnIter(hands.into_iter()))),
                            heading("OPTIONS"),
                            (
                                row(),
                                children![
                                    button(Control::ToggleSightReading, "Sight-reading", 14.0),
                                    button(Control::ToggleMetronome, "Metronome", 14.0),
                                    button(Control::CycleLabels, "Bar labels", 14.0),
                                ],
                            ),
                            (
                                row(),
                                children![
                                    button(Control::ToggleMidiEcho, "Mute MIDI keyboard", 14.0),
                                    button(Control::ToggleAchievements, "Achievements", 14.0),
                                    button(Control::OpenChordBook, "Chord book", 14.0),
                                ],
                            ),
                        ],
                    ),
                ],
            ),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(16.0),
                    margin: UiRect::top(Val::Px(24.0)),
                    ..default()
                },
                children![
                    button(Control::StartSong, "Start", 26.0),
                    button(Control::Quit, "Quit", 18.0),
                ],
            ),
            label_text(
                "Play a button's chord to press it  ·  in a song, hold C C# D to pause  ·  Enter starts  ·  Esc quits",
                12.0,
            ),
            (
                ChordReadout,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(palette::CHORD_BADGE),
                Node {
                    margin: UiRect::top(Val::Px(14.0)),
                    ..default()
                },
            ),
        ],
    ));
}

/// One button per song, rebuilt when the titles are (re)loaded.
fn fill_song_rows(
    mut commands: Commands,
    rows: Single<Entity, With<SongRows>>,
    titles: Res<Titles>,
) {
    let rows = *rows;
    commands.entity(rows).despawn_children();
    titles.0.iter().enumerate().for_each(|(i, title)| {
        let row = commands
            .spawn(button(Control::SelectSong(i), title.clone(), 15.0))
            // Wider than a plain button, title left and best right.
            .insert(Node {
                width: Val::Px(600.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::SpaceBetween,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            })
            .with_child((SongBest(i), label_text("", 14.0)))
            .id();
        commands.entity(rows).add_child(row);
    });
}

/// What the held notes are: a chord name if they spell one (and whether a
/// button on screen uses it), else the notes themselves.
pub(super) fn describe_held(held: &[MidiNote], button_chord: bool) -> Option<String> {
    let mut notes = held.to_vec();
    notes.sort();
    let named = (notes.len() >= 3)
        .then(|| chords::name(&notes, KeySignature::new(0)))
        .flatten();
    let what = match named {
        Some(chord) if button_chord => format!("{chord}  (presses its button)"),
        Some(chord) => format!("{chord}  (no button)"),
        None => notes
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" "),
    };
    (!notes.is_empty()).then(|| format!("You're playing  {what}"))
}

fn show_held(
    screen: Res<Screen>,
    held: Res<HeldKeys>,
    buttons: Query<(&ControlButton, &InheritedVisibility)>,
    mut readout: Single<&mut Text, With<ChordReadout>>,
) {
    if *screen != Screen::Menu {
        return;
    }
    let notes: Vec<MidiNote> = held.0.iter().copied().collect();
    let button_chord = buttons
        .iter()
        .filter(|(_, visible)| visible.get())
        .any(|(b, _)| b.0.chord().is_some_and(|c| c.matches(&notes)));
    let wanted = describe_held(&notes, button_chord).unwrap_or_default();
    if readout.0 != wanted {
        readout.0 = wanted;
    }
}

fn grade_label(grade: Option<Grade>) -> &'static str {
    grade.map_or("-", Grade::label)
}

/// Keeps the song-best query apart from the other start-screen texts.
type NotOtherLines = (
    Without<ChallengeLine>,
    Without<ModeHint>,
    Without<PlayingAs>,
);

#[expect(clippy::too_many_arguments, reason = "Bevy system parameters")]
fn refresh(
    screen: Res<Screen>,
    mut root: Single<&mut Visibility, With<StartScreen>>,
    mut bests_text: Query<(&SongBest, &mut Text), NotOtherLines>,
    mut challenge_line: Single<&mut Text, (With<ChallengeLine>, Without<ModeHint>)>,
    mut hint: Single<&mut Text, (With<ModeHint>, Without<PlayingAs>)>,
    mut playing_as: Single<&mut Text, (With<PlayingAs>, Without<ChallengeLine>)>,
    profile: Res<Profile>,
    titles: Res<Titles>,
    playlist: Res<Playlist>,
    transport: Res<Transport>,
    judge: Res<Judge>,
    bests: Res<Bests>,
    practice: Res<Practice>,
) {
    let wanted = match *screen {
        Screen::Menu => Visibility::Inherited,
        Screen::Song | Screen::ChordBook | Screen::Players => Visibility::Hidden,
    };
    root.set_if_neq(wanted);
    if *screen != Screen::Menu {
        return;
    }
    let challenge = for_day(practice.today, playlist.entries().len());
    bests_text.iter_mut().for_each(|(best, mut text)| {
        let title = titles.0.get(best.0).cloned().unwrap_or_default();
        let record = bests.records.best(&RecordKey {
            title,
            mode: transport.mode,
            hands: judge.0.hands(),
        });
        let score = record.map_or_else(
            || "not played".to_owned(),
            |b| {
                format!(
                    "{}  {}",
                    grade_label(Some(Grade::from_accuracy(b.accuracy))),
                    b.points
                )
            },
        );
        let today = if best.0 == challenge.song {
            "today  ·  "
        } else {
            ""
        };
        text.0 = format!("{today}{score}");
    });
    challenge_line.0 = titles
        .0
        .get(challenge.song)
        .map(|title| {
            let done = if practice.log.challenge_done(practice.today) {
                "  (done!)"
            } else {
                ""
            };
            format!("{}{done}", challenge.describe(title))
        })
        .unwrap_or_default();
    let who = format!("Playing as {}", profile.player.as_str());
    if playing_as.0 != who {
        playing_as.0 = who;
    }
    hint.0 = match transport.mode {
        PlayMode::Listen => "Listen: watch and hear the song; nothing is scored.",
        PlayMode::Wait => "Wait: the music waits until you play each right note.",
        PlayMode::Play => "Play: keep up in time; scored and graded.",
    }
    .to_owned();
}
