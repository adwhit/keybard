//! Report MIDI playing as user activity to macOS power management.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use objc2_core_foundation::CFString;
use objc2_io_kit::{
    IOPMAssertionDeclareUserActivity, IOPMAssertionID, IOPMAssertionRelease, IOPMUserActiveType,
    kIOPMNullAssertionID,
};

use super::input::{InputSource, KeyEdge, NoteInput};

#[derive(Default)]
struct DisplayActivity {
    assertion: IOPMAssertionID,
    last_report: Option<Instant>,
}

impl Drop for DisplayActivity {
    fn drop(&mut self) {
        if self.assertion != kIOPMNullAssertionID {
            IOPMAssertionRelease(self.assertion);
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, report_midi_activity);
}

fn report_midi_activity(mut notes: MessageReader<NoteInput>, mut activity: Local<DisplayActivity>) {
    let pressed = notes
        .read()
        .filter(|note| note.source == InputSource::Midi && note.edge == KeyEdge::Down)
        .last()
        .is_some();
    if !pressed
        || activity
            .last_report
            .is_some_and(|last| last.elapsed() < Duration::from_secs(1))
    {
        return;
    }

    activity.last_report = Some(Instant::now());
    let name = CFString::from_str("Keybard MIDI keyboard");
    // SAFETY: The name is non-null and both it and the writable assertion ID
    // remain valid throughout the call.
    let result = unsafe {
        IOPMAssertionDeclareUserActivity(
            Some(&name),
            IOPMUserActiveType::Local,
            &mut activity.assertion,
        )
    };
    if result != 0 {
        tracing::debug!(result, "could not report MIDI activity to macOS");
    }
}
