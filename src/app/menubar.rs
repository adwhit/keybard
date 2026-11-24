//! The native macOS menu bar: keybard, Song, Play, Mode, View and Window
//! menus. Every item sends a [`Control`], like its key shortcut would.

use bevy::prelude::*;
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{
    AboutMetadata, CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem,
    Submenu,
};
use tracing::{info, warn};

use super::controls::{Control, ReadControls};
use super::falling::View;
use super::songs::{Playlist, Titles};
use super::transport::{Metronome, Transport};
use super::widgets::capitalise;
use super::{Judge, Phase};
use crate::performance::{Hands, PlayMode};

/// The installed menus; they must live (on the main thread) as long as the
/// app does.
struct MenuBar {
    _menu: Menu,
    checks: Vec<(Control, CheckMenuItem)>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, install.run_if(not_installed))
        .add_systems(Update, forward.in_set(ReadControls))
        .add_systems(Update, sync_checks.in_set(Phase::Present));
}

fn not_installed(bar: Option<NonSend<MenuBar>>) -> bool {
    bar.is_none()
}

fn item(control: Control, text: &str) -> MenuItem {
    MenuItem::with_id(control.id(), text, true, None)
}

fn check(
    control: Control,
    text: &str,
    checks: &mut Vec<(Control, CheckMenuItem)>,
) -> CheckMenuItem {
    let item = CheckMenuItem::with_id(control.id(), text, true, false, None);
    checks.push((control, item.clone()));
    item
}

fn submenu(title: &str, items: &[&dyn IsMenuItem]) -> Submenu {
    let menu = Submenu::new(title, true);
    if let Err(err) = menu.append_items(items) {
        warn!(%err, title, "could not build menu");
    }
    menu
}

/// Build and install the menus (an exclusive system, so on the main thread,
/// once the window's event loop is running).
fn install(world: &mut World) {
    let titles = world.resource::<Titles>().0.clone();
    let mut checks = Vec::new();
    let separator = PredefinedMenuItem::separator;

    let app_menu = submenu(
        "keybard",
        &[
            &PredefinedMenuItem::about(
                None,
                Some(AboutMetadata {
                    name: Some("keybard".into()),
                    version: Some(env!("CARGO_PKG_VERSION").into()),
                    ..default()
                }),
            ),
            &separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &separator(),
            // Our own Quit, so the app shuts down cleanly.
            &MenuItem::with_id(
                Control::Quit.id(),
                "Quit keybard",
                true,
                Some(Accelerator::new(Modifiers::META, Code::KeyQ)),
            ),
        ],
    );

    let songs: Vec<CheckMenuItem> = titles
        .iter()
        .enumerate()
        .map(|(i, title)| check(Control::SelectSong(i), title, &mut checks))
        .collect();
    let mut song_items: Vec<&dyn IsMenuItem> = Vec::new();
    let start_screen = item(Control::OpenMenu, "Start Screen");
    let start = item(Control::StartSong, "Start Song");
    let (sep_a, sep_b) = (separator(), separator());
    let (previous, next) = (
        item(Control::PreviousSong, "Previous Song"),
        item(Control::NextSong, "Next Song"),
    );
    song_items.extend([&start_screen as &dyn IsMenuItem, &start, &sep_a]);
    song_items.extend(songs.iter().map(|s| s as &dyn IsMenuItem));
    song_items.extend([&sep_b as &dyn IsMenuItem, &previous, &next]);
    let song_menu = submenu("Song", &song_items);

    let play_menu = submenu(
        "Play",
        &[
            &item(Control::TogglePlay, "Play / Pause"),
            &item(Control::Restart, "Restart"),
            &separator(),
            &item(Control::SeekBack, "Back 5 Seconds"),
            &item(Control::SeekForward, "Forward 5 Seconds"),
            &separator(),
            &item(Control::Slower, "Slower"),
            &item(Control::Faster, "Faster"),
            &separator(),
            &item(Control::ToggleLoop, "Loop 4 Bars"),
            &check(Control::ToggleMetronome, "Metronome", &mut checks),
        ],
    );

    let modes: Vec<CheckMenuItem> = PlayMode::ALL
        .into_iter()
        .map(|m| check(Control::SetMode(m), m.label(), &mut checks))
        .collect();
    let hands: Vec<CheckMenuItem> = Hands::ALL
        .into_iter()
        .map(|h| check(Control::SetHands(h), &capitalise(h.label()), &mut checks))
        .collect();
    let sep_c = separator();
    let mut mode_items: Vec<&dyn IsMenuItem> = modes.iter().map(|m| m as &dyn IsMenuItem).collect();
    mode_items.push(&sep_c);
    mode_items.extend(hands.iter().map(|h| h as &dyn IsMenuItem));
    let mode_menu = submenu("Mode", &mode_items);

    let view_menu = submenu(
        "View",
        &[
            &check(Control::ToggleSightReading, "Sight-Reading", &mut checks),
            &item(Control::CycleLabels, "Cycle Bar Labels"),
            &item(Control::ToggleAchievements, "Achievements"),
            &separator(),
            &PredefinedMenuItem::fullscreen(None),
        ],
    );

    let window_menu = submenu(
        "Window",
        &[
            &PredefinedMenuItem::minimize(None),
            &PredefinedMenuItem::maximize(None),
            &separator(),
            &PredefinedMenuItem::bring_all_to_front(None),
        ],
    );

    let menu = Menu::new();
    if let Err(err) = menu.append_items(&[
        &app_menu,
        &song_menu,
        &play_menu,
        &mode_menu,
        &view_menu,
        &window_menu,
    ]) {
        warn!(%err, "could not build the menu bar");
        return;
    }
    menu.init_for_nsapp();
    window_menu.set_as_windows_menu_for_nsapp();
    info!("installed the menu bar");
    world.insert_non_send(MenuBar {
        _menu: menu,
        checks,
    });
}

/// Menu clicks become commands.
fn forward(mut out: MessageWriter<Control>) {
    out.write_batch(
        MenuEvent::receiver()
            .try_iter()
            .filter_map(|event| Control::from_id(&event.id.0)),
    );
}

/// Tick the items for the settings in effect.
fn sync_checks(
    bar: Option<NonSend<MenuBar>>,
    transport: Res<Transport>,
    judge: Res<Judge>,
    playlist: Res<Playlist>,
    view: Res<View>,
    metronome: Res<Metronome>,
) {
    let Some(bar) = bar else {
        return;
    };
    bar.checks.iter().for_each(|(control, item)| {
        let on = match *control {
            Control::SetMode(mode) => mode == transport.mode,
            Control::SetHands(hands) => hands == judge.0.hands(),
            Control::SelectSong(i) => i == playlist.current(),
            Control::ToggleSightReading => *view == View::SightReading,
            Control::ToggleMetronome => *metronome == Metronome::On,
            _ => false,
        };
        if item.is_checked() != on {
            item.set_checked(on);
        }
    });
}
