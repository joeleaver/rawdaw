//! Built-in audio nodes.
//!
//! v1 ships:
//! - `SilenceNode` and `ImpulseNode`: trivial test nodes used to verify
//!   the engine's plumbing without depending on real DSP.
//! - `SineNode`: a polyphonic sine oscillator. The first real synth and
//!   the audible end of the realization → engine pipeline.

pub mod impulse;
pub mod silence;
pub mod sine;

pub use impulse::ImpulseNode;
pub use silence::SilenceNode;
pub use sine::SineNode;
