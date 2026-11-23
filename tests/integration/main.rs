//! All integration tests live in one binary: each Bevy-linked test binary is
//! ~100MB, so separate binaries cost seconds of linking and loading apiece.

mod app;
mod layout;
mod midi_file;
mod music;
mod performance;
mod practice;
mod records;
mod song;
