//! Pausing from the piano: three neighbouring keys stop the music and open
//! a panel of settings that chords can change, all without leaving the song.

use super::*;
use keybard::app::falling::View;
use keybard::app::transport::Metronome;

/// The pause cluster: C5, C#5 and D5 together.
const CLUSTER: [u8; 3] = [72, 73, 74];

fn hold(h: &mut Harness, pitches: &[u8]) {
    pitches.iter().for_each(|p| h.midi(*p, KeyEdge::Down));
}

fn release(h: &mut Harness, pitches: &[u8]) {
    pitches.iter().for_each(|p| h.midi(*p, KeyEdge::Up));
    h.frames(2);
}

/// A chord on `root` (a pitch class name's semitone) from middle C up.
fn play(h: &mut Harness, control: Control) {
    let pitches: Vec<u8> = control
        .chord()
        .unwrap_or_else(|| panic!("{control:?} has a chord"))
        .pitch_classes()
        .iter()
        .map(|pc| 60 + pc.semitone())
        .collect();
    hold(h, &pitches);
    h.frames(2);
    release(h, &pitches);
}

/// Playing along with a combo going, then the cluster.
fn paused_mid_song() -> Harness {
    let mut h = Harness::new(song(), PlayMode::Play);
    h.run_until(0.0);
    hold(&mut h, &[48, 60]);
    release(&mut h, &[48, 60]);
    h.run_until(0.5);
    hold(&mut h, &CLUSTER);
    h.frames(2);
    h
}

#[test]
fn three_neighbouring_keys_pause_without_costing_a_thing() {
    let mut h = Harness::new(song(), PlayMode::Play);
    h.run_until(0.0);
    hold(&mut h, &[48, 60]);
    release(&mut h, &[48, 60]);
    h.run_until(0.5);
    let before = h.score();
    assert_eq!(before.combo, 2);
    hold(&mut h, &CLUSTER);
    h.frames(2);
    assert_eq!(h.transport().state, PlayState::Paused);
    assert_eq!(h.score(), before, "the cluster's notes aren't wrong notes");
    let time = h.time();
    h.frames(30);
    assert_eq!(h.time(), time, "the clock stops");
}

#[test]
fn the_cluster_again_carries_on() {
    let mut h = paused_mid_song();
    release(&mut h, &CLUSTER);
    assert_eq!(
        h.transport().state,
        PlayState::Paused,
        "letting go stays paused"
    );
    hold(&mut h, &CLUSTER);
    h.frames(2);
    assert_eq!(h.transport().state, PlayState::Playing);
}

#[test]
fn a_held_cluster_pauses_once() {
    let mut h = paused_mid_song();
    h.frames(30);
    assert_eq!(
        h.transport().state,
        PlayState::Paused,
        "holding doesn't flicker"
    );
}

#[test]
fn the_pause_panel_shows_only_while_paused() {
    let mut h = Harness::new(song(), PlayMode::Play);
    h.frames(5);
    assert!(!h.has_button(Control::Restart), "hidden while playing");
    h.tap(KeyCode::Space);
    assert!(h.has_button(Control::Restart), "Space pauses too");
    assert!(h.has_button(Control::TogglePlay), "a Resume button");
    assert!(
        h.ui_texts().iter().any(|t| t == "PAUSED"),
        "{:?}",
        h.ui_texts()
    );
    h.tap(KeyCode::Space);
    assert!(!h.has_button(Control::Restart));
}

#[test]
fn chords_change_settings_while_paused() {
    let mut h = paused_mid_song();
    release(&mut h, &CLUSTER);

    play(&mut h, Control::Slower);
    assert!((h.transport().speed.get() - 0.95).abs() < 1e-5);
    play(&mut h, Control::Slower);
    play(&mut h, Control::Faster);
    assert!((h.transport().speed.get() - 0.95).abs() < 1e-5);
    assert!(
        h.ui_texts().iter().any(|t| t == "95%"),
        "the panel shows the speed: {:?}",
        h.ui_texts()
    );

    play(&mut h, Control::SetMode(PlayMode::Wait));
    assert_eq!(h.transport().mode, PlayMode::Wait);
    play(&mut h, Control::SetHands(Hands::Right));
    assert_eq!(h.world().resource::<Judge>().0.hands(), Hands::Right);
    play(&mut h, Control::ToggleMetronome);
    assert_eq!(*h.world().resource::<Metronome>(), Metronome::On);
    play(&mut h, Control::ToggleSightReading);
    assert_eq!(*h.world().resource::<View>(), View::SightReading);
    assert_eq!(h.transport().state, PlayState::Paused, "still paused");

    play(&mut h, Control::TogglePlay);
    assert_eq!(h.transport().state, PlayState::Playing, "C major resumes");
}

#[test]
fn every_pause_panel_button_has_its_own_chord() {
    let mut h = paused_mid_song();
    let mut q = h.world().query::<(&ControlButton, &InheritedVisibility)>();
    let controls: Vec<Control> = q
        .iter(h.app.world())
        .filter(|(_, v)| v.get())
        .map(|(b, _)| b.0)
        .collect();
    assert!(controls.len() >= 15, "{controls:?}");
    let mut chords: Vec<String> = controls
        .iter()
        .map(|c| {
            c.chord()
                .unwrap_or_else(|| panic!("{c:?} needs a chord"))
                .label()
        })
        .collect();
    let all = chords.len();
    chords.sort();
    chords.dedup();
    assert_eq!(chords.len(), all, "no two share a chord");
}

#[test]
fn restart_from_the_panel_starts_over() {
    let mut h = paused_mid_song();
    release(&mut h, &CLUSTER);
    play(&mut h, Control::Restart);
    assert_eq!(h.transport().state, PlayState::Playing);
    assert!(h.time().0 < 0.0, "back to the count-in");
    assert_eq!(h.score().combo, 0);
}
