//! Who's playing: where their bests, achievements and practice log live,
//! saving their settings whenever one changes, and the players screen for
//! switching player or typing in a new one.

use std::collections::BTreeSet;

use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use tracing::{info, warn};

use super::book::ChordBook;
use super::controls::{Control, ReadControls, Typing};
use super::falling::{BarLabels, View};
use super::input::MidiEcho;
use super::menu::Screen;
use super::practice::Practice;
use super::results::RecordStore;
use super::results::{Bests, Unlocked};
use super::songs::Titles;
use super::transport::{Metronome, Transport};
use super::widgets::{button, label_text};
use super::{ActiveSong, Judge};
use super::{Phase, palette};
use crate::chord_keys::Inversion;
use crate::layout;
use crate::performance::Timing;
use crate::saves::{PlayerName, SaveFile, Saves, Settings};

/// The player whose saves are in use.
#[derive(Resource, Debug, Clone)]
pub struct Profile {
    pub saves: Saves,
    pub player: PlayerName,
}

impl Profile {
    pub fn store(&self, file: SaveFile) -> RecordStore {
        self.saves
            .file(&self.player, file)
            .map_or(RecordStore::Memory, RecordStore::File)
    }
}

pub fn plugin(profile: Profile) -> impl Fn(&mut App) {
    move |app: &mut App| {
        app.insert_resource(PlayerList(profile.saves.players()))
            .init_resource::<SongRequest>()
            .add_systems(Update, request_song.in_set(ReadControls))
            .insert_resource(profile.clone())
            .add_systems(Startup, spawn_screen)
            .add_systems(
                Update,
                (start_typing, type_name, choose)
                    .chain()
                    .in_set(Phase::Input),
            )
            .add_systems(
                Update,
                (fill_rows.run_if(resource_changed::<PlayerList>), show)
                    .chain()
                    .in_set(Phase::Present),
            )
            .add_systems(Last, save_settings);
    }
}

/// The players to choose from, in the order their buttons are numbered.
#[derive(Resource, Debug, Clone, PartialEq)]
struct PlayerList(Vec<PlayerName>);

#[derive(Component, Debug)]
pub struct PlayersScreen;

#[derive(Component, Debug)]
struct PlayerRows;

/// The name being typed (or the prompt to add one).
#[derive(Component, Debug)]
struct NameLine;

fn spawn_screen(mut commands: Commands) {
    commands.spawn((
        PlayersScreen,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent((1.0 - layout::WHITE_KEY_HEIGHT / layout::CANVAS_HEIGHT) * 100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::top(Val::Percent(5.0)),
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(palette::BACKGROUND),
        GlobalZIndex(30),
        Visibility::Hidden,
        children![
            (
                Text::new("WHO'S PLAYING?"),
                TextFont {
                    font_size: FontSize::Px(40.0),
                    ..default()
                },
                TextColor(palette::timing(Timing::Perfect)),
            ),
            label_text(
                "Each player keeps their own bests, streak and settings.",
                14.0
            ),
            (
                PlayerRows,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
            ),
            (
                NameLine,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(palette::CHORD_BADGE),
            ),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(12.0),
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
                children![
                    button(Control::NewPlayer, "New player", 16.0),
                    button(Control::Back, "Back", 16.0),
                ],
            ),
        ],
    ));
}

fn fill_rows(
    mut commands: Commands,
    rows: Single<Entity, With<PlayerRows>>,
    list: Res<PlayerList>,
) {
    let rows = *rows;
    commands.entity(rows).despawn_children();
    list.0.iter().enumerate().for_each(|(i, name)| {
        let row = commands
            .spawn(button(Control::SelectPlayer(i), name.as_str(), 18.0))
            .insert(Node {
                width: Val::Px(320.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            })
            .id();
        commands.entity(rows).add_child(row);
    });
}

fn start_typing(
    mut controls: MessageReader<Control>,
    screen: Res<Screen>,
    mut typing: ResMut<Typing>,
) {
    if *screen == Screen::Players && controls.read().any(|c| *c == Control::NewPlayer) {
        typing.0 = Some(String::new());
    }
}

/// While typing: letters add to the name, Backspace removes one, Enter
/// creates the player, Escape gives up.
fn type_name(
    mut keys: MessageReader<KeyboardInput>,
    mut typing: ResMut<Typing>,
    mut profile: ResMut<Profile>,
    mut list: ResMut<PlayerList>,
    mut screen: ResMut<Screen>,
    mut switch: Switch,
) {
    let Some(name) = typing.0.as_mut() else {
        keys.clear();
        return;
    };
    let mut done = None;
    keys.read()
        .filter(|key| key.state == ButtonState::Pressed)
        .for_each(|key| match key.key_code {
            KeyCode::Enter | KeyCode::NumpadEnter => done = Some(true),
            KeyCode::Escape => done = Some(false),
            KeyCode::Backspace => {
                name.pop();
            }
            _ => key
                .text
                .iter()
                .flat_map(|text| text.chars())
                .filter(|c| PlayerName::allows(*c))
                .for_each(|c| {
                    if name.chars().count() < PlayerName::MAX_LEN {
                        name.push(c);
                    }
                }),
        });
    match done {
        Some(true) => match PlayerName::new(name) {
            Ok(player) => {
                typing.0 = None;
                match profile.saves.create(&player) {
                    Ok(player) => {
                        list.0 = profile.saves.players();
                        switch.to(&mut profile, player);
                        *screen = Screen::Menu;
                    }
                    Err(err) => warn!(%err, "could not create player"),
                }
            }
            // An unusable name: keep typing.
            Err(err) => info!(%err, "not a usable player name"),
        },
        Some(false) => typing.0 = None,
        None => {}
    }
}

/// Picking a player from the list.
fn choose(
    mut controls: MessageReader<Control>,
    mut profile: ResMut<Profile>,
    list: Res<PlayerList>,
    mut screen: ResMut<Screen>,
    mut switch: Switch,
) {
    if *screen != Screen::Players {
        controls.clear();
        return;
    }
    let picked = controls.read().find_map(|c| match c {
        Control::SelectPlayer(i) => list.0.get(*i).cloned(),
        _ => None,
    });
    if let Some(player) = picked {
        profile.saves.choose(&player);
        switch.to(&mut profile, player);
        *screen = Screen::Menu;
    }
}

/// Everything that changes with the player: their records and settings.
#[derive(SystemParam)]
struct Switch<'w> {
    bests: ResMut<'w, Bests>,
    unlocked: ResMut<'w, Unlocked>,
    practice: ResMut<'w, Practice>,
    transport: ResMut<'w, Transport>,
    judge: ResMut<'w, Judge>,
    view: ResMut<'w, View>,
    metronome: ResMut<'w, Metronome>,
    labels: ResMut<'w, BarLabels>,
    echo: ResMut<'w, MidiEcho>,
    book: ResMut<'w, ChordBook>,
    titles: Res<'w, Titles>,
    song: ResMut<'w, SongRequest>,
}

/// The song a newly switched-to player last chose, to select next frame.
#[derive(Resource, Debug, Default)]
struct SongRequest(Option<usize>);

fn request_song(mut request: ResMut<SongRequest>, mut out: MessageWriter<Control>) {
    if let Some(index) = request.0.take() {
        out.write(Control::SelectSong(index));
    }
}

impl Switch<'_> {
    /// Become `player`: load their records and apply their settings.
    fn to(&mut self, profile: &mut Profile, player: PlayerName) {
        info!(player = player.as_str(), "switching player");
        profile.player = player;
        *self.bests = Bests::open(profile.store(SaveFile::Bests));
        *self.unlocked = Unlocked::open(profile.store(SaveFile::Achievements));
        self.practice.switch(profile.store(SaveFile::Practice));
        let settings = profile.saves.load_settings(&profile.player);
        self.transport.mode = settings.mode;
        self.judge.0.set_hands(settings.hands);
        *self.view = if settings.sight_reading {
            View::SightReading
        } else {
            View::Bars
        };
        *self.metronome = if settings.metronome {
            Metronome::On
        } else {
            Metronome::Off
        };
        *self.labels = settings.labels;
        *self.echo = if settings.midi_muted {
            MidiEcho::Mute
        } else {
            MidiEcho::Play
        };
        *self.book = ChordBook::with_progress(settings.chord_book.iter().copied());
        self.song.0 = settings
            .song
            .and_then(|title| self.titles.0.iter().position(|t| *t == title));
    }
}

fn show(
    screen: Res<Screen>,
    typing: Res<Typing>,
    mut root: Single<&mut Visibility, With<PlayersScreen>>,
    mut line: Single<&mut Text, With<NameLine>>,
) {
    let open = *screen == Screen::Players;
    root.set_if_neq(if open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    let wanted = match &typing.0 {
        Some(name) => format!("Name: {name}_   (Enter to add, Esc to cancel)"),
        None => String::new(),
    };
    if line.0 != wanted {
        line.0 = wanted;
    }
}

/// The settings in effect right now.
#[expect(clippy::too_many_arguments, reason = "one per setting")]
fn gather(
    transport: &Transport,
    judge: &Judge,
    view: View,
    metronome: Metronome,
    labels: BarLabels,
    echo: MidiEcho,
    song: &ActiveSong,
    chord_book: BTreeSet<(usize, Inversion)>,
) -> Settings {
    Settings {
        mode: transport.mode,
        hands: judge.0.hands(),
        sight_reading: view == View::SightReading,
        metronome: metronome == Metronome::On,
        labels,
        midi_muted: echo == MidiEcho::Mute,
        song: Some(song.0.title.clone()),
        chord_book,
    }
}

/// Save the settings once they differ from what's saved. The first frame
/// only notes the starting point.
#[expect(clippy::too_many_arguments, reason = "Bevy system parameters")]
fn save_settings(
    profile: Res<Profile>,
    transport: Res<Transport>,
    judge: Res<Judge>,
    view: Res<View>,
    metronome: Res<Metronome>,
    labels: Res<BarLabels>,
    echo: Res<MidiEcho>,
    song: Res<ActiveSong>,
    book: Res<ChordBook>,
    mut saved: Local<Option<(PlayerName, Settings)>>,
) {
    let now = gather(
        &transport,
        &judge,
        *view,
        *metronome,
        *labels,
        *echo,
        &song,
        book.progress(),
    );
    let changed = saved
        .as_ref()
        .is_some_and(|(player, settings)| *player == profile.player && *settings != now);
    if changed {
        profile.saves.save_settings(&profile.player, &now);
    }
    *saved = Some((profile.player.clone(), now));
}
