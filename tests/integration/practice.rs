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
