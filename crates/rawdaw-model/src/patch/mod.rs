//! Serialized synth patch types.
//!
//! Every track in a [`Project`](crate::Project) carries a
//! [`SynthAssignment`] describing which synth backs it and what patch
//! data to load. This module owns the on-disk shape: each synth crate
//! defines a matching runtime patch type in its own source, and
//! `From` impls in the synth crate convert serialized data to runtime
//! (see U3 of `docs/synth-ui-integration-plan.md`).
//!
//! ## Why two layers
//!
//! The runtime patch types in `rawdaw-dsp` / the synth crates hold
//! efficient in-memory representations (no enum tag indirection on hot
//! paths, fixed-size arrays sized exactly to the voice's needs). The
//! serialized mirrors here use serde and explicit `format_version`s so
//! the on-disk schema can evolve without coupling to the dsp layout —
//! a new mod-source variant in the synth crate doesn't have to break
//! every old project file. See the U0 design decisions in the synth-
//! UI integration plan for the trade-off discussion.
//!
//! ## Kind / synth invariant
//!
//! [`SynthAssignment`] doesn't statically enforce that a
//! `TrackKind::Pitched` track gets a `Wavetable` patch — making the
//! `Track` type generic would balloon `Project::tracks` into a
//! heterogeneous collection. The invariant is enforced by
//! [`Track::new`](crate::Track::new) (which picks the right default
//! from the kind) plus
//! [`Track::kind_matches_synth`](crate::Track::kind_matches_synth) +
//! a runtime debug-assert in the audio-graph builder.

pub mod drum;
pub mod wavetable;

use serde::{Deserialize, Serialize};

use crate::track::TrackKind;
use drum::DrumPatchData;
use wavetable::WavetablePatchData;

/// Which synth backs a track + its patch.
///
/// Variants line up with `TrackKind`: `Pitched` tracks get
/// `Wavetable`, `Drum` tracks get `Drum`. Future synths (physical
/// modeller, sample player) add their own variants and grow the match
/// in the audio-graph builder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SynthAssignment {
    Wavetable(WavetablePatchData),
    Drum(DrumPatchData),
}

impl SynthAssignment {
    /// The default assignment for a given track kind. Used by
    /// [`Track::new`](crate::Track::new) to pair kind and patch
    /// correctly at construction time.
    pub fn default_for_kind(kind: &TrackKind) -> Self {
        match kind {
            TrackKind::Pitched { .. } => Self::Wavetable(WavetablePatchData::default()),
            TrackKind::Drum { .. } => Self::Drum(DrumPatchData::default()),
        }
    }

    /// Whether this assignment is compatible with the given kind.
    /// Used by [`Track::kind_matches_synth`](crate::Track::kind_matches_synth)
    /// and the audio-graph builder's debug-assert.
    pub fn matches_kind(&self, kind: &TrackKind) -> bool {
        matches!(
            (self, kind),
            (Self::Wavetable(_), TrackKind::Pitched { .. })
                | (Self::Drum(_), TrackKind::Drum { .. }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::DrumKitId;
    use crate::track::Role;

    #[test]
    fn default_for_pitched_kind_is_wavetable() {
        let kind = TrackKind::Pitched { role: Role::Bass };
        let synth = SynthAssignment::default_for_kind(&kind);
        assert!(matches!(synth, SynthAssignment::Wavetable(_)));
        assert!(synth.matches_kind(&kind));
    }

    #[test]
    fn default_for_drum_kind_is_drum() {
        let kind = TrackKind::Drum {
            kit: DrumKitId::new(0),
        };
        let synth = SynthAssignment::default_for_kind(&kind);
        assert!(matches!(synth, SynthAssignment::Drum(_)));
        assert!(synth.matches_kind(&kind));
    }

    #[test]
    fn wavetable_does_not_match_drum_kind() {
        let synth = SynthAssignment::Wavetable(WavetablePatchData::default());
        let drum_kind = TrackKind::Drum {
            kit: DrumKitId::new(0),
        };
        assert!(!synth.matches_kind(&drum_kind));
    }

    #[test]
    fn drum_does_not_match_pitched_kind() {
        let synth = SynthAssignment::Drum(DrumPatchData::default());
        let pitched_kind = TrackKind::Pitched {
            role: Role::Melodic,
        };
        assert!(!synth.matches_kind(&pitched_kind));
    }

    #[test]
    fn ron_round_trip_preserves_synth_assignment() {
        let pitched = SynthAssignment::default_for_kind(&TrackKind::Pitched {
            role: Role::Pad,
        });
        let s = ron::ser::to_string(&pitched).expect("serialize");
        let restored: SynthAssignment = ron::de::from_str(&s).expect("deserialize");
        assert_eq!(pitched, restored);

        let drum = SynthAssignment::default_for_kind(&TrackKind::Drum {
            kit: DrumKitId::new(7),
        });
        let s = ron::ser::to_string(&drum).expect("serialize");
        let restored: SynthAssignment = ron::de::from_str(&s).expect("deserialize");
        assert_eq!(drum, restored);
    }
}
