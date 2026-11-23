//! Simple sine-wave synthesis using Bevy's built-in `Pitch` audio source.

use std::time::Duration;

use bevy::audio::Volume;
use bevy::prelude::*;

use super::Phase;
use crate::music::MidiNote;
use crate::song::Seconds;

/// Request to sound a note.
#[derive(Message, Debug, Clone, Copy)]
pub struct PlayTone {
    pub note: MidiNote,
    pub duration: Seconds,
}

const VOLUME: f32 = 0.18;
const MIN_DURATION: Seconds = Seconds(0.08);

pub fn plugin(app: &mut App) {
    app.add_systems(Update, play_tones.in_set(Phase::Present));
}

fn play_tones(
    mut requests: MessageReader<PlayTone>,
    mut pitches: ResMut<Assets<Pitch>>,
    mut commands: Commands,
) {
    requests.read().for_each(|tone| {
        let seconds = tone.duration.0.max(MIN_DURATION.0);
        let pitch = Pitch::new(tone.note.frequency(), Duration::from_secs_f32(seconds));
        commands.spawn((
            AudioPlayer(pitches.add(pitch)),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(VOLUME)),
        ));
    });
}
