//! Players and their saved settings.
//!
//! Each player has a folder under the data directory,
//! `profiles/<name>/`, holding their bests, achievements, practice log and
//! settings; `current.txt` remembers who played last. A memory-only
//! [`Saves`] (tests, headless and snapshot runs) keeps nothing on disk.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::Resource;
use tracing::{info, warn};

use crate::chord_keys::Inversion;
use crate::error::Error;
use crate::performance::{Hands, PlayMode};

const MAX_NAME: usize = 20;
const FIRST_PLAYER: &str = "Player 1";

/// A player's name, safe to use as a folder name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerName(String);

impl PlayerName {
    /// Letters, digits, spaces, `-`, `_` and `'`; 1 to 20 characters.
    pub fn new(name: &str) -> Result<Self, Error> {
        let name = name.trim();
        let ok =
            !name.is_empty() && name.chars().count() <= MAX_NAME && name.chars().all(Self::allows);
        if ok {
            Ok(Self(name.to_owned()))
        } else {
            Err(Error::InvalidPlayerName(name.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `c` may be typed into a name.
    pub fn allows(c: char) -> bool {
        c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '\'')
    }

    pub const MAX_LEN: usize = MAX_NAME;
}

/// The files kept for each player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFile {
    Bests,
    Achievements,
    Practice,
    Settings,
}

impl SaveFile {
    fn name(self) -> &'static str {
        match self {
            Self::Bests => "bests.tsv",
            Self::Achievements => "achievements.txt",
            Self::Practice => "practice.tsv",
            Self::Settings => "settings.txt",
        }
    }
}

/// Which labels falling bars carry (F1 cycles).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BarLabels {
    #[default]
    Both,
    Fingers,
    Names,
    Off,
}

impl BarLabels {
    const ALL: [Self; 4] = [Self::Both, Self::Fingers, Self::Names, Self::Off];

    fn key(self) -> &'static str {
        match self {
            Self::Both => "both",
            Self::Fingers => "fingers",
            Self::Names => "names",
            Self::Off => "off",
        }
    }
}

/// Everything a player can adjust that is kept between runs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Settings {
    pub mode: PlayMode,
    pub hands: Hands,
    pub sight_reading: bool,
    pub metronome: bool,
    pub labels: BarLabels,
    pub midi_muted: bool,
    /// Title of the song last chosen.
    pub song: Option<String>,
    /// Chord-book challenges done: (page, inversion).
    pub chord_book: BTreeSet<(usize, Inversion)>,
}

fn inversion_key(inversion: Inversion) -> &'static str {
    match inversion {
        Inversion::Root => "root",
        Inversion::First => "first",
        Inversion::Second => "second",
    }
}

impl Settings {
    /// `key=value` lines.
    pub fn to_text(&self) -> String {
        let chord_book = self
            .chord_book
            .iter()
            .map(|(page, inversion)| format!("{page}:{}", inversion_key(*inversion)))
            .collect::<Vec<_>>()
            .join(",");
        [
            format!("mode={}", self.mode.label()),
            format!("hands={}", self.hands.label()),
            format!("sight_reading={}", self.sight_reading),
            format!("metronome={}", self.metronome),
            format!("labels={}", self.labels.key()),
            format!("midi_muted={}", self.midi_muted),
            format!("song={}", self.song.as_deref().unwrap_or("")),
            format!("chord_book={chord_book}"),
        ]
        .into_iter()
        .map(|line| line + "\n")
        .collect()
    }

    /// Read `key=value` lines; anything missing or unreadable keeps its
    /// default.
    pub fn parse(text: &str) -> Self {
        text.lines().filter_map(|line| line.split_once('=')).fold(
            Self::default(),
            |mut s, (key, value)| {
                let flag = value.parse::<bool>().ok();
                match key.trim() {
                    "mode" => {
                        if let Some(m) = PlayMode::ALL.into_iter().find(|m| m.label() == value) {
                            s.mode = m;
                        }
                    }
                    "hands" => {
                        if let Some(h) = Hands::ALL.into_iter().find(|h| h.label() == value) {
                            s.hands = h;
                        }
                    }
                    "labels" => {
                        if let Some(l) = BarLabels::ALL.into_iter().find(|l| l.key() == value) {
                            s.labels = l;
                        }
                    }
                    "sight_reading" => s.sight_reading = flag.unwrap_or(s.sight_reading),
                    "metronome" => s.metronome = flag.unwrap_or(s.metronome),
                    "midi_muted" => s.midi_muted = flag.unwrap_or(s.midi_muted),
                    "song" => s.song = (!value.is_empty()).then(|| value.to_owned()),
                    "chord_book" => {
                        s.chord_book = value
                            .split(',')
                            .filter_map(|entry| {
                                let (page, inversion) = entry.split_once(':')?;
                                let inversion = Inversion::ALL
                                    .into_iter()
                                    .find(|i| inversion_key(*i) == inversion)?;
                                Some((page.parse().ok()?, inversion))
                            })
                            .collect();
                    }
                    _ => {}
                }
                s
            },
        )
    }
}

/// Where players are kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saves {
    root: Option<PathBuf>,
}

impl Saves {
    /// Players kept under `root`.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Some(root.into()),
        }
    }

    /// Nothing kept on disk.
    pub fn memory() -> Self {
        Self { root: None }
    }

    /// The platform's user data directory, if there is one.
    pub fn default_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "keybard").map(|dirs| dirs.data_dir().to_owned())
    }

    fn profiles(root: &Path) -> PathBuf {
        root.join("profiles")
    }

    fn player_dir(&self, player: &PlayerName) -> Option<PathBuf> {
        self.root
            .as_deref()
            .map(|root| Self::profiles(root).join(player.as_str()))
    }

    /// Where one of a player's files lives (none in memory).
    pub fn file(&self, player: &PlayerName, file: SaveFile) -> Option<PathBuf> {
        self.player_dir(player).map(|dir| dir.join(file.name()))
    }

    /// Every player, by name.
    pub fn players(&self) -> Vec<PlayerName> {
        let Some(root) = &self.root else {
            return vec![Self::first()];
        };
        let mut players: Vec<PlayerName> = fs::read_dir(Self::profiles(root))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| PlayerName::new(&entry.file_name().to_string_lossy()).ok())
            .collect();
        players.sort();
        players
    }

    fn first() -> PlayerName {
        PlayerName(FIRST_PLAYER.to_owned())
    }

    /// Who played last; on a first run, a new "Player 1".
    pub fn current(&self) -> PlayerName {
        let Some(root) = &self.root else {
            return Self::first();
        };
        let players = self.players();
        let remembered = fs::read_to_string(root.join("current.txt"))
            .ok()
            .and_then(|name| PlayerName::new(&name).ok())
            .filter(|name| players.contains(name));
        match (remembered, players.first()) {
            (Some(name), _) => name,
            (None, Some(first)) => first.clone(),
            (None, None) => self.create(&Self::first()).unwrap_or_else(|err| {
                warn!(%err, "could not create the first player");
                Self::first()
            }),
        }
    }

    /// Add a player (or reuse one with that name) and make them current.
    pub fn create(&self, player: &PlayerName) -> Result<PlayerName, Error> {
        if let Some(dir) = self.player_dir(player) {
            fs::create_dir_all(&dir).map_err(|source| Error::Io { path: dir, source })?;
            info!(player = player.as_str(), "created player");
        }
        self.choose(player);
        Ok(player.clone())
    }

    /// Remember `player` as the one playing.
    pub fn choose(&self, player: &PlayerName) {
        if let Some(root) = &self.root
            && let Err(err) = fs::write(root.join("current.txt"), player.as_str())
        {
            warn!(%err, "could not remember the current player");
        }
    }

    pub fn load_settings(&self, player: &PlayerName) -> Settings {
        self.file(player, SaveFile::Settings)
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|text| Settings::parse(&text))
            .unwrap_or_default()
    }

    pub fn save_settings(&self, player: &PlayerName, settings: &Settings) {
        let Some(path) = self.file(player, SaveFile::Settings) else {
            return;
        };
        let written = path
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| fs::write(&path, settings.to_text()));
        if let Err(err) = written {
            warn!(%err, path = %path.display(), "could not save settings");
        }
    }
}
