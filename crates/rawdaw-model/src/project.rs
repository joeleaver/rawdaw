//! Project: top-level container.
//!
//! Owns the library of reusable objects (patterns, chord loops, sections,
//! drum kits), the project-wide tracks, the arrangement, the tempo map, and
//! the project default key.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::chord::ChordLoop;
use crate::id::{
    ChordLoopId, DrumKitId, NoteId, NoteOverrideId, PatternId, SectionId, SectionRefId, TrackId,
};
use crate::pattern::Pattern;
use crate::scale::Scale;
use crate::section::{Arrangement, Section};
use crate::tempo::TempoMap;
use crate::track::Track;

/// The current on-disk schema version. Bumped any time the serialized shape
/// of `Project` (or anything reachable from it) changes in a way that an
/// older version of rawdaw wouldn't understand.
///
/// When this changes, also: add a migration entry, update the test
/// `loading_an_unsupported_version_fails`, and document the change.
pub const SCHEMA_VERSION: u32 = 1;

/// Top-level project state.
///
/// Library maps use `BTreeMap` rather than `HashMap` so iteration order is
/// deterministic — important for project-file serialization, since project
/// files will live in version control and need to diff cleanly.
///
/// `schema_version` is serialized first so it's visible at a glance and
/// future migration tooling can identify the file's version without parsing
/// the whole body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: u32,

    /// Project-wide default key. Sections inherit unless they override.
    pub default_key: Scale,
    pub tempo_map: TempoMap,

    pub patterns: BTreeMap<PatternId, Pattern>,
    pub chord_loops: BTreeMap<ChordLoopId, ChordLoop>,
    pub sections: BTreeMap<SectionId, Section>,
    pub drum_kits: BTreeMap<DrumKitId, DrumKitStub>,

    /// Tracks are project-global. Ordering controls mixer display order
    /// (via the `MixerPlacement` indices on each track).
    pub tracks: Vec<Track>,

    pub arrangement: Arrangement,

    /// ID allocators. NoteId and NoteOverrideId are durable (never reused).
    /// Other IDs are also monotonic but the durability requirement is less
    /// strict — they just need to be unique within a project.
    pub id_allocators: IdAllocators,
}

/// Monotonic ID counters. Kept in one struct so serialization is local.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdAllocators {
    pub next_note_id: u64,
    pub next_note_override_id: u64,
    pub next_pattern_id: u64,
    pub next_section_id: u64,
    pub next_section_ref_id: u64,
    pub next_track_id: u64,
    pub next_chord_loop_id: u64,
    pub next_drum_kit_id: u64,
    pub next_activation_entry_id: u64,
    pub next_instrument_id: u64,
}

impl IdAllocators {
    pub fn alloc_note(&mut self) -> NoteId {
        let id = NoteId::new(self.next_note_id);
        self.next_note_id = self.next_note_id.checked_add(1).expect("NoteId overflow");
        id
    }

    pub fn alloc_note_override(&mut self) -> NoteOverrideId {
        let id = NoteOverrideId::new(self.next_note_override_id);
        self.next_note_override_id = self
            .next_note_override_id
            .checked_add(1)
            .expect("NoteOverrideId overflow");
        id
    }

    pub fn alloc_pattern(&mut self) -> PatternId {
        let id = PatternId::new(self.next_pattern_id);
        self.next_pattern_id += 1;
        id
    }

    pub fn alloc_section(&mut self) -> SectionId {
        let id = SectionId::new(self.next_section_id);
        self.next_section_id += 1;
        id
    }

    pub fn alloc_section_ref(&mut self) -> SectionRefId {
        let id = SectionRefId::new(self.next_section_ref_id);
        self.next_section_ref_id += 1;
        id
    }

    pub fn alloc_track(&mut self) -> TrackId {
        let id = TrackId::new(self.next_track_id);
        self.next_track_id += 1;
        id
    }

    pub fn alloc_chord_loop(&mut self) -> ChordLoopId {
        let id = ChordLoopId::new(self.next_chord_loop_id);
        self.next_chord_loop_id += 1;
        id
    }
}

impl Project {
    pub fn new(default_key: Scale) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            default_key,
            tempo_map: TempoMap::default(),
            patterns: BTreeMap::new(),
            chord_loops: BTreeMap::new(),
            sections: BTreeMap::new(),
            drum_kits: BTreeMap::new(),
            tracks: Vec::new(),
            arrangement: Arrangement::default(),
            id_allocators: IdAllocators::default(),
        }
    }

    /// Serialize the project to a pretty-printed RON string.
    pub fn save(&self) -> Result<String, SaveError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).map_err(SaveError::Ron)
    }

    /// Deserialize a project from RON, verifying the schema version matches
    /// [`SCHEMA_VERSION`].
    ///
    /// **Migration is not yet implemented.** Loading a file with a different
    /// schema version returns `LoadError::UnsupportedSchemaVersion`. When we
    /// add migrations, they'll dispatch by version after a lightweight peek
    /// that doesn't require parsing the full body.
    pub fn load(s: &str) -> Result<Self, LoadError> {
        // First peek at just the version. If the body has shifted in a way
        // that the *full* `Project` deserialize can't handle, we still want
        // to give the user a clean "wrong version" error instead of a parse
        // error from somewhere deep inside the file.
        let header: SchemaHeader = ron::de::from_str(s).map_err(LoadError::Parse)?;
        if header.schema_version != SCHEMA_VERSION {
            return Err(LoadError::UnsupportedSchemaVersion {
                found: header.schema_version,
                expected: SCHEMA_VERSION,
            });
        }
        ron::de::from_str(s).map_err(LoadError::Parse)
    }
}

/// Header-only view of a project file, used to peek at the schema version
/// before doing a full deserialization. Relies on serde's default behavior
/// of ignoring unknown fields.
#[derive(Deserialize)]
#[allow(dead_code)]
struct SchemaHeader {
    schema_version: u32,
}

#[derive(Debug)]
pub enum SaveError {
    Ron(ron::Error),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ron(e) => write!(f, "failed to serialize project: {e}"),
        }
    }
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Ron(e) => Some(e),
        }
    }
}

#[derive(Debug)]
pub enum LoadError {
    Parse(ron::de::SpannedError),
    UnsupportedSchemaVersion { found: u32, expected: u32 },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "failed to parse project: {e}"),
            Self::UnsupportedSchemaVersion { found, expected } => write!(
                f,
                "unsupported project schema version: file is v{found}, this build understands v{expected}",
            ),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(e) => Some(e),
            Self::UnsupportedSchemaVersion { .. } => None,
        }
    }
}

/// Placeholder for drum-kit data. Full kit definition (TOML parsing, voice
/// mapping, output declarations) will live in `rawdaw-drumkits`. This stub
/// holds enough identity for the model to reference kits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrumKitStub {
    pub id: DrumKitId,
    pub name: String,
}
