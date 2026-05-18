//! Oscillators — phase-accumulator wave generators + their
//! per-voice configuration types.

mod params;
mod wavetable;

pub use params::{note_offset_hz, WavetableOscParams};
pub use wavetable::{Wavetable, WavetableOsc, TABLE_LEN};
