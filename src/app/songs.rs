//! Switching songs at runtime (PgUp/PgDn, the start screen or the menu bar).
//!
//! Everything that depends on the song is tagged [`SongScoped`] and rebuilt
//! by systems in [`SongLoad`] whenever [`ActiveSong`] changes, including the
//! first frame.

use std::path::PathBuf;

use bevy::prelude::*;
use tracing::{info, warn};

use super::controls::{Control, ReadControls};
use super::results::LastResult;
use super::section::SectionLoop;
use super::transport::Transport;
use super::{ActiveSong, Judge, Keys, Phase};
use crate::builtin::BuiltinSong;
use crate::error::Error;
use crate::layout::KeyRange;
use crate::midi_file;
use crate::performance::Performance;
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

/// Systems that (re)build song-dependent entities. They run when
/// [`ActiveSong`] changes, before input is read, after [`SongClear`].
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SongLoad;

/// Clears the previous song's entities, ahead of [`SongLoad`].
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct SongClear;

/// Cached titles, so lists don't reload every song each frame.
#[derive(Resource, Debug, Default)]
pub struct Titles(pub Vec<String>);

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
        .add_systems(Startup, cache_titles)
        .add_systems(Update, despawn_previous.in_set(SongClear))
        // Switch first, so this very frame clears, rebuilds and draws the
        // new song consistently.
        .add_systems(Update, switch.after(ReadControls).before(SongClear));
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
    mut controls: MessageReader<Control>,
    mut playlist: ResMut<Playlist>,
    mut song: ResMut<ActiveSong>,
    mut range: ResMut<Keys>,
    mut judge: ResMut<Judge>,
    mut transport: ResMut<Transport>,
    mut section: ResMut<SectionLoop>,
) {
    // The last song request this frame wins.
    let Some(index) = controls
        .read()
        .filter_map(|c| match *c {
            Control::PreviousSong => playlist.offset(-1),
            Control::NextSong => playlist.offset(1),
            Control::SelectSong(i) => (i < playlist.entries.len()).then_some(i),
            _ => None,
        })
        .last()
    else {
        return;
    };
    if index == playlist.current {
        return;
    }
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
