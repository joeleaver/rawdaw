//! rawdaw-fx — `AudioNode` implementations for effects.
//!
//! v0 had just one node — [`GainNode`] — used as the master output
//! attenuator so multi-voice synth output doesn't pre-clip into
//! cpal. X2 of the master-FX-chain milestone adds [`SoftClipNode`],
//! the first FX node with end-to-end Param event support
//! (encode/decode via [`SoftClipParam`], `Arc<Mutex<Patch>>` +
//! `Arc<AtomicU64>` publishers, runtime patch with `From<SoftClipData>`).
//! EQ / reverb / delay land as additional variants over later
//! passes following the same shape.

#![forbid(unsafe_code)]

mod gain;
mod softclip;

pub use gain::GainNode;
pub use softclip::{
    SoftClipNode, SoftClipParam, SoftClipPatch, SoftClipPublishers, DEFAULT_THRESHOLD,
    MAX_THRESHOLD, MIN_THRESHOLD, SOFT_CLIP_FX_KIND,
};
