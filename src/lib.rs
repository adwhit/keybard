//! keybard: a falling-notes piano trainer.

pub mod achievements;
pub mod app;
pub mod builtin;
pub mod error;
pub mod fingering;
pub mod key;
pub mod layout;
pub mod midi_file;
pub mod midi_port;
pub mod music;
pub mod notation;
pub mod performance;
pub mod practice;
pub mod records;
pub mod render;
pub mod song;
pub mod streak;
pub mod synth;

pub use error::Error;
