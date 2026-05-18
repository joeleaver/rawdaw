//! rawdaw-dsp — leaf-level DSP primitives.
//!
//! Pure-Rust signal-processing building blocks that the synth and FX
//! crates compose into `AudioNode` implementations. Nothing in this
//! crate knows about the engine's command / event queues or the model
//! layer's domain types; this is a leaf in the dependency graph so
//! every higher-level crate can pull it in without inheriting baggage.
//!
//! ## Modules
//!
//! Each primitive lives in its own module so the crate stays
//! navigable as it grows.
//!
//! - **Oscillators** — wavetable, sine, …
//! - **Envelopes** — ADSR, …
//! - **Filters** — state-variable lowpass, …
//! - **LFOs** — sine, …
//! - **Voicing** — generic voice pool for polyphonic synths.
//!
//! ## RT-safety contract
//!
//! Every public type's per-sample / per-block "tick" method must be
//! allocation-free and lock-free in the steady state. Construction +
//! `prepare()`-style methods may allocate. Public types document any
//! deviation from this contract.

#![forbid(unsafe_code)]

pub mod envelope;
pub mod filter;
pub mod lfo;
pub mod modulation;
pub mod noise;
pub mod oscillator;
pub mod voicing;

pub use envelope::{Adsr, AdsrParams, AdsrStage, PitchEnvelope};
pub use filter::{SvfHighpass, SvfLowpass};
pub use lfo::SineLfo;
pub use modulation::{
    ModDestination, ModMatrix, ModSlot, ModSource, Modulations, NUM_OSCS_PER_VOICE,
};
pub use noise::NoiseSource;
pub use oscillator::{note_offset_hz, Wavetable, WavetableOsc, WavetableOscParams, TABLE_LEN};
pub use voicing::{Voice, VoicePool};
