//! End-of-song results card and personal-best bookkeeping.

use std::path::PathBuf;

use bevy::prelude::*;
use tracing::{info, warn};

use super::controls::Control;
use super::transport::{PlayState, Transport};
use super::widgets::button;
use super::{ActiveSong, Judge, Phase, palette};
use crate::achievements::{Achievement, Achievements, RunSummary};
use crate::performance::{Insights, PlayMode, Score, Timing};
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
    pub challenge: ChallengeOutcome,
}

/// How a finished run relates to today's challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeOutcome {
    NotMet,
    /// Completed for the first time today; the challenge streak so far.
    Completed {
        streak: u32,
    },
    /// Met again after already completing it today.
    AlreadyDone,
}

/// One cell of the per-bar accuracy strip on the results card.
#[derive(Component, Debug, Clone, Copy)]
pub struct BarCell(pub usize);

#[derive(Component)]
pub struct ResultsCard;

/// Shown when a song finishes in Listen mode (nothing to score), so it
/// doesn't just stop: what to do next.
#[derive(Component)]
pub struct EndCard;

/// The F5 overlay listing every achievement.
#[derive(Component)]
pub struct AchievementGallery;

impl Bests {
    /// Load the bests kept in `store`.
    pub fn open(store: RecordStore) -> Self {
        let records = match &store {
            RecordStore::File(path) => Records::load(path).unwrap_or_else(|err| {
                warn!(%err, "could not load personal bests; starting fresh");
                Records::default()
            }),
            RecordStore::Memory => Records::default(),
        };
        Self { store, records }
    }
}

impl Unlocked {
    /// Load the achievements kept in `store`.
    pub fn open(store: RecordStore) -> Self {
        let achievements = match &store {
            RecordStore::File(path) => Achievements::load(path).unwrap_or_else(|err| {
                warn!(%err, "could not load achievements; starting fresh");
                Achievements::default()
            }),
            RecordStore::Memory => Achievements::default(),
        };
        Self {
            store,
            achievements,
        }
    }
}

pub fn plugin(bests: RecordStore, achievements: RecordStore) -> impl Fn(&mut App) {
    move |app: &mut App| {
        app.insert_resource(Unlocked::open(achievements.clone()))
            .insert_resource(Bests::open(bests.clone()))
            .add_systems(Startup, spawn_gallery)
            .add_systems(Update, toggle_gallery.in_set(Phase::Input))
            .add_systems(
                Update,
                (record_finish, show_card, show_end_card, refresh_gallery)
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
    mut practice: ResMut<Practice>,
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
        let met = !playlist.entries().is_empty()
            && for_day(practice.today, playlist.entries().len()).is_met_by(
                playlist.current(),
                transport.mode,
                run.score.grade(),
            );
        let today = practice.today;
        let challenge = match (met, met && practice.log.complete_challenge(today)) {
            (false, _) => ChallengeOutcome::NotMet,
            (true, true) => {
                practice.persist();
                ChallengeOutcome::Completed {
                    streak: practice.log.challenge_streak(today),
                }
            }
            (true, false) => ChallengeOutcome::AlreadyDone,
        };
        let bars = judge.0.bar_accuracy(&song.0);
        let weakest = weakest_section(&bars, usize::from(LOOP_BARS)).map(|i| i as u32 + 1);
        commands.insert_resource(LastResult {
            score,
            submission,
            bars,
            weakest,
            insights: judge.0.insights(&song.0),
            new_achievements,
            challenge,
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
    mut controls: MessageReader<Control>,
    mut gallery: Single<&mut Visibility, With<AchievementGallery>>,
) {
    controls
        .read()
        .filter(|c| **c == Control::ToggleAchievements)
        .for_each(|_| {
            **gallery = match **gallery {
                Visibility::Hidden => Visibility::Inherited,
                _ => Visibility::Hidden,
            };
        });
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
    playlist: Res<Playlist>,
) {
    let Some(result) = result else {
        cards.iter().for_each(|e| commands.entity(e).despawn());
        return;
    };
    if result.is_added() && transport.state == PlayState::Finished {
        cards.iter().for_each(|e| commands.entity(e).despawn());
        spawn_card(
            &mut commands,
            &result,
            &practice,
            &unlocked,
            playlist.entries().len() > 1,
        );
    }
}

fn show_end_card(
    mut commands: Commands,
    cards: Query<Entity, With<EndCard>>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    playlist: Res<Playlist>,
) {
    let wanted = transport.state == PlayState::Finished && !transport.mode.is_scored();
    match (wanted, cards.is_empty()) {
        (true, true) => {
            commands
                .spawn((
                    SongScoped,
                    EndCard,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(25.0),
                        right: Val::Percent(25.0),
                        top: Val::Percent(30.0),
                        padding: UiRect::all(Val::Px(20.0)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(8.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(palette::BACKGROUND),
                    BorderColor::all(palette::timing(Timing::Great)),
                    GlobalZIndex(10),
                ))
                .with_children(|card| {
                    card.spawn(text("THE END", 40.0, palette::timing(Timing::Great)));
                    card.spawn(text(song.0.title.clone(), 18.0, palette::HUD_TEXT));
                    card.spawn(text(
                        "Your turn? Play it and see how you score.",
                        14.0,
                        palette::HUD_DIM,
                    ));
                    card.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(10.0),
                        margin: UiRect::top(Val::Px(8.0)),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn(button(Control::SetMode(PlayMode::Play), "Play it", 16.0));
                        row.spawn(button(Control::TogglePlay, "Listen again", 16.0));
                        if playlist.entries().len() > 1 {
                            row.spawn(button(Control::NextSong, "Next song", 16.0));
                        }
                        row.spawn(button(Control::OpenMenu, "Start screen", 16.0));
                    });
                });
        }
        (false, false) => cards.iter().for_each(|e| commands.entity(e).despawn()),
        _ => {}
    }
}

fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        // Headings and big numbers in bold, the rest regular.
        if size >= 20.0 {
            super::fonts::bold(size)
        } else {
            super::fonts::regular(size)
        },
        TextColor(color),
    )
}

fn spawn_card(
    commands: &mut Commands,
    result: &LastResult,
    practice: &Practice,
    unlocked: &Unlocked,
    has_next_song: bool,
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
        (Submission::NotBest { best }, _) => format!("best {}", best.points),
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
            BackgroundColor(palette::BACKGROUND),
            BorderColor::all(grade_color),
            GlobalZIndex(10),
        ))
        .with_children(|card| {
            card.spawn(text(grade_label, 96.0, grade_color));
            card.spawn(text(full_combo, 18.0, palette::timing(Timing::Perfect)));
            match result.challenge {
                ChallengeOutcome::Completed { streak } => {
                    card.spawn(text("DAILY CHALLENGE COMPLETE!", 18.0, palette::FIRE));
                    if streak > 1 {
                        card.spawn(text(
                            format!("{streak}-day challenge streak"),
                            14.0,
                            palette::FIRE,
                        ));
                    }
                }
                ChallengeOutcome::AlreadyDone => {
                    card.spawn(text(
                        "Daily challenge already done today",
                        13.0,
                        palette::HUD_DIM,
                    ));
                }
                ChallengeOutcome::NotMet => {}
            }
            card.spawn(text(format!("{}", score.points), 36.0, palette::HUD_TEXT));
            card.spawn(text(banner, 18.0, palette::timing(Timing::Great)));
            card.spawn(text(
                format!("{accuracy}  ·  max combo {}", score.max_combo),
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
                    "Weakest: bars {first}-{}  ·  F3 to drill them",
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
                    "{} · {} min practised today",
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
            card.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            })
            .with_children(|row| {
                row.spawn(button(Control::TogglePlay, "Play again", 16.0));
                if result.weakest.is_some() {
                    row.spawn(button(Control::ToggleLoop, "Drill weakest bars", 16.0));
                }
                if has_next_song {
                    row.spawn(button(Control::NextSong, "Next song", 16.0));
                }
                row.spawn(button(Control::OpenMenu, "Start screen", 16.0));
            });
            card.spawn(text(
                "Play a button's chord to press it | Space plays again | Esc start screen",
                12.0,
                palette::HUD_DIM,
            ));
        });
}
