//! Switching songs at runtime (PgUp/PgDn) and the song list shown before the
//! music starts.
//!
//! Everything that depends on the song is tagged [`SongScoped`] and rebuilt
//! by systems in [`SongLoad`] whenever [`ActiveSong`] changes, including the
//! first frame.

use std::path::PathBuf;

use bevy::prelude::*;
use tracing::{info, warn};

use super::results::{Bests, LastResult};
use super::section::SectionLoop;
use super::transport::{PlayState, Transport};
use super::{ActiveSong, Judge, Keys, Phase, palette};
use crate::builtin::BuiltinSong;
use crate::error::Error;
use crate::layout::KeyRange;
use crate::midi_file;
use crate::performance::{Grade, Performance};
use crate::records::RecordKey;
use crate::song::Song;

/// Where a song comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum SongSource {
    Builtin(BuiltinSong),
    File(PathBuf),
}

impl SongSource {
    pub fn load(&self) -> Result<Song, Error> {
        match self {
            Self::Builtin(b) => b.load(),
            Self::File(path) => midi_file::load(path),
        }
    }
}

/// The songs PgUp/PgDn step through.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct Playlist {
    entries: Vec<SongSource>,
    current: usize,
}

impl Playlist {
    pub fn new(entries: Vec<SongSource>, current: usize) -> Self {
        let current = current.min(entries.len().saturating_sub(1));
        Self { entries, current }
    }

    pub fn entries(&self) -> &[SongSource] {
        &self.entries
    }

    pub fn current(&self) -> usize {
        self.current
    }

    /// Index `steps` away, wrapping around.
    fn offset(&self, steps: isize) -> Option<usize> {
        let len = self.entries.len() as isize;
        (len > 0).then(|| (self.current as isize + steps).rem_euclid(len) as usize)
    }
}

/// Belongs to the current song; despawned when the song changes.
#[derive(Component, Debug, Clone, Copy)]
pub struct SongScoped;

/// The song list overlay.
#[derive(Component, Debug, Clone, Copy)]
pub struct SongList;

/// Systems that (re)build song-dependent entities. They run when
/// [`ActiveSong`] changes, before input is read, after [`SongClear`].
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SongLoad;

/// Clears the previous song's entities, ahead of [`SongLoad`].
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct SongClear;

/// Cached titles, so the list doesn't reload every song each frame.
#[derive(Resource, Debug, Default)]
struct Titles(Vec<String>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Playlist>()
        .init_resource::<Titles>()
        .configure_sets(
            Update,
            (SongClear, SongLoad)
                .chain()
                .before(Phase::Input)
                .run_if(resource_changed::<ActiveSong>),
        )
        .add_systems(Startup, (spawn_list, cache_titles))
        .add_systems(Update, despawn_previous.in_set(SongClear))
        // Switch first, so this very frame clears, rebuilds and draws the
        // new song consistently.
        .add_systems(Update, switch.before(SongClear))
        .add_systems(Update, show_list.in_set(Phase::Present));
}

fn despawn_previous(mut commands: Commands, scoped: Query<Entity, With<SongScoped>>) {
    scoped.iter().for_each(|e| commands.entity(e).despawn());
}

fn cache_titles(playlist: Res<Playlist>, mut titles: ResMut<Titles>) {
    titles.0 = playlist
        .entries
        .iter()
        .map(|source| match source.load() {
            Ok(song) => song.title,
            Err(_) => match source {
                SongSource::Builtin(b) => format!("{b:?}"),
                SongSource::File(path) => path.display().to_string(),
            },
        })
        .collect();
}

#[expect(
    clippy::too_many_arguments,
    reason = "a song switch touches every piece of state"
)]
fn switch(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut playlist: ResMut<Playlist>,
    mut song: ResMut<ActiveSong>,
    mut range: ResMut<Keys>,
    mut judge: ResMut<Judge>,
    mut transport: ResMut<Transport>,
    mut section: ResMut<SectionLoop>,
) {
    let steps = [(KeyCode::PageUp, -1), (KeyCode::PageDown, 1)]
        .into_iter()
        .filter(|(k, _)| keys.just_pressed(*k))
        .map(|(_, d)| d)
        .sum::<isize>();
    let Some(index) = (steps != 0).then(|| playlist.offset(steps)).flatten() else {
        return;
    };
    let next = match playlist.entries[index].load() {
        Ok(next) => next,
        Err(err) => {
            warn!(%err, "could not load song; staying put");
            return;
        }
    };
    info!(title = %next.title, "switching song");
    playlist.current = index;
    let (low, high) = next.pitch_range();
    range.0 = KeyRange::covering(low, high);
    let hands = judge.0.hands();
    judge.0 = Performance::new(&next);
    judge.0.set_hands(hands);
    let fresh = Transport::new(transport.mode, next.tempo);
    *transport = Transport {
        speed: transport.speed,
        ..fresh
    };
    section.0 = None;
    commands.remove_resource::<LastResult>();
    song.0 = next;
}

fn spawn_list(mut commands: Commands) {
    commands.spawn((
        SongList,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(palette::HUD_TEXT),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            top: Val::Percent(40.0),
            ..default()
        },
        BackgroundColor(palette::BACKGROUND.with_alpha(0.75)),
    ));
}

fn grade_label(grade: Option<Grade>) -> &'static str {
    grade.map_or("-", Grade::label)
}

fn show_list(
    mut list: Single<(&mut Text, &mut Visibility), With<SongList>>,
    playlist: Res<Playlist>,
    titles: Res<Titles>,
    transport: Res<Transport>,
    judge: Res<Judge>,
    bests: Res<Bests>,
) {
    let (text, visibility) = &mut *list;
    let wanted = match transport.state {
        _ if playlist.entries.len() < 2 => Visibility::Hidden,
        PlayState::Paused => Visibility::Inherited,
        PlayState::Playing if transport.time.0 < 0.0 => Visibility::Inherited,
        PlayState::Playing | PlayState::Finished => Visibility::Hidden,
    };
    **visibility = wanted;
    if wanted == Visibility::Hidden {
        return;
    }
    let lines: Vec<String> = titles
        .0
        .iter()
        .enumerate()
        .map(|(i, title)| {
            let marker = if i == playlist.current { ">" } else { " " };
            let best = bests.records.best(&RecordKey {
                title: title.clone(),
                mode: transport.mode,
                hands: judge.0.hands(),
            });
            let score = best.map_or_else(
                || "   not played".to_owned(),
                |b| {
                    format!(
                        "{} {:07}",
                        grade_label(Some(Grade::from_accuracy(b.accuracy))),
                        b.points
                    )
                },
            );
            format!("{marker} {title:<36} {score}")
        })
        .collect();
    text.0 = format!(
        "SONGS  (PgUp/PgDn)   best in {} mode\n{}",
        transport.mode.label(),
        lines.join("\n")
    );
}
