use keybard::render::{render_song, write_wav};
use keybard::song::{Hand, Song, Tempo, parse_voice};
use keybard::synth::{RELEASE, SAMPLE_RATE};

fn song() -> Song {
    let tempo = Tempo::from_bpm(120.0, 4);
    let notes = parse_voice("C4+E4+G4/2 C5", Hand::Right, tempo)
        .unwrap()
        .into_iter()
        .chain(parse_voice("C3/3", Hand::Left, tempo).unwrap())
        .collect();
    Song::new("chords", tempo, notes).unwrap()
}

#[test]
fn renders_the_whole_song_plus_release() {
    let song = song();
    let samples = render_song(&song);
    let expected = ((song.duration() + RELEASE).0 * SAMPLE_RATE as f32).round() as usize;
    assert!((samples.len() as i64 - expected as i64).abs() <= 2);
}

#[test]
fn mixes_without_clipping() {
    let samples = render_song(&song());
    let peak = samples.iter().map(|s| s.abs()).fold(0.0, f32::max);
    assert!(peak <= 1.0, "peak {peak}");
    assert!(peak > 0.3, "normalised to a sensible level: {peak}");
}

#[test]
fn notes_start_at_their_times() {
    let samples = render_song(&song());
    let at = |t: f32| (t * SAMPLE_RATE as f32) as usize;
    // Second right-hand note (C5) starts at 1.0s: energy jumps there.
    let energy =
        |from: usize, len: usize| -> f32 { samples[from..from + len].iter().map(|s| s * s).sum() };
    let before = energy(at(0.95), at(0.04));
    let after = energy(at(1.01), at(0.04));
    assert!(after > before * 1.2, "{before} -> {after}");
}

#[test]
fn writes_a_valid_wav_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.wav");
    let samples = vec![0.0, 0.5, -0.5, 1.0];
    write_wav(&path, &samples).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..16], b"WAVEfmt ");
    assert_eq!(
        u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
        SAMPLE_RATE
    );
    assert_eq!(
        u16::from_le_bytes(bytes[34..36].try_into().unwrap()),
        16,
        "16-bit"
    );
    assert_eq!(&bytes[36..40], b"data");
    assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 8);
    assert_eq!(bytes.len(), 44 + 8);
    let second = i16::from_le_bytes(bytes[46..48].try_into().unwrap());
    assert_eq!(second, 16384);
}
