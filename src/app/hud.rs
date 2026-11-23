//! Heads-up display: song info, score, combo, timing readout and a progress
//! strip that records every judgement as a coloured tick.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use super::section::SectionLoop;
use super::songs::SongScoped;
use super::transport::{PlayState, Transport};
use super::{ActiveSong, Feedback, Judge, Phase, palette};
use crate::layout;
use crate::performance::{Judgement, Timing};

#[derive(Component)]
struct InfoText;

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct ComboText;

#[derive(Component)]
struct TimingText;

#[derive(Component)]
struct ProgressFill;

/// Key help, shown before the music starts and while paused.
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

const HELP: &str = "Space play/pause | Left/Right seek | Up/Down speed | Tab mode | F2 hands | F3 loop 4 bars | PgUp/PgDn song | Backspace restart | F1 note names | Esc quit\n\
Play with Z S X D C V... and Q 2 W 3 E R... | -/= shift octave | or a MIDI keyboard";

const STRIP_HEIGHT: f32 = 5.0;
const STRIP_Z: f32 = 6.0;
const COMBO_Z: f32 = 0.5;
/// Combos shorter than this aren't worth shouting about.
const MIN_SHOWN_COMBO: u32 = 3;

pub fn plugin(app: &mut App) {
    app.init_resource::<Display>()
        .add_systems(Startup, (spawn_hud, spawn_strip))
        .add_systems(
            Update,
            (
                update_info,
                update_score,
                update_combo,
                update_timing,
                update_strip,
                tick_strip,
                show_help,
            )
                .in_set(Phase::Present),
        );
}

fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

fn spawn_hud(mut commands: Commands) {
    let top = Val::Percent(31.0);
    commands.spawn((
        InfoText,
        Text::new(""),
        font(13.0),
        TextColor(palette::HUD_TEXT),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            top,
            ..default()
        },
    ));
    commands.spawn((
        ScoreText,
        Text::new(""),
        font(26.0),
        TextColor(palette::HUD_TEXT),
        TextLayout::justify(Justify::Right),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(10.0),
            top,
            ..default()
        },
    ));
    commands.spawn((
        HelpText,
        Text::new(HELP),
        font(10.0),
        TextColor(palette::HUD_DIM.with_alpha(0.7)),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Percent(66.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
    ));
    commands.spawn((
        ComboText,
        Text2d::new(""),
        font(64.0),
        TextColor(palette::HUD_TEXT.with_alpha(0.0)),
        Transform::from_xyz(0.0, 70.0, COMBO_Z),
    ));
    commands.spawn((
        TimingText,
        Text2d::new(""),
        font(14.0),
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

fn update_info(
    mut text: Single<&mut Text, With<InfoText>>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    judge: Res<Judge>,
    section: Res<SectionLoop>,
) {
    let state = match transport.state {
        PlayState::Playing => "",
        PlayState::Paused => "  PAUSED",
        PlayState::Finished => "",
    };
    let elapsed = transport.time.0.max(0.0);
    let total = song.0.duration().0;
    let clock = |s: f32| format!("{}:{:02}", (s / 60.0) as u32, (s % 60.0) as u32);
    text.0 = format!(
        "{title}\n{mode} mode | {hands} | {speed:.0}% speed | {elapsed} / {total}{state}",
        title = song.0.title,
        mode = transport.mode.label(),
        hands = judge.0.hands().label(),
        speed = transport.speed.get() * 100.0,
        elapsed = clock(elapsed),
        total = clock(total),
    );
    if let Some(looping) = &section.0 {
        let tempo = song.0.tempo;
        let first = looping.region.first_bar(tempo);
        let last = first + looping.region.bars(tempo) - 1;
        text.0.push_str(&format!(
            "\nLOOP bars {first}-{last} | pass {pass}",
            pass = looping.passes + 1
        ));
        if transport.mode.is_scored() {
            text.0
                .push_str(&format!(" | clean {}", looping.clean_passes));
            if let Some(last) = looping.last_pass {
                text.0.push_str(&format!(
                    " | last: {} hit, {} missed, {} wrong",
                    last.hits, last.missed, last.wrong
                ));
            }
        }
    }
}

fn update_score(
    mut text: Single<&mut Text, With<ScoreText>>,
    mut display: ResMut<Display>,
    judge: Res<Judge>,
    transport: Res<Transport>,
    time: Res<Time>,
) {
    if !transport.mode.is_scored() {
        text.0.clear();
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
    let accuracy = score
        .accuracy()
        .map_or_else(|| "-".to_owned(), |a| format!("{:.1}%", a * 100.0));
    text.0 = format!(
        "{points:07}\nx{mult}  |  {accuracy}",
        points = display.points.round() as u32,
        mult = score.multiplier(),
    );
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
        Feedback::Missed(_) | Feedback::Wrong(_) | Feedback::AutoPlayed(_) => {}
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
            Feedback::AutoPlayed(_) | Feedback::Wrong(_) => return,
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
        PlayState::Paused => Visibility::Inherited,
        PlayState::Playing if transport.time.0 < 0.0 => Visibility::Inherited,
        PlayState::Playing | PlayState::Finished => Visibility::Hidden,
    };
    if **help != wanted {
        **help = wanted;
    }
}
