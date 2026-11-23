//! Personal bests per song and play mode, persisted as a small TSV file.

use std::collections::BTreeMap;
use std::path::Path;

use tracing::{debug, warn};

use crate::error::Error;
use crate::performance::{PlayMode, Score};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecordKey {
    pub title: String,
    pub mode: PlayMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Best {
    pub points: u32,
    pub accuracy: f32,
    pub max_combo: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Submission {
    NewBest {
        previous: Option<Best>,
    },
    NotBest {
        best: Best,
    },
    /// Nothing was judged, so there is nothing to record.
    Unscored,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Records {
    bests: BTreeMap<RecordKey, Best>,
}

fn mode_from_label(label: &str) -> Option<PlayMode> {
    PlayMode::ALL.into_iter().find(|m| m.label() == label)
}

/// Titles may contain anything; keep the TSV one-record-per-line.
fn escape(title: &str) -> String {
    title
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn parse_line(line: &str) -> Option<(RecordKey, Best)> {
    let mut fields = line.splitn(5, '\t');
    let mode = mode_from_label(fields.next()?)?;
    let points = fields.next()?.parse().ok()?;
    let accuracy = fields.next()?.parse().ok()?;
    let max_combo = fields.next()?.parse().ok()?;
    let title = unescape(fields.next()?);
    Some((
        RecordKey { title, mode },
        Best {
            points,
            accuracy,
            max_combo,
        },
    ))
}

impl Records {
    pub fn best(&self, key: &RecordKey) -> Option<Best> {
        self.bests.get(key).copied()
    }

    pub fn submit(&mut self, key: RecordKey, score: &Score) -> Submission {
        let Some(accuracy) = score.accuracy() else {
            return Submission::Unscored;
        };
        let candidate = Best {
            points: score.points,
            accuracy,
            max_combo: score.max_combo,
        };
        match self.bests.get(&key).copied() {
            Some(best) if best.points >= candidate.points => Submission::NotBest { best },
            previous => {
                self.bests.insert(key, candidate);
                Submission::NewBest { previous }
            }
        }
    }

    pub fn load(path: &Path) -> Result<Self, Error> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(Error::Io {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        let bests = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|line| {
                let parsed = parse_line(line);
                if parsed.is_none() {
                    warn!(line, "skipping unreadable record");
                }
                parsed
            })
            .collect();
        Ok(Self { bests })
    }

    pub fn save(&self, path: &Path) -> Result<(), Error> {
        let io = |source| Error::Io {
            path: path.to_owned(),
            source,
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let text: String = self
            .bests
            .iter()
            .map(|(key, best)| {
                format!(
                    "{}\t{}\t{}\t{}\t{}\n",
                    key.mode.label(),
                    best.points,
                    best.accuracy,
                    best.max_combo,
                    escape(&key.title)
                )
            })
            .collect();
        std::fs::write(path, text).map_err(io)?;
        debug!(path = %path.display(), count = self.bests.len(), "saved records");
        Ok(())
    }
}
