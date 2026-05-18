//! Built-in audio nodes.
//!
//! v1 ships:
//! - `SilenceNode` and `ImpulseNode`: trivial test nodes used to verify
//!   the engine's plumbing without depending on real DSP.
//! - `SineNode`: a polyphonic sine oscillator. The first real synth and
//!   the audible end of the realization → engine pipeline.
//! - `MixerNode`: stereo summing mixer (N stereo inputs → 1 stereo
//!   output). The project master in the engine-wiring milestone's
//!   per-track sine graph.

pub mod impulse;
pub mod mixer;
pub mod silence;
pub mod sine;

pub use impulse::ImpulseNode;
pub use mixer::MixerNode;
pub use silence::SilenceNode;
pub use sine::SineNode;
