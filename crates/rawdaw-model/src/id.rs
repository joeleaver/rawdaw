//! Newtype IDs for the model.
//!
//! All IDs are `u64` under the hood. `NoteId` and `NoteOverrideId` are durably
//! allocated (never reused) so per-note overrides survive deletes and undo.
//! Other IDs are also monotonic but the durability requirement is less strict.

use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u64);

        impl $name {
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

id_type!(
    /// Stable identifier for a single event inside a pattern body.
    /// Allocated monotonically per project; never reused, even after the
    /// event is deleted, so per-note overrides cannot silently re-bind.
    NoteId
);

id_type!(
    /// Identifier for a per-note override on an activation.
    NoteOverrideId
);

id_type!(PatternId);
id_type!(SectionId);
id_type!(TrackId);
id_type!(ChordLoopId);
id_type!(DrumKitId);
id_type!(ActivationEntryId);
id_type!(InstrumentId);

id_type!(
    /// Identifier for a `SectionRef` placement in the arrangement.
    /// Distinct from `SectionId`: many `SectionRef`s can point to one `SectionId`.
    SectionRefId
);

/// User-named variant identifier (e.g. "main", "fill", "build", "final").
/// String-based because users name and rename these in the UI.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VariantId(pub String);

impl VariantId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub const BASE_STR: &'static str = "base";
    pub const MAIN_STR: &'static str = "main";

    pub fn base() -> Self {
        Self(Self::BASE_STR.to_owned())
    }

    pub fn main() -> Self {
        Self(Self::MAIN_STR.to_owned())
    }
}

impl From<&str> for VariantId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for VariantId {
    fn from(s: String) -> Self {
        Self(s)
    }
}
