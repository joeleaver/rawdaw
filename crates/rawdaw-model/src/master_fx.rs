//! Master-FX chain serialized types.
//!
//! The master chain is the ordered list of FX nodes that sit between the
//! engine's master gain and cpal. v1 ships one variant: a soft-clipper
//! (`SoftClipData`) that replaces the audible `-12 dB` master-gain
//! workaround currently in `rawdaw-app::audio`. Future variants
//! (`EqData`, `ReverbData`, …) extend [`MasterFxData`] without breaking
//! the project file format — additive enum growth is non-breaking under
//! serde's tagged-enum representation.
//!
//! The split is the same two-layer shape established by the synth-UI
//! integration milestone (U2/U3a): serialized data types live here in
//! `rawdaw-model` (pure f32/u32/Vec + serde, no `rawdaw-dsp` dependency),
//! runtime patch types + DSP nodes live in `rawdaw-fx` and convert via
//! `From<SoftClipData>` etc. Keeps `rawdaw-model` leaf with respect to
//! the audio crate stack.
//!
//! See `docs/master-fx-chain-plan.md` for the full milestone.

use serde::{Deserialize, Serialize};

/// Schema version for [`MasterChainData`] and its variants. Bumped any
/// time the on-disk shape of the chain or any [`MasterFxData`] variant
/// changes in a way an older build wouldn't understand. Independent of
/// [`crate::project::SCHEMA_VERSION`] — the chain version travels with
/// the chain, not the project, so a future Project schema bump that
/// touches an unrelated field doesn't force every variant to also
/// declare a new format_version.
pub const MASTER_CHAIN_FORMAT_VERSION: u32 = 1;

/// Schema version for [`SoftClipData`]. Same independence rule as
/// [`MASTER_CHAIN_FORMAT_VERSION`] — bumped only when the
/// `SoftClipData` shape itself changes.
pub const SOFT_CLIP_FORMAT_VERSION: u32 = 1;

/// One FX node's serialized parameters. Tagged-enum: each variant
/// carries the params for that FX kind. Add new variants by appending
/// to the enum; deserialization of unknown variants from a future
/// file fails cleanly (rejected by `Project::check_loadable`'s
/// future schema-version gate before we even get here).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MasterFxData {
    /// Memoryless tanh-knee soft-clipper. Currently the only FX kind.
    SoftClip(SoftClipData),
}

/// Soft-clipper params.
///
/// Threshold is the input level above which the tanh knee starts to
/// compress. `threshold = 1.0` is full-scale; `threshold = 0.7` (the
/// default) trips the knee at ~ -3.1 dBFS — generous headroom for the
/// pre-master synth output, with the tanh smoothly limiting transients
/// that would otherwise hard-clip in cpal.
///
/// `f32` only (no `rawdaw-dsp` types here — see module docs). The
/// runtime `SoftClipPatch` in `rawdaw-fx` carries the same field and
/// converts via `From<SoftClipData>`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SoftClipData {
    pub format_version: u32,
    pub threshold: f32,
}

impl Default for SoftClipData {
    fn default() -> Self {
        Self {
            format_version: SOFT_CLIP_FORMAT_VERSION,
            threshold: 0.7,
        }
    }
}

/// Ordered list of FX nodes. Evaluated in order from the engine's
/// master gain toward cpal — `fx[0]` runs first, `fx[last]`'s output
/// is what cpal reads.
///
/// Fresh projects ship a default chain with a single
/// [`MasterFxData::SoftClip`] entry so the safety net is on by
/// default (see [`Default`] impl). Users can explicitly set
/// `fx: vec![]` to clear the chain; that's distinct from the
/// missing-field-in-old-file case (which gets the default chain via
/// serde's `#[serde(default)]` attribute on `Project.master_chain`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MasterChainData {
    pub format_version: u32,
    pub fx: Vec<MasterFxData>,
}

impl Default for MasterChainData {
    fn default() -> Self {
        Self {
            format_version: MASTER_CHAIN_FORMAT_VERSION,
            fx: vec![MasterFxData::SoftClip(SoftClipData::default())],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_clip_default_threshold_is_0_7() {
        // The default threshold pins the audio-thread contract: the
        // runtime SoftClipNode in rawdaw-fx defaults to the same
        // value, so a default-constructed chain produces audibly the
        // same signal whether the runtime node is built from the
        // model default or from its own default. If these ever drift,
        // a freshly-loaded project would sound different from a
        // freshly-constructed one.
        let s = SoftClipData::default();
        assert_eq!(s.threshold, 0.7);
        assert_eq!(s.format_version, SOFT_CLIP_FORMAT_VERSION);
    }

    #[test]
    fn master_chain_default_has_single_softclip() {
        // The default chain is the safety net — single SoftClip with
        // default params. Pins the contract that fresh projects ship
        // with a non-empty chain.
        let c = MasterChainData::default();
        assert_eq!(c.format_version, MASTER_CHAIN_FORMAT_VERSION);
        assert_eq!(c.fx.len(), 1);
        match &c.fx[0] {
            MasterFxData::SoftClip(s) => assert_eq!(s.threshold, 0.7),
        }
    }

    #[test]
    fn master_chain_round_trips_empty() {
        // Explicitly-cleared chain (user removed the safety net)
        // must survive serialize → deserialize.
        let original = MasterChainData {
            format_version: MASTER_CHAIN_FORMAT_VERSION,
            fx: Vec::new(),
        };
        let serialized = ron::ser::to_string(&original).unwrap();
        let deserialized: MasterChainData = ron::de::from_str(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn master_chain_round_trips_single_entry() {
        let original = MasterChainData::default();
        let serialized = ron::ser::to_string(&original).unwrap();
        let deserialized: MasterChainData = ron::de::from_str(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn master_chain_round_trips_multi_entry() {
        // Forward-compatible: multi-entry chains must round-trip
        // even though v1 only has one variant — protects future
        // EQ/Reverb additions from a sneaky regression in the Vec
        // serialization shape.
        let original = MasterChainData {
            format_version: MASTER_CHAIN_FORMAT_VERSION,
            fx: vec![
                MasterFxData::SoftClip(SoftClipData {
                    format_version: SOFT_CLIP_FORMAT_VERSION,
                    threshold: 0.5,
                }),
                MasterFxData::SoftClip(SoftClipData {
                    format_version: SOFT_CLIP_FORMAT_VERSION,
                    threshold: 0.9,
                }),
            ],
        };
        let serialized = ron::ser::to_string(&original).unwrap();
        let deserialized: MasterChainData = ron::de::from_str(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }
}
