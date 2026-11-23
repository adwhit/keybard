use keybard::song::Seconds;
use keybard::synth::{PianoVoice, RELEASE, SAMPLE_RATE};

fn render(frequency: f32, held: f32) -> Vec<f32> {
    PianoVoice::new(frequency, Seconds(held)).collect()
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

fn ms(n: f32) -> usize {
    (n / 1000.0 * SAMPLE_RATE as f32) as usize
}

#[test]
fn length_is_the_held_time_plus_release() {
    let samples = render(440.0, 0.5);
    let expected = ((0.5_f32 + RELEASE.0) * SAMPLE_RATE as f32).round() as usize;
    assert!(
        (samples.len() as i64 - expected as i64).abs() <= 1,
        "{}",
        samples.len()
    );
}

#[test]
fn starts_and_ends_silently_to_avoid_clicks() {
    let samples = render(440.0, 0.4);
    assert!(samples[0].abs() < 1e-3);
    assert!(samples.last().unwrap().abs() < 1e-3);
    // No sample-to-sample jump anywhere is large enough to click.
    let max_step = samples
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max);
    assert!(max_step < 0.2, "max step {max_step}");
}

#[test]
fn stays_within_full_scale() {
    [27.5, 261.6, 440.0, 4186.0].into_iter().for_each(|f| {
        let peak = render(f, 0.5).iter().map(|s| s.abs()).fold(0.0, f32::max);
        assert!(peak <= 1.0 && peak > 0.1, "{f} Hz peaks at {peak}");
    });
}

#[test]
fn strikes_then_decays_like_a_struck_string() {
    let samples = render(261.6, 1.5);
    let early = rms(&samples[ms(10.0)..ms(60.0)]);
    let middle = rms(&samples[ms(600.0)..ms(650.0)]);
    let late = rms(&samples[ms(1400.0)..ms(1450.0)]);
    assert!(early > middle && middle > late, "{early} {middle} {late}");
    assert!(late > 0.0, "still ringing while held");
}

#[test]
fn high_notes_die_away_faster_than_low_ones() {
    let fade = |f: f32| {
        let s = render(f, 1.0);
        rms(&s[ms(800.0)..ms(850.0)]) / rms(&s[ms(10.0)..ms(60.0)])
    };
    assert!(fade(1046.5) < fade(130.8));
}

#[test]
fn sounds_at_the_requested_pitch() {
    // The fundamental period is the lag where the waveform best matches
    // itself (harmonics add wiggles, so zero crossings would mislead).
    let samples = render(220.0, 1.0);
    let window = &samples[ms(100.0)..ms(300.0)];
    let correlation =
        |lag: usize| -> f32 { window.iter().zip(&window[lag..]).map(|(a, b)| a * b).sum() };
    let best_lag = (100..400)
        .max_by(|a, b| correlation(*a).total_cmp(&correlation(*b)))
        .unwrap();
    let hz = SAMPLE_RATE as f32 / best_lag as f32;
    assert!((hz - 220.0).abs() < 3.0, "measured {hz} Hz");
}

#[test]
fn is_deterministic() {
    assert_eq!(render(330.0, 0.2), render(330.0, 0.2));
}
