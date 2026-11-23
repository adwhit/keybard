//! Logs time spent playing per day and keeps the daily streak.

use bevy::prelude::*;
use tracing::warn;

use super::effects::popup;
use super::results::RecordStore;
use super::transport::{PlayState, Transport, advance};
use super::{Phase, palette};
use crate::song::Seconds;
use crate::streak::{Day, PracticeLog};

/// Save at least this often while playing.
const SAVE_EVERY: Seconds = Seconds(30.0);

#[derive(Resource, Debug)]
pub struct Practice {
    store: RecordStore,
    pub log: PracticeLog,
    pub today: Day,
    unsaved: Seconds,
}

impl Practice {
    pub fn streak(&self) -> u32 {
        self.log.streak(self.today)
    }

    /// "3-day streak", or "No streak yet".
    pub fn streak_label(&self) -> String {
        match self.streak() {
            0 => "No streak yet".to_owned(),
            n => format!("{n}-day streak"),
        }
    }

    pub fn minutes_today(&self) -> u32 {
        (self.log.on(self.today).0 / 60.0) as u32
    }

    fn save(&mut self) {
        if let RecordStore::File(path) = &self.store
            && let Err(err) = self.log.save(path)
        {
            warn!(%err, "could not save practice log");
        }
        self.unsaved = Seconds(0.0);
    }
}

pub fn plugin(store: RecordStore) -> impl Fn(&mut App) {
    move |app: &mut App| {
        let log = match &store {
            RecordStore::File(path) => PracticeLog::load(path).unwrap_or_else(|err| {
                warn!(%err, "could not load practice log; starting fresh");
                PracticeLog::default()
            }),
            RecordStore::Memory => PracticeLog::default(),
        };
        app.insert_resource(Practice {
            store: store.clone(),
            log,
            today: Day::today(),
            unsaved: Seconds(0.0),
        })
        .add_systems(Update, accumulate.in_set(Phase::Simulate).after(advance));
    }
}

fn accumulate(
    mut commands: Commands,
    mut practice: ResMut<Practice>,
    transport: Res<Transport>,
    time: Res<Time>,
    mut was_playing: Local<bool>,
) {
    let playing = transport.state == PlayState::Playing;
    if playing {
        // Wall-clock time: slowed-down practice counts in full.
        let today = practice.today;
        let dt = Seconds(time.delta_secs());
        let before = practice.streak();
        practice.log.add(today, dt);
        let after = practice.streak();
        // Today just started counting: celebrate the (extended) streak.
        if after > before {
            popup(
                &mut commands,
                &format!("{after}-DAY STREAK!"),
                palette::FIRE,
                Vec2::new(0.0, 150.0),
                30.0,
                1.0,
            );
        }
        practice.unsaved = practice.unsaved + dt;
    }
    let stopped = *was_playing && !playing;
    if practice.unsaved >= SAVE_EVERY || (stopped && practice.unsaved.0 > 0.0) {
        practice.save();
    }
    *was_playing = playing;
}
