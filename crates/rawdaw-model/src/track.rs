//! Tracks: a project-global list of parts. Each track is either pitched
//! (carrying a role) or drum (carrying a kit reference).
//!
//! Mixer placement is a stub here; the audio engine crate will define mixer
//! details. The model layer only needs to know that a track has a routing
//! identity.

use serde::{Deserialize, Serialize};

use crate::id::{DrumKitId, InstrumentId, TrackId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub kind: TrackKind,
    pub instrument: InstrumentId,
    pub mixer: MixerPlacement,
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
