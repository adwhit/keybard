use keybard::practice::LoopRegion;
use keybard::song::{Seconds, Tempo};

fn tempo() -> Tempo {
    Tempo::from_bpm(120.0, 4) // 2s bars
}

#[test]
fn loops_start_on_the_bar_containing_the_time() {
    let region = LoopRegion::around(tempo(), Seconds(5.3), 2, Seconds(100.0));
    assert_eq!((region.start, region.end), (Seconds(4.0), Seconds(8.0)));
    assert_eq!(region.first_bar(tempo()), 3, "bars are numbered from 1");
    assert_eq!(region.bars(tempo()), 2);
}

#[test]
fn loops_before_the_music_start_at_bar_one() {
    let region = LoopRegion::around(tempo(), Seconds(-1.5), 4, Seconds(100.0));
    assert_eq!((region.start, region.end), (Seconds(0.0), Seconds(8.0)));
}

#[test]
fn loops_are_clipped_to_the_song() {
    let region = LoopRegion::around(tempo(), Seconds(9.0), 4, Seconds(11.0));
    assert_eq!((region.start, region.end), (Seconds(8.0), Seconds(11.0)));
}

#[test]
fn wrapping_rewinds_to_a_lead_in_before_the_start() {
    let region = LoopRegion::around(tempo(), Seconds(5.0), 2, Seconds(100.0));
    let lead = Seconds(2.5);
    assert_eq!(region.wrap(Seconds(7.9), lead), None);
    assert_eq!(region.wrap(Seconds(8.0), lead), Some(Seconds(1.5)));
    assert_eq!(region.wrap(Seconds(8.3), lead), Some(Seconds(1.5)));
}

#[test]
fn weakest_section_is_the_worst_window_of_bars() {
    use keybard::practice::weakest_section;
    let acc = [
        Some(1.0),
        Some(0.9),
        Some(0.4),
        Some(0.5),
        Some(0.8),
        Some(1.0),
    ];
    assert_eq!(weakest_section(&acc, 2), Some(2), "bars 3-4 (0-based 2)");
    assert_eq!(weakest_section(&acc, 4), Some(1));
}

#[test]
fn weakest_section_ignores_unplayed_bars_and_perfect_runs() {
    use keybard::practice::weakest_section;
    assert_eq!(
        weakest_section(&[Some(1.0), None, Some(1.0)], 2),
        None,
        "nothing to fix"
    );
    assert_eq!(weakest_section(&[None, None], 2), None);
    // Empty bars don't drag a window down.
    assert_eq!(
        weakest_section(&[Some(0.9), None, None, Some(0.5)], 2),
        Some(2)
    );
    // Shorter songs than the window still get an answer.
    assert_eq!(weakest_section(&[Some(0.5)], 4), Some(0));
    // Ties go to the earliest.
    assert_eq!(
        weakest_section(&[Some(0.5), Some(0.5), Some(0.5)], 1),
        Some(0)
    );
}

#[test]
fn the_speed_trainer_steps_up_after_clean_passes_only() {
    use keybard::practice::{DRILL_START_SPEED, trainer_speed};
    assert!((trainer_speed(0.7, true) - 0.75).abs() < 1e-6);
    assert_eq!(
        trainer_speed(0.7, false),
        0.7,
        "a messy pass holds the speed"
    );
    assert_eq!(trainer_speed(0.98, true), 1.0, "capped at full tempo");
    assert_eq!(trainer_speed(1.0, true), 1.0);
    assert_eq!(
        trainer_speed(1.3, true),
        1.3,
        "never slows a fast player down"
    );
    const { assert!(DRILL_START_SPEED < 1.0) };
}
