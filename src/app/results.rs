//! End-of-song results card and personal-best bookkeeping.

use std::path::PathBuf;

use bevy::prelude::*;
use tracing::{info, warn};

use super::transport::{PlayState, Transport};
use super::{ActiveSong, Judge, Phase, palette};
use crate::performance::{Score, Timing};
use crate::practice::weakest_section;
use crate::records::{RecordKey, Records, Submission};

use super::section::LOOP_BARS;

/// Where personal bests live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordStore {
    /// Load from and save to this file.
    File(PathBuf),
    /// Keep bests for this run only (tests, or no data directory).
    Memory,
}

#[derive(Resource, Debug)]
pub struct Bests {
    store: RecordStore,
    pub records: Records,
}

/// The outcome of the last finished, scored run.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct LastResult {
    pub score: Score,
    pub submission: Submission,
    /// Accuracy of each bar (`None` where nothing was yours to play).
    pub bars: Vec<Option<f32>>,
    /// 1-based first bar of the weakest stretch worth drilling.
    pub weakest: Option<u32>,
}

/// One cell of the per-bar accuracy strip on the results card.
#[derive(Component, Debug, Clone, Copy)]
pub struct BarCell(pub usize);

#[derive(Component)]
pub struct ResultsCard;

pub fn plugin(store: RecordStore) -> impl Fn(&mut App) {
    move |app: &mut App| {
        let records = match &store {
            RecordStore::File(path) => Records::load(path).unwrap_or_else(|err| {
                warn!(%err, "could not load personal bests; starting fresh");
                Records::default()
            }),
            RecordStore::Memory => Records::default(),
        };
        app.insert_resource(Bests {
            store: store.clone(),
            records,
        })
        .add_systems(
            Update,
            (record_finish, show_card).chain().in_set(Phase::Present),
        );
    }
}

fn record_finish(
    mut commands: Commands,
    transport: Res<Transport>,
    judge: Res<Judge>,
    song: Res<ActiveSong>,
    mut bests: ResMut<Bests>,
    mut was_finished: Local<bool>,
) {
    let finished = transport.state == PlayState::Finished;
    if finished && !*was_finished && transport.mode.is_scored() {
        let score = judge.0.score().clone();
        let key = RecordKey {
            title: song.0.title.clone(),
            mode: transport.mode,
            hands: judge.0.hands(),
        };
        let submission = bests.records.submit(key, &score);
        info!(?submission, points = score.points, "song result");
        if let (Submission::NewBest { .. }, RecordStore::File(path)) = (submission, &bests.store)
            && let Err(err) = bests.records.save(path)
        {
            warn!(%err, "could not save personal bests");
        }
        let bars = judge.0.bar_accuracy(&song.0);
        let weakest = weakest_section(&bars, usize::from(LOOP_BARS)).map(|i| i as u32 + 1);
        commands.insert_resource(LastResult {
            score,
            submission,
            bars,
            weakest,
        });
    }
    if !finished {
        commands.remove_resource::<LastResult>();
    }
    *was_finished = finished;
}

fn show_card(
    mut commands: Commands,
    result: Option<Res<LastResult>>,
    cards: Query<Entity, With<ResultsCard>>,
    transport: Res<Transport>,
) {
    let Some(result) = result else {
        cards.iter().for_each(|e| commands.entity(e).despawn());
        return;
    };
    if result.is_added() && transport.state == PlayState::Finished {
        cards.iter().for_each(|e| commands.entity(e).despawn());
        spawn_card(&mut commands, &result);
    }
}

fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

fn spawn_card(commands: &mut Commands, result: &LastResult) {
    let score = &result.score;
    let grade = score.grade();
    let (grade_label, grade_color) =
        grade.map_or(("-", palette::HUD_DIM), |g| (g.label(), palette::grade(g)));
    let accuracy = score
        .accuracy()
        .map_or_else(|| "-".to_owned(), |a| format!("{:.1}% accuracy", a * 100.0));
    let banner = match (result.submission, score.is_full_combo()) {
        (Submission::NewBest { previous: Some(p) }, _) => {
            format!("NEW BEST!  +{}", score.points.saturating_sub(p.points))
        }
        (Submission::NewBest { previous: None }, _) => "FIRST CLEAR!".to_owned(),
        (Submission::NotBest { best }, _) => format!("best {:07}", best.points),
        (Submission::Unscored, _) => String::new(),
    };
    let full_combo = if score.is_full_combo() {
        "FULL COMBO"
    } else {
        ""
    };
    let tiers = Timing::ALL
        .iter()
        .map(|t| format!("{} {}", t.label(), score.count(*t)))
        .chain([
            format!("MISS {}", score.missed),
            format!("WRONG {}", score.wrong),
        ])
        .collect::<Vec<_>>()
        .join("   ");

    commands
        .spawn((
            ResultsCard,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(25.0),
                right: Val::Percent(25.0),
                top: Val::Percent(30.0),
                padding: UiRect::all(Val::Px(16.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(palette::BACKGROUND.with_alpha(0.92)),
            BorderColor::all(grade_color),
            GlobalZIndex(10),
        ))
        .with_children(|card| {
            card.spawn(text(grade_label, 96.0, grade_color));
            card.spawn(text(full_combo, 18.0, palette::timing(Timing::Perfect)));
            card.spawn(text(
                format!("{:07}", score.points),
                36.0,
                palette::HUD_TEXT,
            ));
            card.spawn(text(banner, 18.0, palette::timing(Timing::Great)));
            card.spawn(text(
                format!("{accuracy}  |  max combo {}", score.max_combo),
                15.0,
                palette::HUD_TEXT,
            ));
            card.spawn(text(tiers, 13.0, palette::HUD_DIM));
            card.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(2.0),
                margin: UiRect::vertical(Val::Px(4.0)),
                ..default()
            })
            .with_children(|strip| {
                result.bars.iter().enumerate().for_each(|(i, accuracy)| {
                    strip.spawn((
                        BarCell(i),
                        Node {
                            width: Val::Px(10.0),
                            height: Val::Px(14.0),
                            ..default()
                        },
                        BackgroundColor(palette::bar_accuracy(*accuracy)),
                    ));
                });
            });
            let advice = match result.weakest {
                Some(first) => format!(
                    "Weakest: bars {first}-{}  |  F3 to drill them",
                    first + u32::from(LOOP_BARS) - 1
                ),
                None => "Every bar perfect!".to_owned(),
            };
            card.spawn(text(advice, 14.0, palette::HUD_TEXT));
            card.spawn(text(
                "Space to play again | Tab to change mode | Esc to quit",
                12.0,
                palette::HUD_DIM,
            ));
        });
}
