//! rawdaw composition data model.
//!
//! This crate is pure data and pure functions: no audio I/O, no UI, no
//! threading. The realization pass (see `docs/design/realization.md`) lives
//! here too; it consumes the data model and produces sample-timed MIDI events.
//!
//! Crate is `#![forbid(unsafe_code)]` at the workspace level.

pub mod activation;
pub mod chord;
pub mod id;
pub mod pattern;
pub mod pitch;
pub mod project;
pub mod realize;
pub mod scale;
pub mod section;
pub mod tempo;
pub mod time;
pub mod track;

pub use activation::*;
pub use chord::*;
pub use id::*;
pub use pattern::*;
pub use pitch::*;
pub use project::*;
pub use realize::{
    realize, Midi2Message, MidiChannel, Provenance, ResolvedChord, TimedEvent, U16Velocity,
};
pub use scale::*;
pub use section::*;
pub use tempo::*;
pub use time::*;
pub use track::*;
