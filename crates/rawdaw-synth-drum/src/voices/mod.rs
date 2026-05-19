//! Per-drum-type voice implementations.
//!
//! Each voice type owns its own pool inside `DrumSynthNode`. Drum
//! synthesis is one-shot — `note_off` is a no-op since drums decay
//! naturally from `note_on`. `is_active()` reports whether the amp
//! envelope hasn't yet returned to idle.

mod hat;
mod kick;
mod snare;

pub use hat::HatVoice;
pub use kick::KickVoice;
pub use snare::SnareVoice;
