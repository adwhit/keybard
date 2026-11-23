//! Plays notes through the piano-like synth in [`crate::synth`].

use std::time::Duration;

use bevy::audio::{AddAudioSource, ChannelCount, Decodable, Sample, SampleRate, Source, Volume};
use bevy::prelude::*;

use super::Phase;
use crate::music::MidiNote;
use crate::song::Seconds;
use crate::synth::{self, PianoVoice};

/// Request to sound a note.
#[derive(Message, Debug, Clone, Copy)]
pub struct PlayTone {
    pub note: MidiNote,
    pub duration: Seconds,
}

const VOLUME: f32 = 0.35;
const MIN_DURATION: Seconds = Seconds(0.08);

/// One synthesised note, as a Bevy audio asset.
#[derive(Asset, TypePath, Debug, Clone, Copy)]
pub struct PianoTone {
    frequency: f32,
    held: Seconds,
}

/// Adapts the pure synth iterator to the audio backend.
pub struct PianoDecoder(PianoVoice);

impl Iterator for PianoDecoder {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl Source for PianoDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::MIN
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(synth::SAMPLE_RATE).unwrap_or(SampleRate::MIN)
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(
            self.0.len() as f32 / synth::SAMPLE_RATE as f32,
        ))
    }
}

impl Decodable for PianoTone {
    type Decoder = PianoDecoder;

    fn decoder(&self) -> PianoDecoder {
        PianoDecoder(PianoVoice::new(self.frequency, self.held))
    }
}

pub fn plugin(app: &mut App) {
    app.add_audio_source::<PianoTone>()
        .add_systems(Update, play_tones.in_set(Phase::Present));
}

fn play_tones(
    mut requests: MessageReader<PlayTone>,
    mut tones: ResMut<Assets<PianoTone>>,
    mut commands: Commands,
) {
    requests.read().for_each(|tone| {
        let tone = PianoTone {
            frequency: tone.note.frequency(),
            held: Seconds(tone.duration.0.max(MIN_DURATION.0)),
        };
        commands.spawn((
            AudioPlayer(tones.add(tone)),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(VOLUME)),
        ));
    });
}
