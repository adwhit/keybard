//! All integration tests live in one binary: each Bevy-linked test binary is
//! ~100MB, so separate binaries cost seconds of linking and loading apiece.

mod app;
mod fingering;
mod key;
mod layout;
mod midi_file;
mod music;
mod notation;
mod performance;
mod practice;
mod records;
mod render;
mod song;
mod streak;
mod synth;
