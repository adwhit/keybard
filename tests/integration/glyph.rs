use bevy::math::Vec2;
use keybard::glyph::{bass, treble};

#[test]
fn the_treble_clef_spans_from_below_to_above_the_staff() {
    let (lo, hi) = treble().bounds();
    // Staff lines are at steps 0..=8.
    assert!(lo.y < -2.5 && lo.y > -5.0, "tail below the staff: {lo}");
    assert!(hi.y > 10.0 && hi.y < 12.5, "loop above the staff: {hi}");
    assert!(
        lo.x >= -0.5 && hi.x < 7.0,
        "about two spaces wide: {lo} {hi}"
    );
}

#[test]
fn the_treble_clef_curls_around_the_g_line() {
    let g = treble();
    // The spiral's eye is open, its ring is inked, on the G line (step 2).
    assert!(!g.contains(Vec2::new(2.2, 2.3)), "open eye of the spiral");
    assert!(g.contains(Vec2::new(0.1, 2.3)), "inked left of the eye");
    assert!(g.contains(Vec2::new(5.2, 2.3)), "inked right of the eye");
}

#[test]
fn the_bass_clef_dots_sit_either_side_of_the_f_line() {
    let f = bass();
    // F line is step 6: the dots are in the spaces above and below it.
    assert!(f.contains(Vec2::new(6.1, 7.0)));
    assert!(f.contains(Vec2::new(6.1, 5.0)));
    assert!(
        !f.contains(Vec2::new(6.1, 6.0)),
        "the F line runs between them"
    );
    // The head dot sits on the F line at the start of the curl.
    assert!(f.contains(Vec2::new(1.0, 6.0)));
    let (lo, hi) = f.bounds();
    assert!(
        lo.y < 0.5 && hi.y > 8.0,
        "from the bottom line to above the top: {lo} {hi}"
    );
}

#[test]
fn strokes_are_thick_and_thin() {
    // Across the right side of the bass curl (thick) vs its thin tail end.
    let f = bass();
    let thickness = |y: f32, x0: f32, x1: f32| {
        (0..400)
            .map(|i| x0 + (x1 - x0) * i as f32 / 400.0)
            .filter(|x| f.contains(Vec2::new(*x, y)))
            .count() as f32
            * (x1 - x0)
            / 400.0
    };
    let body = thickness(5.4, 3.0, 6.0);
    let tail = thickness(0.3, -1.0, 1.5);
    assert!(body > 2.0 * tail, "body {body} vs tail {tail}");
}
