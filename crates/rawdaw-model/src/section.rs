//! Sections: named structural units in the song. Sections have a base body
//! and a flat set of named variants that sparse-override the base. Activation
//! entries within a section are keyed by `TrackId` (activation-based ownership;
//! tracks themselves are project-global).
//!
//! See `docs/design/section-variants.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::activation::ActivationEntry;
use crate::id::{ChordLoopId, SectionId, SectionRefId, TrackId, VariantId};
use crate::scale::Scale;
use crate::time::{BarRange, MusicalTime};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: SectionId,
    pub name: String,
    pub base: SectionBody,
    pub variants: BTreeMap<VariantId, SectionVariantOverride>,
    /// The variant selected when a `SectionRef` is created without an explicit
    /// choice. Typically [`VariantId::BASE`].
    pub default_variant: VariantId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionBody {
    pub duration_bars: u32,
    pub scale_override: Option<Scale>,
    /// Ordered list of chord loops scheduled across the section's bar range.
    /// Ranges should not overlap. Uncovered ranges have no chord context.
    pub chord_loops: Vec<(BarRange, ChordLoopId)>,
    pub activations: BTreeMap<TrackId, ActivationEntry>,
}

/// A sparse override on top of a section's base body. Each top-level field is
/// either inherited (absent / `None`) or replaced wholesale.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SectionVariantOverride {
    pub duration_bars: Option<u32>,
    /// Outer `Option`: `None` inherits the base's value. Inner `Option`:
    /// `Some(None)` explicitly clears the base's scale override; `Some(Some(s))`
    /// sets a different scale.
    pub scale_override: Option<Option<Scale>>,
    pub chord_loops: Option<Vec<(BarRange, ChordLoopId)>>,
    /// Sparse override per track. Absent key = inherit base's activation
    /// (if any) for that track.
    pub activations: BTreeMap<TrackId, ActivationOverride>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActivationOverride {
    /// Replace the base's activation for this track with a different one.
    Replace(ActivationEntry),
    /// This track is silent in this variant (overrides any base activation).
    Silent,
}

// ---------- Arrangement ----------

/// A placement of a section variant in the song's timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionRef {
    pub id: SectionRefId,
    pub section: SectionId,
    pub variant: VariantId,
    /// Absolute start position in the arrangement.
    pub start: MusicalTime,
}

/// The arrangement: an ordered list of section placements.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Arrangement {
    pub sections: Vec<SectionRef>,
}
