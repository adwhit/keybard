//! keybard: a falling-notes piano trainer.

pub mod app;
pub mod builtin;
pub mod error;
pub mod layout;
pub mod midi_file;
pub mod music;
pub mod performance;
pub mod practice;
pub mod records;
pub mod render;
pub mod song;
pub mod synth;

pub use error::Error;
