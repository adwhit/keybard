//! Offline rendering of a whole song through the synth, for listening to the
//! sound without running the app (`--render-wav`).

use std::path::Path;

use tracing::info;

use crate::error::Error;
use crate::song::{Seconds, Song};
use crate::synth::{PianoVoice, RELEASE, SAMPLE_RATE};

/// Loudest sample after normalisation.
const PEAK: f32 = 0.9;

fn sample_index(time: Seconds) -> usize {
    (time.0.max(0.0) * SAMPLE_RATE as f32).round() as usize
}

/// Mix every note of the song into one mono track, normalised to [`PEAK`].
pub fn render_song(song: &Song) -> Vec<f32> {
    let mut mix = vec![0.0_f32; sample_index(song.duration() + RELEASE)];
    song.notes().iter().for_each(|note| {
        let start = sample_index(note.start).min(mix.len());
        mix[start..]
            .iter_mut()
            .zip(PianoVoice::new(note.pitch.frequency(), note.duration))
            .for_each(|(slot, s)| *slot += s);
    });
    let peak = mix.iter().map(|s| s.abs()).fold(0.0, f32::max);
    if peak > 0.0 {
        let gain = PEAK / peak;
        mix.iter_mut().for_each(|s| *s *= gain);
    }
    mix
}

/// Write mono 16-bit PCM WAV.
pub fn write_wav(path: &Path, samples: &[f32]) -> Result<(), Error> {
    let data_len = u32::try_from(samples.len() * 2).unwrap_or(u32::MAX);
    let header = [
        b"RIFF".as_slice(),
        &(36 + data_len).to_le_bytes(),
        b"WAVEfmt ",
        &16u32.to_le_bytes(), // fmt chunk size
        &1u16.to_le_bytes(),  // PCM
        &1u16.to_le_bytes(),  // mono
        &SAMPLE_RATE.to_le_bytes(),
        &(SAMPLE_RATE * 2).to_le_bytes(), // byte rate
        &2u16.to_le_bytes(),              // block align
        &16u16.to_le_bytes(),             // bits per sample
        b"data",
        &data_len.to_le_bytes(),
    ]
    .concat();
    let pcm = samples
        .iter()
        .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    let bytes: Vec<u8> = header.into_iter().chain(pcm).collect();
    std::fs::write(path, bytes).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })?;
    info!(path = %path.display(), seconds = samples.len() as f32 / SAMPLE_RATE as f32, "wrote wav");
    Ok(())
}
