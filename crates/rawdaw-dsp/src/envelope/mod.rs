//! Envelopes — time-varying amplitude shapers.

mod adsr;
mod pitch;

pub use adsr::{Adsr, AdsrParams, AdsrStage};
pub use pitch::PitchEnvelope;
