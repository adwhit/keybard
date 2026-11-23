//! End-of-song results card and personal-best bookkeeping.

use std::path::PathBuf;

use bevy::prelude::*;
use tracing::{info, warn};

use super::transport::{PlayState, Transport};
use super::{ActiveSong, Judge, Phase, palette};
use crate::achievements::{Achievement, Achievements, RunSummary};
use crate::performance::{Insights, Score, Timing};
use crate::practice::weakest_section;
use crate::records::{RecordKey, Records, Submission};

use super::falling::View;
use super::practice::Practice;
use super::section::LOOP_BARS;
use super::songs::Playlist;
use super::songs::SongScoped;
use crate::challenge::for_day;

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

/// Unlocked achievements and where they're kept.
#[derive(Resource, Debug)]
pub struct Unlocked {
    store: RecordStore,
    pub achievements: Achievements,
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
    pub insights: Insights,
    pub new_achievements: Vec<Achievement>,
    /// This run met today's challenge.
    pub challenge_met: bool,
}

/// One cell of the per-bar accuracy strip on the results card.
#[derive(Component, Debug, Clone, Copy)]
pub struct BarCell(pub usize);

#[derive(Component)]
pub struct ResultsCard;

/// The F5 overlay listing every achievement.
#[derive(Component)]
pub struct AchievementGallery;

pub fn plugin(store: RecordStore) -> impl Fn(&mut App) {
    move |app: &mut App| {
        let records = match &store {
            RecordStore::File(path) => Records::load(path).unwrap_or_else(|err| {
                warn!(%err, "could not load personal bests; starting fresh");
                Records::default()
            }),
            RecordStore::Memory => Records::default(),
        };
        // Achievements live next to the bests.
        let achievement_store = match &store {
            RecordStore::File(path) => RecordStore::File(path.with_file_name("achievements.txt")),
            RecordStore::Memory => RecordStore::Memory,
        };
        let achievements = match &achievement_store {
            RecordStore::File(path) => Achievements::load(path).unwrap_or_else(|err| {
                warn!(%err, "could not load achievements; starting fresh");
                Achievements::default()
            }),
            RecordStore::Memory => Achievements::default(),
        };
        app.insert_resource(Unlocked {
            store: achievement_store,
            achievements,
        })
        .insert_resource(Bests {
            store: store.clone(),
            records,
        })
        .add_systems(Startup, spawn_gallery)
        .add_systems(Update, toggle_gallery.in_set(Phase::Input))
        .add_systems(
            Update,
            (record_finish, show_card, refresh_gallery)
                .chain()
                .in_set(Phase::Present),
        );
    }
}

#[expect(clippy::too_many_arguments, reason = "Bevy system parameters")]
fn record_finish(
    mut commands: Commands,
    transport: Res<Transport>,
    judge: Res<Judge>,
    song: Res<ActiveSong>,
    mut bests: ResMut<Bests>,
    mut unlocked: ResMut<Unlocked>,
    practice: Res<Practice>,
    view: Res<View>,
    playlist: Res<Playlist>,
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
        let run = RunSummary {
            score: score.clone(),
            mode: transport.mode,
            hands: judge.0.hands(),
            speed: transport.speed.get(),
            streak: practice.streak(),
            minutes_today: practice.minutes_today(),
            songs_with_bests: bests.records.song_count(),
            sight_reading: *view == View::SightReading,
        };
        let new_achievements = unlocked.achievements.unlock(&run);
        if !new_achievements.is_empty() {
            info!(?new_achievements, "achievements unlocked");
            if let RecordStore::File(path) = &unlocked.store
                && let Err(err) = unlocked.achievements.save(path)
            {
                warn!(%err, "could not save achievements");
            }
        }
        let score_grade = run.score.grade();
        let bars = judge.0.bar_accuracy(&song.0);
        let weakest = weakest_section(&bars, usize::from(LOOP_BARS)).map(|i| i as u32 + 1);
        commands.insert_resource(LastResult {
            score,
            submission,
            bars,
            weakest,
            insights: judge.0.insights(&song.0),
            new_achievements,
            challenge_met: !playlist.entries().is_empty()
                && for_day(practice.today, playlist.entries().len()).is_met_by(
                    playlist.current(),
                    transport.mode,
                    score_grade,
                ),
        });
    }
    if !finished {
        commands.remove_resource::<LastResult>();
    }
    *was_finished = finished;
}

fn spawn_gallery(mut commands: Commands) {
    commands.spawn((
        AchievementGallery,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(palette::HUD_TEXT),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(18.0),
            top: Val::Percent(30.0),
            padding: UiRect::all(Val::Px(14.0)),
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(palette::BACKGROUND.with_alpha(0.97)),
        BorderColor::all(palette::timing(Timing::Perfect)),
        GlobalZIndex(20),
        Visibility::Hidden,
    ));
}

fn toggle_gallery(
    keys: Res<ButtonInput<KeyCode>>,
    mut gallery: Single<&mut Visibility, With<AchievementGallery>>,
) {
    if keys.just_pressed(KeyCode::F5) {
        **gallery = match **gallery {
            Visibility::Hidden => Visibility::Inherited,
            _ => Visibility::Hidden,
        };
    }
}

fn refresh_gallery(
    unlocked: Res<Unlocked>,
    mut gallery: Single<(&mut Text, &Visibility), With<AchievementGallery>>,
) {
    let (text, visibility) = &mut *gallery;
    if **visibility != Visibility::Hidden {
        let wanted = unlocked.achievements.gallery();
        if text.0 != wanted {
            text.0 = wanted;
        }
    }
}

fn show_card(
    mut commands: Commands,
    result: Option<Res<LastResult>>,
    cards: Query<Entity, With<ResultsCard>>,
    transport: Res<Transport>,
    practice: Res<Practice>,
    unlocked: Res<Unlocked>,
) {
    let Some(result) = result else {
        cards.iter().for_each(|e| commands.entity(e).despawn());
        return;
    };
    if result.is_added() && transport.state == PlayState::Finished {
        cards.iter().for_each(|e| commands.entity(e).despawn());
        spawn_card(&mut commands, &result, &practice, &unlocked);
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

fn spawn_card(
    commands: &mut Commands,
    result: &LastResult,
    practice: &Practice,
    unlocked: &Unlocked,
) {
    let score = &result.score;
    let grade = score.grade();
    let (grade_label, grade_color) =
        grade.map_or(("-", palette::HUD_DIM), |g| (g.label(), palette::grade(g)));
    let accuracy = score
        .accuracy()
        .map_or_else(|| "-".to_owned(), |a| format!("{:.1}% accuracy", a * 100.0));
    let banner = match (result.submission, score.is_full_combo()) {
        _ if score.hits() == 0 => "Keep practising!".to_owned(),
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
            SongScoped,
            ResultsCard,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(25.0),
                right: Val::Percent(25.0),
                // Near the top: the notation above is empty once the song ends.
                top: Val::Percent(3.0),
                padding: UiRect::all(Val::Px(16.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(palette::BACKGROUND.with_alpha(0.98)),
            BorderColor::all(grade_color),
            GlobalZIndex(10),
        ))
        .with_children(|card| {
            card.spawn(text(grade_label, 96.0, grade_color));
            card.spawn(text(full_combo, 18.0, palette::timing(Timing::Perfect)));
            if result.challenge_met {
                card.spawn(text("DAILY CHALLENGE COMPLETE!", 18.0, palette::FIRE));
            }
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
            result.insights.summary().into_iter().for_each(|line| {
                card.spawn(text(line, 13.0, palette::timing(Timing::Great)));
            });
            card.spawn(text(
                format!(
                    "{} | {} min practised today",
                    practice.streak_label(),
                    practice.minutes_today()
                ),
                13.0,
                palette::FIRE,
            ));
            result.new_achievements.iter().for_each(|a| {
                card.spawn(text(
                    format!("UNLOCKED  {}  -  {}", a.title(), a.description()),
                    14.0,
                    palette::timing(Timing::Perfect),
                ));
            });
            card.spawn(text(
                format!(
                    "Achievements {}/{}",
                    unlocked.achievements.count(),
                    Achievement::ALL.len()
                ),
                12.0,
                palette::HUD_DIM,
            ));
            card.spawn(text(
                "Space to play again | Tab to change mode | Esc to quit",
                12.0,
                palette::HUD_DIM,
            ));
        });
}
