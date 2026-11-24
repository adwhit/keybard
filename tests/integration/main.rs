//! All integration tests live in one binary: each Bevy-linked test binary is
//! ~100MB, so separate binaries cost seconds of linking and loading apiece.

mod achievements;
mod app;
mod challenge;
mod chord_keys;
mod chords;
mod dynamics;
mod fingering;
mod glyph;
mod key;
mod layout;
mod midi_file;
mod midi_port;
mod music;
mod notation;
mod performance;
mod practice;
mod records;
mod render;
mod saves;
mod song;
mod streak;
mod synth;
