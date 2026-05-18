//! Polyphonic voice management — allocation, stealing, NoteOff
//! matching, voice age tracking.

mod pool;

pub use pool::{Voice, VoicePool};
