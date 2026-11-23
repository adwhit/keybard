//! Practice time per day and the daily streak, persisted as TSV.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::warn;

use crate::error::Error;
use crate::song::Seconds;

/// A calendar day (UTC), counted from the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(pub u32);

const SECONDS_PER_DAY: u64 = 86_400;

impl Day {
    pub fn from_unix_seconds(seconds: u64) -> Self {
        Self(u32::try_from(seconds / SECONDS_PER_DAY).unwrap_or(u32::MAX))
    }

    pub fn today() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Self::from_unix_seconds(now)
    }

    fn previous(self) -> Option<Self> {
        self.0.checked_sub(1).map(Self)
    }
}

/// A day only counts towards the streak with at least this much playing.
pub const MIN_DAILY_PRACTICE: Seconds = Seconds(60.0);

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PracticeLog {
    days: BTreeMap<Day, Seconds>,
}

impl PracticeLog {
    pub fn add(&mut self, day: Day, time: Seconds) {
        let total = self.days.entry(day).or_default();
        *total = *total + time;
    }

    pub fn on(&self, day: Day) -> Seconds {
        self.days.get(&day).copied().unwrap_or_default()
    }

    fn counts(&self, day: Day) -> bool {
        self.on(day) >= MIN_DAILY_PRACTICE
    }

    /// Consecutive qualifying days ending today, or ending yesterday if today
    /// doesn't qualify yet (there's still time to keep the streak).
    pub fn streak(&self, today: Day) -> u32 {
        let end = if self.counts(today) {
            Some(today)
        } else {
            today.previous()
        };
        std::iter::successors(end, |d| d.previous())
            .take_while(|d| self.counts(*d))
            .count() as u32
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
        let days = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|line| {
                let parsed = line
                    .split_once('\t')
                    .and_then(|(d, s)| Some((Day(d.parse().ok()?), Seconds(s.parse().ok()?))));
                if parsed.is_none() {
                    warn!(line, "skipping unreadable practice entry");
                }
                parsed
            })
            .collect();
        Ok(Self { days })
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
            .days
            .iter()
            .map(|(day, time)| format!("{}\t{}\n", day.0, time.0))
            .collect();
        std::fs::write(path, text).map_err(io)
    }
}
