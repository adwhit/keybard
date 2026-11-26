//! Plays notes through the piano-like synth in [`crate::synth`].

use std::time::{Duration, Instant};

use bevy::prelude::*;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::cpal::{self, BufferSize, Device, DeviceId, Host};
use rodio::stream::DeviceSinkError;
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Sample, SampleRate, Source};

use super::Phase;
use crate::dynamics::Velocity;
use crate::music::MidiNote;
use crate::song::Seconds;
use crate::synth::{self, PianoVoice};

/// Request to sound a note.
#[derive(Message, Debug, Clone, Copy)]
pub struct PlayTone {
    pub note: MidiNote,
    pub duration: Seconds,
    pub velocity: Velocity,
}

const VOLUME: f32 = 0.35;
const MIN_DURATION: Seconds = Seconds(0.08);

// About 5 ms at 48 kHz. The device can reject the request.
const BUFFER_FRAMES: u32 = 256;
const OUTPUT_CHECK_INTERVAL: Duration = Duration::from_millis(500);

/// Adapts the pure synth iterator to the audio backend.
struct PianoDecoder(PianoVoice);

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

struct AudioOutput {
    host: Host,
    next_check: Instant,
    stream: Option<(Option<DeviceId>, MixerDeviceSink)>,
}

impl AudioOutput {
    fn refresh(&mut self) {
        let now = Instant::now();
        if now < self.next_check {
            return;
        }
        self.next_check = now + OUTPUT_CHECK_INTERVAL;

        let Some(device) = self.host.default_output_device() else {
            self.stream = None;
            return;
        };
        let id = device.id().ok();
        if self
            .stream
            .as_ref()
            .is_some_and(|(current, _)| *current == id)
        {
            return;
        }

        self.stream = None;
        match open_output(device) {
            Ok(stream) => {
                tracing::debug!(?id, "opened default audio output");
                self.stream = Some((id, stream));
            }
            Err(error) => {
                tracing::debug!(%error, "could not open default audio output, will retry")
            }
        }
    }
}

fn open_output(device: Device) -> Result<MixerDeviceSink, DeviceSinkError> {
    DeviceSinkBuilder::from_device(device.clone())?
        .with_buffer_size(BufferSize::Fixed(BUFFER_FRAMES))
        .open_stream()
        .or_else(|error| {
            tracing::debug!(%error, "low-latency audio unavailable, trying standard output");
            DeviceSinkBuilder::from_device(device)?.open_sink_or_fallback()
        })
}

pub fn plugin(app: &mut App) {
    app.insert_non_send(AudioOutput {
        host: cpal::default_host(),
        next_check: Instant::now(),
        stream: None,
    })
    .add_systems(Update, play_tones.in_set(Phase::Present));
}

fn play_tones(mut requests: MessageReader<PlayTone>, mut output: NonSendMut<AudioOutput>) {
    output.refresh();
    let Some((_, stream)) = &output.stream else {
        requests.clear();
        return;
    };
    requests.read().for_each(|tone| {
        // Relative to a mezzo-forte note, so the default sounds as before.
        let loudness = (tone.velocity.gain() / Velocity::DEFAULT.gain()).min(1.6);
        let held = Seconds(tone.duration.0.max(MIN_DURATION.0));
        let voice = PianoDecoder(PianoVoice::new(tone.note.frequency(), held));
        stream.mixer().add(voice.amplify(VOLUME * loudness));
    });
}
