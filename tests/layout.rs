use keybard::layout::{self, CANVAS_WIDTH, KeyRange, LEFT, RIGHT};
use keybard::music::{KeyColor, MidiNote, PitchClass};
use keybard::song::Seconds;

fn n(v: u8) -> MidiNote {
    MidiNote::new(v).expect("valid note")
}

#[test]
fn ranges_are_whole_octaves_from_c_to_b() {
    let range = KeyRange::covering(n(48), n(67));
    assert_eq!(range.low().pitch_class(), PitchClass::C);
    assert_eq!(range.high().pitch_class(), PitchClass::B);
    assert!(range.contains(n(48)) && range.contains(n(67)));
}

#[test]
fn small_ranges_are_padded_to_four_octaves() {
    let range = KeyRange::covering(n(60), n(62));
    assert_eq!(range.notes().count(), 48);
    assert!(range.contains(n(60)));
}

#[test]
fn wide_ranges_are_kept() {
    let range = KeyRange::covering(n(21), n(108));
    assert!(range.contains(n(21)) && range.contains(n(108)));
    assert_eq!(range.low(), n(12));
    assert_eq!(range.high(), n(119));
}

#[test]
fn white_keys_tile_the_canvas() {
    let range = KeyRange::covering(n(48), n(83));
    let whites: Vec<_> = range
        .notes()
        .filter_map(|note| range.geometry(note))
        .filter(|g| g.color == KeyColor::White)
        .collect();
    assert_eq!(whites.len(), range.white_key_count());
    let w = range.white_key_width();
    assert!((w * whites.len() as f32 - CANVAS_WIDTH).abs() < 1e-3);
    assert!((whites[0].center_x - w / 2.0 - LEFT).abs() < 1e-3);
    assert!((whites.last().unwrap().center_x + w / 2.0 - RIGHT).abs() < 1e-3);
    // Strictly increasing, evenly spaced.
    assert!(
        whites
            .windows(2)
            .all(|p| (p[1].center_x - p[0].center_x - w).abs() < 1e-3)
    );
}

#[test]
fn black_keys_sit_between_their_neighbours() {
    let range = KeyRange::covering(n(48), n(83));
    let geo = |v| range.geometry(n(v)).expect("on keyboard");
    let (c, cs, d) = (geo(60), geo(61), geo(62));
    assert_eq!(cs.color, KeyColor::Black);
    assert!(cs.width < c.width);
    assert!((cs.center_x - (c.center_x + d.center_x) / 2.0).abs() < 1e-3);
    assert!(range.geometry(n(30)).is_none());
}

#[test]
fn falling_coordinates() {
    assert_eq!(layout::fall_y(Seconds(0.0)), layout::HIT_LINE);
    let top = layout::fall_y(layout::fall_lookahead());
    assert!((top - layout::NOTATION_BOTTOM).abs() < 1e-3);
}

#[test]
fn staff_coordinates() {
    use keybard::layout::staff;
    use keybard::music::{Clef, StaffPosition};
    assert_eq!(staff::x(Seconds(0.0)), staff::PLAYHEAD_X);
    assert!(staff::x(Seconds(1.0)) > staff::PLAYHEAD_X);
    assert!(staff::is_visible(staff::PLAYHEAD_X));
    assert!(!staff::is_visible(LEFT + 10.0));
    let treble_bottom = staff::y(Clef::Treble, StaffPosition::BOTTOM_LINE);
    let bass_top = staff::y(Clef::Bass, StaffPosition::TOP_LINE);
    assert!(treble_bottom > bass_top, "treble staff must sit above bass");
    // Middle C ledger lines of the two staves must not cross.
    assert!(staff::y(Clef::Treble, StaffPosition(-2)) > staff::y(Clef::Bass, StaffPosition(10)));
}
