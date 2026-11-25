//! Heads-up display: song info, score, combo, timing readout and a progress
//! strip that records every judgement as a coloured tick.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use super::effects::popup;
use super::falling::View;
use super::input::{MidiEcho, MidiStatus};
use super::practice::Practice;
use super::results::Bests;
use super::section::SectionLoop;
use super::songs::SongScoped;
use super::transport::{Metronome, PlayState, Transport};
use super::widgets::capitalise;
use super::{ActiveSong, Feedback, Judge, Phase, fonts, palette};
use crate::layout;
use crate::performance::{Judgement, Timing};
use crate::records::{self, Best, Chase, RecordKey};

/// The text lines of the song card.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum InfoLine {
    Title,
    Clock,
    /// Streak, practice today and the MIDI keyboard.
    Meta,
    /// The loop being drilled and how the last pass went.
    Loop,
}

/// Tags under the title: mode, hands, speed and whatever else is on.
#[derive(Component)]
struct Chips;

/// The card on the right: score, multiplier, accuracy and the chase.
#[derive(Component)]
struct ScoreCard;

/// The lines of the score card.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum ScoreLine {
    Points,
    Multiplier,
    Accuracy,
    /// How far behind (or ahead of) the personal best.
    Chase,
}

#[derive(Component)]
struct ComboText;

#[derive(Component)]
struct TimingText;

#[derive(Component)]
struct ProgressFill;

/// A quiet reminder of how to pause, in the corner while playing.
#[derive(Component)]
pub struct HelpText;

/// A judgement tick on the progress strip.
#[derive(Component)]
pub struct ProgressTick;

/// Values the HUD animates towards.
#[derive(Resource, Debug, Default)]
struct Display {
    points: f32,
    combo_pulse: f32,
    last: Option<Judgement>,
    last_age: f32,
}

const HELP: &str = "Hold C C# D to pause";

const STRIP_HEIGHT: f32 = 5.0;
const STRIP_Z: f32 = 6.0;
const COMBO_Z: f32 = 0.5;
/// Combos shorter than this aren't worth shouting about.
const MIN_SHOWN_COMBO: u32 = 3;
/// Where the HUD cards sit: just under the notation.
const CARD_TOP: Val = Val::Percent(31.5);

pub fn plugin(app: &mut App) {
    app.init_resource::<Display>()
        .add_systems(Startup, (spawn_hud, spawn_strip))
        .add_systems(
            Update,
            (
                update_info,
                update_chips,
                update_score,
                best_beaten,
                update_combo,
                update_timing,
                update_strip,
                tick_strip,
                show_help,
            )
                .in_set(Phase::Present),
        );
}

/// A translucent card floating over the falling bars.
fn card(node: Node) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..node
        },
        BackgroundColor(palette::PANEL.with_alpha(0.78)),
    )
}

fn line(marker: impl Component, font: TextFont, color: Color) -> impl Bundle {
    (marker, Text::new(""), font, TextColor(color))
}

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        card(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(10.0),
            top: CARD_TOP,
            max_width: Val::Percent(46.0),
            ..default()
        }),
        children![
            line(InfoLine::Title, fonts::bold(18.0), palette::HUD_TEXT),
            (
                Chips,
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(5.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
            ),
            line(InfoLine::Clock, fonts::numbers(14.0), palette::HUD_TEXT),
            line(InfoLine::Meta, fonts::regular(12.0), palette::HUD_DIM),
            line(
                InfoLine::Loop,
                fonts::regular(12.0),
                palette::LOOP_REGION.with_alpha(1.0)
            ),
        ],
    ));
    commands.spawn((
        ScoreCard,
        card(Node {
            position_type: PositionType::Absolute,
            right: Val::Px(10.0),
            top: CARD_TOP,
            align_items: AlignItems::End,
            min_width: Val::Px(170.0),
            ..default()
        }),
        children![
            line(ScoreLine::Points, fonts::numbers(34.0), palette::HUD_TEXT),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(10.0),
                    ..default()
                },
                children![
                    (
                        Node {
                            padding: UiRect::axes(Val::Px(7.0), Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(9.0)),
                            ..default()
                        },
                        BackgroundColor(palette::BUTTON),
                        children![line(
                            ScoreLine::Multiplier,
                            fonts::numbers(14.0),
                            palette::HUD_TEXT
                        )],
                    ),
                    line(ScoreLine::Accuracy, fonts::numbers(16.0), palette::HUD_TEXT),
                ],
            ),
            line(ScoreLine::Chase, fonts::regular(12.0), palette::HUD_DIM),
        ],
    ));
    // Bottom right of the falling area, just above the keys.
    commands.spawn((
        HelpText,
        Text::new(HELP),
        fonts::regular(11.0),
        TextColor(palette::HUD_DIM.with_alpha(0.7)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(10.0),
            bottom: Val::Percent(layout::WHITE_KEY_HEIGHT / layout::CANVAS_HEIGHT * 100.0 + 0.8),
            ..default()
        },
    ));
    commands.spawn((
        ComboText,
        Text2d::new(""),
        fonts::numbers(64.0),
        TextColor(palette::HUD_TEXT.with_alpha(0.0)),
        Transform::from_xyz(0.0, 70.0, COMBO_Z),
    ));
    commands.spawn((
        TimingText,
        Text2d::new(""),
        fonts::regular(14.0),
        TextColor(palette::HUD_DIM),
        Transform::from_xyz(0.0, 18.0, COMBO_Z),
    ));
}

fn spawn_strip(mut commands: Commands) {
    commands.spawn((
        Sprite::from_color(
            palette::PROGRESS_TRACK,
            Vec2::new(layout::CANVAS_WIDTH, STRIP_HEIGHT),
        ),
        Anchor::TOP_LEFT,
        Transform::from_xyz(layout::LEFT, layout::NOTATION_BOTTOM, STRIP_Z),
    ));
    commands.spawn((
        ProgressFill,
        Sprite::from_color(
            palette::PROGRESS_FILL.with_alpha(0.45),
            Vec2::new(1.0, STRIP_HEIGHT),
        ),
        Anchor::TOP_LEFT,
        Transform::from_xyz(layout::LEFT, layout::NOTATION_BOTTOM, STRIP_Z + 0.1),
    ));
}

fn set(text: &mut Text, wanted: String) {
    if text.0 != wanted {
        text.0 = wanted;
    }
}

fn clock(seconds: f32) -> String {
    format!("{}:{:02}", (seconds / 60.0) as u32, (seconds % 60.0) as u32)
}

/// Song title, clock, practice and MIDI, and the loop.
fn update_info(
    mut lines: Query<(&InfoLine, &mut Text)>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    section: Res<SectionLoop>,
    practice: Res<Practice>,
    midi: Res<MidiStatus>,
) {
    let looping = || {
        section.0.as_ref().map_or_else(String::new, |looping| {
            let tempo = song.0.tempo;
            let first = looping.region.first_bar(tempo);
            let last = first + looping.region.bars(tempo) - 1;
            let mut line = format!("Looping bars {first}-{last} · pass {}", looping.passes + 1);
            if transport.mode.is_scored() {
                line.push_str(&format!(" · {} clean", looping.clean_passes));
                if let Some(pass) = looping.last_pass {
                    line.push_str(&format!(
                        " · last: {} hit, {} missed, {} wrong",
                        pass.hits, pass.missed, pass.wrong
                    ));
                }
            }
            line
        })
    };
    lines.iter_mut().for_each(|(line, mut text)| {
        let wanted = match line {
            InfoLine::Title => song.0.title.clone(),
            InfoLine::Clock => format!(
                "{} / {}",
                clock(transport.time.0.max(0.0)),
                clock(song.0.duration().0)
            ),
            InfoLine::Meta => format!(
                "{} · {} min today · {}",
                practice.streak_label(),
                practice.minutes_today(),
                midi.label()
            ),
            InfoLine::Loop => looping(),
        };
        set(&mut text, wanted);
    });
}

/// A rounded tag under the song title.
fn chip(label: String, color: Color) -> impl Bundle {
    (
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(color.with_alpha(0.18)),
        BorderColor::all(color.with_alpha(0.6)),
        children![(Text::new(label), fonts::bold(11.0), TextColor(color))],
    )
}

/// Rebuild the chips whenever what they say changes.
#[expect(clippy::too_many_arguments, reason = "Bevy system parameters")]
fn update_chips(
    mut commands: Commands,
    chips: Single<Entity, With<Chips>>,
    transport: Res<Transport>,
    judge: Res<Judge>,
    metronome: Res<Metronome>,
    view: Res<View>,
    echo: Res<MidiEcho>,
    mut shown: Local<Vec<(String, Color)>>,
) {
    let accent = palette::timing(Timing::Perfect);
    let plain = palette::HUD_TEXT;
    let wanted: Vec<(String, Color)> = [
        Some((transport.mode.label().to_uppercase(), accent)),
        Some((capitalise(judge.0.hands().label()), plain)),
        Some((format!("{:.0}%", transport.speed.get() * 100.0), plain)),
        (*metronome == Metronome::On).then(|| ("Metronome".to_owned(), plain)),
        (*view == View::SightReading).then(|| ("Sight-reading".to_owned(), plain)),
        (*echo == MidiEcho::Mute).then(|| ("MIDI muted".to_owned(), plain)),
        (transport.state == PlayState::Paused).then(|| ("PAUSED".to_owned(), palette::MISSED)),
    ]
    .into_iter()
    .flatten()
    .collect();
    if *shown == wanted {
        return;
    }
    let row = *chips;
    commands.entity(row).despawn_children();
    wanted.iter().for_each(|(label, color)| {
        let tag = commands.spawn(chip(label.clone(), *color)).id();
        commands.entity(row).add_child(tag);
    });
    *shown = wanted;
}

/// The best for what's being played now, if any.
fn current_best(
    bests: &Bests,
    song: &ActiveSong,
    judge: &Judge,
    transport: &Transport,
) -> Option<Best> {
    bests.records.best(&RecordKey {
        title: song.0.title.clone(),
        mode: transport.mode,
        hands: judge.0.hands(),
    })
}

/// 39700 -> "39,700".
fn grouped(n: u32) -> String {
    let digits = n.to_string();
    let len = digits.len();
    digits
        .chars()
        .enumerate()
        .flat_map(|(i, c)| {
            let comma = i > 0 && (len - i).is_multiple_of(3);
            comma.then_some(',').into_iter().chain([c])
        })
        .collect()
}

#[expect(clippy::too_many_arguments, reason = "Bevy system parameters")]
fn update_score(
    mut card: Single<&mut Visibility, With<ScoreCard>>,
    mut lines: Query<(&ScoreLine, &mut Text)>,
    mut display: ResMut<Display>,
    judge: Res<Judge>,
    transport: Res<Transport>,
    time: Res<Time>,
    bests: Res<Bests>,
    song: Res<ActiveSong>,
) {
    let scored = transport.mode.is_scored();
    card.set_if_neq(if scored {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    if !scored {
        return;
    }
    let score = judge.0.score();
    let target = score.points as f32;
    // Roll the number up quickly but visibly.
    let step = (target - display.points) * (1.0 - (-12.0 * time.delta_secs()).exp());
    display.points = if (target - display.points).abs() < 1.0 {
        target
    } else {
        display.points + step
    };
    let best = current_best(&bests, &song, &judge, &transport);
    // Once finished, the run is already in the bests; the card says how it went.
    let chasing = transport.state != PlayState::Finished;
    lines.iter_mut().for_each(|(line, mut text)| {
        let wanted = match line {
            ScoreLine::Points => grouped(display.points.round() as u32),
            ScoreLine::Multiplier => format!("×{}", score.multiplier()),
            ScoreLine::Accuracy => score
                .accuracy()
                .map_or_else(|| "–".to_owned(), |a| format!("{:.1}%", a * 100.0)),
            ScoreLine::Chase => records::chase(score.points, best)
                .label()
                .filter(|_| chasing)
                .unwrap_or_default(),
        };
        set(&mut text, wanted);
    });
}

/// One big pop the moment a run overtakes the personal best.
fn best_beaten(
    mut commands: Commands,
    judge: Res<Judge>,
    transport: Res<Transport>,
    bests: Res<Bests>,
    song: Res<ActiveSong>,
    mut was_ahead: Local<bool>,
) {
    let best = current_best(&bests, &song, &judge, &transport);
    let points = judge.0.score().points;
    let ahead = transport.mode.is_scored()
        && transport.state == PlayState::Playing
        && matches!(records::chase(points, best), Chase::Ahead { .. });
    // A restart drops back behind and re-arms the pop.
    let behind = !matches!(records::chase(points, best), Chase::Ahead { .. });
    if ahead && !*was_ahead {
        popup(
            &mut commands,
            "BEST BEATEN!",
            palette::timing(Timing::Perfect),
            Vec2::new(0.0, 60.0),
            34.0,
            1.0,
        );
        *was_ahead = true;
    } else if behind {
        *was_ahead = false;
    }
}

fn update_combo(
    mut combo_text: Single<(&mut Text2d, &mut TextColor, &mut Transform), With<ComboText>>,
    mut display: ResMut<Display>,
    judge: Res<Judge>,
    time: Res<Time>,
    transport: Res<Transport>,
    mut last_combo: Local<u32>,
) {
    let combo = judge.0.score().combo;
    if combo > *last_combo {
        display.combo_pulse = 1.0;
    }
    *last_combo = combo;
    display.combo_pulse = (display.combo_pulse - time.delta_secs() * 5.0).max(0.0);
    let (text, color, transform) = &mut *combo_text;
    // The results card takes over at the end.
    if combo < MIN_SHOWN_COMBO || transport.state == PlayState::Finished {
        color.0 = color.0.with_alpha(0.0);
        return;
    }
    text.0 = format!("{combo}");
    // Gets bolder as the multiplier climbs.
    let mult = judge.0.score().multiplier() as f32;
    let tint = palette::HUD_TEXT.mix(&palette::timing(Timing::Perfect), (mult - 1.0) / 3.0);
    color.0 = tint.with_alpha(0.22 + 0.25 * display.combo_pulse);
    transform.scale = Vec3::splat(1.0 + 0.18 * display.combo_pulse);
}

fn update_timing(
    mut text: Single<(&mut Text2d, &mut TextColor), With<TimingText>>,
    mut display: ResMut<Display>,
    mut feedback: MessageReader<Feedback>,
    time: Res<Time>,
) {
    feedback.read().for_each(|event| match event {
        Feedback::Hit(j) => {
            display.last = Some(*j);
            display.last_age = 0.0;
        }
        Feedback::Missed(_)
        | Feedback::Wrong { .. }
        | Feedback::AutoPlayed(_)
        | Feedback::Touch { .. } => {}
    });
    display.last_age += time.delta_secs();
    let (text, color) = &mut *text;
    match display.last {
        Some(j) => {
            let ms = (j.offset.0 * 1000.0).round() as i32;
            text.0 = match ms {
                0 => "on the beat".to_owned(),
                m if m < 0 => format!("{} ms early", -m),
                m => format!("{m} ms late"),
            };
            let fade = (1.0 - (display.last_age - 1.0).max(0.0)).clamp(0.0, 1.0);
            color.0 = palette::timing(j.timing).with_alpha(0.8 * fade);
        }
        None => text.0.clear(),
    }
}

fn update_strip(
    mut fill: Single<&mut Transform, With<ProgressFill>>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
) {
    let fraction = (transport.time.0 / song.0.duration().0).clamp(0.0, 1.0);
    fill.scale.x = fraction * layout::CANVAS_WIDTH;
}

fn tick_strip(
    mut commands: Commands,
    mut feedback: MessageReader<Feedback>,
    song: Res<ActiveSong>,
    mut ticks: Query<Entity, With<ProgressTick>>,
    transport: Res<Transport>,
) {
    // Restarting clears the history.
    if transport.is_changed() && transport.time.0 < 0.0 {
        ticks.iter_mut().for_each(|e| commands.entity(e).despawn());
    }
    let duration = song.0.duration().0;
    feedback.read().for_each(|event| {
        let (id, color) = match *event {
            Feedback::Hit(j) => (j.note, palette::timing(j.timing)),
            Feedback::Missed(id) => (id, palette::MISSED),
            Feedback::AutoPlayed(_) | Feedback::Wrong { .. } | Feedback::Touch { .. } => return,
        };
        let x = layout::LEFT + song.0.note(id).start.0 / duration * layout::CANVAS_WIDTH;
        commands.spawn((
            SongScoped,
            ProgressTick,
            Sprite::from_color(color, Vec2::new(2.0, STRIP_HEIGHT)),
            Anchor::TOP_CENTER,
            Transform::from_xyz(x, layout::NOTATION_BOTTOM, STRIP_Z + 0.2),
        ));
    });
}

fn show_help(mut help: Single<&mut Visibility, With<HelpText>>, transport: Res<Transport>) {
    let wanted = match transport.state {
        PlayState::Playing => Visibility::Inherited,
        // Paused, the pause panel takes over.
        PlayState::Paused | PlayState::Finished => Visibility::Hidden,
    };
    if **help != wanted {
        **help = wanted;
    }
}
