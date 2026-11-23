//! keybard: a falling-notes piano trainer.

pub mod app;
pub mod builtin;
pub mod error;
pub mod layout;
pub mod midi_file;
pub mod music;
pub mod performance;
pub mod records;
pub mod song;

pub use error::Error;
