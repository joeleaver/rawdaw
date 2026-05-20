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
/// When this changes, also add a migration entry to [`Project::migrate_to_current`]
/// plus a `loadable_versions` entry in [`Project::check_loadable`], update the
/// test `loading_an_unsupported_version_fails`, and document the change here.
///
/// # Version history
///
/// - **v1** (initial): `schema_version, default_key, tempo_map, patterns,
///   chord_loops, sections, drum_kits, tracks, arrangement, id_allocators`.
/// - **v2** (composition-writability C4): adds `name: String`. v1 files
///   migrate via the `serde(default = "default_project_name")` attribute
///   on `Project.name` (defaults missing field to `"Untitled"`) and
///   [`Project::migrate_to_current`] bumps the in-memory version.
pub const SCHEMA_VERSION: u32 = 2;

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

    /// User-facing project name. Shown in the top bar; seeds the
    /// default filename in "Save As…". Added in schema v2; v1 files
    /// load with this defaulted to `"Untitled"` via
    /// [`default_project_name`].
    #[serde(default = "default_project_name")]
    pub name: String,

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

/// Default value for [`Project::name`] when loading a v1 file (which
/// doesn't carry the field) and when [`Project::new`] builds an empty
/// project. Kept in sync with `composition-writability-plan.md` C4.
pub fn default_project_name() -> String {
    "Untitled".to_string()
}

impl Project {
    pub fn new(default_key: Scale) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            name: default_project_name(),
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

    /// Deserialize a project from RON, with migration support for
    /// older known versions.
    ///
    /// Loading dispatches in three steps:
    /// 1. Peek the on-disk `schema_version` via a header-only
    ///    deserialize. A version this build doesn't recognize returns
    ///    [`LoadError::UnsupportedSchemaVersion`] before any heavy
    ///    parsing — see [`Project::check_loadable`].
    /// 2. Deserialize the full body. Missing fields added in newer
    ///    versions are populated by `serde(default = ...)` attributes
    ///    on the field declarations.
    /// 3. Run [`Project::migrate_to_current`] to bump the in-memory
    ///    `schema_version` so any follow-up `save` writes the current
    ///    format.
    pub fn load(s: &str) -> Result<Self, LoadError> {
        let header: SchemaHeader = ron::de::from_str(s).map_err(LoadError::Parse)?;
        Self::check_loadable(header.schema_version)?;
        let project: Project = ron::de::from_str(s).map_err(LoadError::Parse)?;
        Ok(Self::migrate_to_current(project))
    }

    /// Return `Ok` if `version` is a version this build knows how to
    /// load (current [`SCHEMA_VERSION`] plus any older versions
    /// covered by [`Project::migrate_to_current`]). Returns
    /// [`LoadError::UnsupportedSchemaVersion`] otherwise.
    ///
    /// Exposed so callers that deserialize a `Project` *outside* of
    /// [`Project::load`] (notably the app's `project_io` bundle
    /// loader, which deserializes the project as a sub-value of a
    /// `SavedBundle`) can run the same version check + migration
    /// without duplicating the dispatch logic.
    pub fn check_loadable(version: u32) -> Result<(), LoadError> {
        match version {
            1 | 2 => Ok(()),
            v => Err(LoadError::UnsupportedSchemaVersion {
                found: v,
                expected: SCHEMA_VERSION,
            }),
        }
    }

    /// Apply any required migrations to bring `project` up to the
    /// current [`SCHEMA_VERSION`]. Idempotent — re-running on an
    /// already-current project is a no-op.
    ///
    /// Migration entries (newest first):
    ///
    /// - **v1 → v2** (composition-writability C4): `Project.name` was
    ///   added. v1 RON files don't carry the field; serde's
    ///   `default = "default_project_name"` attribute on `Project.name`
    ///   populates it with `"Untitled"` during the deserialize step
    ///   that precedes this call. The migration step here only needs
    ///   to bump the in-memory `schema_version`.
    pub fn migrate_to_current(mut project: Project) -> Project {
        if project.schema_version == 1 {
            project.schema_version = 2;
        }
        debug_assert_eq!(
            project.schema_version, SCHEMA_VERSION,
            "migrate_to_current must leave the project at SCHEMA_VERSION; \
             missing migration entry for v{}?",
            project.schema_version,
        );
        project
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
