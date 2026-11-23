//! Status line and key help.

use bevy::prelude::*;

use super::transport::{PlayState, Transport};
use super::{ActiveSong, Judge, Phase, palette};

#[derive(Component)]
struct StatusText;

const HELP: &str = "Space play/pause · ←/→ seek · ↑/↓ speed · Tab mode · Backspace restart · Esc quit\n\
Play with Z S X D C V… (lower octave) and Q 2 W 3 E R… (upper) · -/= shift octave · or a MIDI keyboard";

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_hud)
        .add_systems(Update, update_status.in_set(Phase::Present));
}

fn spawn_hud(mut commands: Commands) {
    let font = |size: f32| TextFont {
        font_size: FontSize::Px(size),
        ..default()
    };
    commands.spawn((
        StatusText,
        Text::new(""),
        font(14.0),
        TextColor(palette::HUD_TEXT),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            top: Val::Percent(31.0),
            ..default()
        },
    ));
    commands.spawn((
        Text::new(HELP),
        font(11.0),
        TextColor(palette::KEY_LABEL),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(8.0),
            top: Val::Percent(31.0),
            ..default()
        },
        TextLayout::justify(Justify::Right),
    ));
}

fn update_status(
    mut text: Single<&mut Text, With<StatusText>>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    judge: Res<Judge>,
) {
    let score = judge.0.score();
    let state = match transport.state {
        PlayState::Playing => "",
        PlayState::Paused => " · PAUSED",
        PlayState::Finished => " · FINISHED (Space to replay)",
    };
    let elapsed = transport.time.0.max(0.0);
    let total = song.0.duration().0;
    text.0 = format!(
        "{title}\n{mode} · {speed:.0}% · {elapsed:.0}/{total:.0}s{state}\nhit {hit} · missed {missed} · wrong {wrong}",
        title = song.0.title,
        mode = transport.mode.label(),
        speed = transport.speed.get() * 100.0,
        hit = score.hit,
        missed = score.missed,
        wrong = score.wrong,
    );
}
