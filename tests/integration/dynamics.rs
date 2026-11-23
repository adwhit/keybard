use keybard::dynamics::{Dynamic, Velocity, markings};
use keybard::music::MidiNote;
use keybard::song::{Hand, Note, Seconds, Song, Tempo};

#[test]
fn velocities_map_to_dynamic_levels() {
    assert_eq!(
        Dynamic::from_velocity(Velocity::new(20)),
        Dynamic::Pianissimo
    );
    assert_eq!(Dynamic::from_velocity(Velocity::new(45)), Dynamic::Piano);
    assert_eq!(
        Dynamic::from_velocity(Velocity::new(64)),
        Dynamic::MezzoPiano
    );
    assert_eq!(
        Dynamic::from_velocity(Velocity::DEFAULT),
        Dynamic::MezzoForte
    );
    assert_eq!(Dynamic::from_velocity(Velocity::new(100)), Dynamic::Forte);
    assert_eq!(
        Dynamic::from_velocity(Velocity::new(127)),
        Dynamic::Fortissimo
    );
    assert_eq!(Dynamic::Pianissimo.label(), "pp");
    assert_eq!(Dynamic::MezzoForte.label(), "mf");
    assert_eq!(
        Velocity::new(200),
        Velocity::new(127),
        "clamped to MIDI range"
    );
}

#[test]
fn louder_notes_sound_louder() {
    assert!(Velocity::new(40).gain() < Velocity::DEFAULT.gain());
    assert!(Velocity::DEFAULT.gain() < Velocity::new(120).gain());
    assert!(Velocity::new(127).gain() <= 1.0);
}

fn song(velocities_per_bar: &[u8]) -> Song {
    // 60bpm 4/4: one whole note per bar.
    let tempo = Tempo::from_bpm(60.0, 4);
    let notes = velocities_per_bar
        .iter()
        .enumerate()
        .map(|(bar, v)| Note {
            pitch: MidiNote::new(60).unwrap(),
            start: Seconds(bar as f32 * 4.0),
            duration: Seconds(4.0),
            hand: Hand::Right,
            finger: None,
            velocity: Velocity::new(*v),
        })
        .collect();
    Song::new("d", tempo, notes).unwrap()
}

#[test]
fn markings_appear_where_the_level_changes() {
    let marks: Vec<(f32, &str)> = markings(&song(&[40, 42, 100, 100, 80]))
        .iter()
        .map(|(t, d)| (t.0, d.label()))
        .collect();
    assert_eq!(marks, vec![(0.0, "p"), (8.0, "f"), (16.0, "mf")]);
}

#[test]
fn uniform_songs_get_one_marking() {
    assert_eq!(markings(&song(&[80, 80, 80])).len(), 1);
}

#[test]
fn scores_can_mark_dynamics() {
    use keybard::song::parse_voice;
    let tempo = Tempo::from_bpm(60.0, 4);
    let notes = parse_voice("!pp C4 D4 | !f E4 R !mf F4", Hand::Right, tempo).unwrap();
    let levels: Vec<&str> = notes
        .iter()
        .map(|n| Dynamic::from_velocity(n.velocity).label())
        .collect();
    assert_eq!(levels, vec!["pp", "pp", "f", "mf"]);
    assert!(parse_voice("!loud C4", Hand::Right, tempo).is_err());
}

#[test]
fn each_level_has_a_typical_velocity() {
    [
        Dynamic::Pianissimo,
        Dynamic::Piano,
        Dynamic::MezzoPiano,
        Dynamic::MezzoForte,
        Dynamic::Forte,
        Dynamic::Fortissimo,
    ]
    .into_iter()
    .for_each(|d| assert_eq!(Dynamic::from_velocity(d.velocity()), d));
}

#[test]
fn fur_elise_opens_pianissimo() {
    let song = keybard::builtin::BuiltinSong::FurElise.load().unwrap();
    assert_eq!(
        markings(&song).first().map(|(_, d)| *d),
        Some(Dynamic::Pianissimo)
    );
}

#[test]
fn ode_to_joy_swells_in_its_middle_phrase() {
    let song = keybard::builtin::BuiltinSong::OdeToJoy.load().unwrap();
    let bar = song.tempo.bar_length().0;
    let marks: Vec<(u32, &str)> = markings(&song)
        .iter()
        .map(|(t, d)| ((t.0 / bar).round() as u32 + 1, d.label()))
        .collect();
    assert_eq!(marks, vec![(1, "mf"), (9, "f"), (13, "mf")]);
}
