//! Tracks: a project-global list of parts. Each track is either pitched
//! (carrying a role) or drum (carrying a kit reference).
//!
//! Mixer placement is a stub here; the audio engine crate will define mixer
//! details. The model layer only needs to know that a track has a routing
//! identity.

use serde::{Deserialize, Serialize};

use crate::id::{DrumKitId, InstrumentId, TrackId};
use crate::patch::SynthAssignment;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub kind: TrackKind,
    pub instrument: InstrumentId,
    pub mixer: MixerPlacement,
    /// Which synth backs this track + its patch data. Defaults pair
    /// with `kind` via [`SynthAssignment::default_for_kind`] when the
    /// track is built with [`Track::new`]. The (kind, synth) invariant
    /// can be checked at runtime with [`Self::kind_matches_synth`].
    pub synth: SynthAssignment,
}

impl Track {
    /// Build a track with a default synth assignment for its kind.
    /// `Pitched` tracks get the default wavetable patch; `Drum` tracks
    /// get the default drum patch. Custom patches can be installed by
    /// overwriting [`Self::synth`] directly afterwards.
    pub fn new(
        id: TrackId,
        name: String,
        kind: TrackKind,
        instrument: InstrumentId,
        mixer: MixerPlacement,
    ) -> Self {
        let synth = SynthAssignment::default_for_kind(&kind);
        Self {
            id,
            name,
            kind,
            instrument,
            mixer,
            synth,
        }
    }

    /// Whether this track's `synth` variant matches its `kind`.
    /// Audio-graph builders should `debug_assert!` this at the
    /// configuration boundary so a mismatched assignment (which can
    /// only happen via direct field mutation, not [`Self::new`])
    /// surfaces in debug builds.
    pub fn kind_matches_synth(&self) -> bool {
        self.synth.matches_kind(&self.kind)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TrackKind {
    Pitched { role: Role },
    Drum { kit: DrumKitId },
}

/// Compositional role of a pitched track. Hints at default register, default
/// voicing, and how realization should interpret degree-based events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Bass,
    Voicing,
    Arp,
    Melodic,
    Pad,
    Countermelody,
    Other,
}

/// Stub for mixer placement. The engine crate will own the real channel-strip
/// model; the data model only needs an opaque routing identity for now.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MixerPlacement {
    /// Display order in the mixer view.
    pub display_index: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::drum::DrumPatchData;
    use crate::patch::wavetable::WavetablePatchData;

    fn pitched_kind() -> TrackKind {
        TrackKind::Pitched { role: Role::Bass }
    }

    fn drum_kind() -> TrackKind {
        TrackKind::Drum {
            kit: DrumKitId::new(0),
        }
    }

    fn make_track(kind: TrackKind) -> Track {
        Track::new(
            TrackId::new(1),
            "test".into(),
            kind,
            InstrumentId::new(0),
            MixerPlacement::default(),
        )
    }

    #[test]
    fn new_pitched_picks_wavetable_default() {
        let t = make_track(pitched_kind());
        assert!(matches!(t.synth, SynthAssignment::Wavetable(_)));
        assert!(t.kind_matches_synth());
    }

    #[test]
    fn new_drum_picks_drum_default() {
        let t = make_track(drum_kind());
        assert!(matches!(t.synth, SynthAssignment::Drum(_)));
        assert!(t.kind_matches_synth());
    }

    #[test]
    fn manually_mismatched_assignment_fails_kind_check() {
        // Track::new always pairs correctly, but the synth field is
        // pub so direct mutation could break the invariant. The
        // kind-check method catches it; consumers debug_assert at the
        // audio-graph boundary so the mismatch surfaces in tests.
        let mut t = make_track(pitched_kind());
        t.synth = SynthAssignment::Drum(DrumPatchData::default());
        assert!(!t.kind_matches_synth());

        let mut t = make_track(drum_kind());
        t.synth = SynthAssignment::Wavetable(WavetablePatchData::default());
        assert!(!t.kind_matches_synth());
    }

    #[test]
    fn ron_round_trip_preserves_track() {
        let t = make_track(pitched_kind());
        let s = ron::ser::to_string(&t).expect("serialize");
        let restored: Track = ron::de::from_str(&s).expect("deserialize");
        assert_eq!(t, restored);
    }
}
