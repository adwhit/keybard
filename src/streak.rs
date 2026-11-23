//! Practice time per day and the daily streak, persisted as TSV.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::warn;

use crate::error::Error;
use crate::song::Seconds;

/// A local calendar day, counted from the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(pub u32);

const SECONDS_PER_DAY: u64 = 86_400;

impl Day {
    pub fn from_unix_seconds(seconds: u64) -> Self {
        Self(u32::try_from(seconds / SECONDS_PER_DAY).unwrap_or(u32::MAX))
    }

    /// The local calendar day of a Unix instant, given the local offset
    /// from UTC in seconds (negative west of Greenwich).
    pub fn local(unix_seconds: u64, utc_offset_seconds: i32) -> Self {
        let local = unix_seconds.saturating_add_signed(i64::from(utc_offset_seconds));
        Self::from_unix_seconds(local)
    }

    /// Today, by the local clock.
    pub fn today() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let offset = chrono::Local::now().offset().local_minus_utc();
        Self::local(now, offset)
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
    /// Days on which the daily challenge was completed.
    challenges: BTreeSet<Day>,
}

impl PracticeLog {
    pub fn add(&mut self, day: Day, time: Seconds) {
        let total = self.days.entry(day).or_default();
        *total = *total + time;
    }

    pub fn on(&self, day: Day) -> Seconds {
        self.days.get(&day).copied().unwrap_or_default()
    }

    /// Record today's challenge as done; true the first time each day.
    pub fn complete_challenge(&mut self, day: Day) -> bool {
        self.challenges.insert(day)
    }

    pub fn challenge_done(&self, day: Day) -> bool {
        self.challenges.contains(&day)
    }

    /// Consecutive days of completed challenges, alive until today ends.
    pub fn challenge_streak(&self, today: Day) -> u32 {
        run_length(today, |d| self.challenge_done(d))
    }

    fn counts(&self, day: Day) -> bool {
        self.on(day) >= MIN_DAILY_PRACTICE
    }

    /// Consecutive qualifying days ending today, or ending yesterday if today
    /// doesn't qualify yet (there's still time to keep the streak).
    pub fn streak(&self, today: Day) -> u32 {
        run_length(today, |d| self.counts(d))
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
        // Each line: day, seconds practised, and "challenge" if completed.
        let entries: Vec<(Day, Seconds, bool)> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|line| {
                let mut fields = line.split('\t');
                let mut field = || fields.next();
                let parsed = field()
                    .and_then(|d| d.parse().ok())
                    .zip(field().and_then(|s| s.parse().ok()))
                    .map(|(d, s)| (Day(d), Seconds(s), field() == Some("challenge")));
                if parsed.is_none() {
                    warn!(line, "skipping unreadable practice entry");
                }
                parsed
            })
            .collect();
        Ok(Self {
            days: entries
                .iter()
                .filter(|(_, t, _)| t.0 > 0.0)
                .map(|(d, t, _)| (*d, *t))
                .collect(),
            challenges: entries.iter().filter(|e| e.2).map(|e| e.0).collect(),
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), Error> {
        let io = |source| Error::Io {
            path: path.to_owned(),
            source,
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let all_days: BTreeSet<Day> = self.days.keys().chain(&self.challenges).copied().collect();
        let text: String = all_days
            .into_iter()
            .map(|day| {
                let flag = if self.challenge_done(day) {
                    "\tchallenge"
                } else {
                    ""
                };
                format!("{}\t{}{flag}\n", day.0, self.on(day).0)
            })
            .collect();
        std::fs::write(path, text).map_err(io)
    }
}

/// Consecutive days satisfying `qualifies`, ending today, or yesterday if
/// today doesn't qualify yet.
fn run_length(today: Day, qualifies: impl Fn(Day) -> bool) -> u32 {
    let end = if qualifies(today) {
        Some(today)
    } else {
        today.previous()
    };
    std::iter::successors(end, |d| d.previous())
        .take_while(|d| qualifies(*d))
        .count() as u32
}
