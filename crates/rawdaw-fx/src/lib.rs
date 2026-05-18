//! rawdaw-fx — `AudioNode` implementations for effects.
//!
//! v0 has just one node — [`GainNode`] — used as the master output
//! attenuator so multi-voice synth output doesn't pre-clip into cpal.
//! EQ / reverb / delay / saturation land here as separate node types
//! over subsequent passes.
//!
//! Parameter automation is not yet wired through the engine's
//! command queue; nodes take their parameters at construction and
//! the host can't change them on the fly. When the parameter system
//! grows, this crate's nodes will be the first consumers.

#![forbid(unsafe_code)]

mod gain;

pub use gain::GainNode;
