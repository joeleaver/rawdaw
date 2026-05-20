//! On-disk bundle format: model `Project` + UI `ProjectOverlay`.
//!
//! Saved as a single RON value (pretty-printed) with a leading
//! `bundle_version` integer that's checked at load time. The
//! `Project` half itself carries `rawdaw_model::project::SCHEMA_VERSION`;
//! the two version numbers are **independent**. The overlay format
//! can grow without touching the model schema and vice versa.

use serde::{Deserialize, Serialize};

use rawdaw_model::project::Project;

use crate::overlay::ProjectOverlay;

/// Current bundle format version. Bumped whenever the
/// [`SavedBundle`] *shape* changes — distinct from
/// [`rawdaw_model::project::SCHEMA_VERSION`], which versions the
/// `Project` half independently.
///
/// v1 (current): `{ bundle_version, project, overlay }`.
pub const BUNDLE_VERSION: u32 = 1;

/// File extension for saved projects (no leading dot).
pub const FILE_EXTENSION: &str = "rawd";

/// The on-disk payload. RON-serialized as a single value.
///
/// Field order matters for the bundle-version peek path
/// ([`crate::project_io::load::load_from_path`] reads
/// `bundle_version` via a header-only view before deserializing the
/// full body, mirroring `Project::load`'s schema-version peek).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedBundle {
    /// Bundle format version. See [`BUNDLE_VERSION`].
    pub bundle_version: u32,
    /// Model project — track structure, sections, chord loops,
    /// patterns, arrangement, tempo. Saved/loaded through serde via
    /// [`Project::save`]'s underlying RON serializer.
    pub project: Project,
    /// UI-only decorations layered on the project — colors, library
    /// meta strings, per-cell realization decorations. See
    /// [`ProjectOverlay`] for the field shape.
    pub overlay: ProjectOverlay,
}

impl SavedBundle {
    /// Wrap an existing `(project, overlay)` pair in a current-version
    /// bundle ready for [`crate::project_io::save::save_to_path`].
    pub fn new(project: Project, overlay: ProjectOverlay) -> Self {
        Self {
            bundle_version: BUNDLE_VERSION,
            project,
            overlay,
        }
    }
}

/// Header-only view for the bundle-version peek. Mirrors
/// `Project`'s `SchemaHeader` trick — relies on serde's default
/// behavior of ignoring unknown fields so the partial deserialize
/// succeeds regardless of how the body has shifted.
#[derive(Deserialize)]
pub(crate) struct BundleHeader {
    pub bundle_version: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    use rawdaw_model::fixtures::build_round1_project;

    #[test]
    fn bundle_version_constant_is_one() {
        // Pin the version so a casual bump doesn't silently break
        // older files. Real version increments need a matching
        // load-side migration.
        assert_eq!(BUNDLE_VERSION, 1);
    }

    #[test]
    fn ron_round_trip_preserves_project_and_overlay() {
        // The headline contract: serialize → parse → equal value.
        // Uses the round-1 fixture so the bundle exercises the real
        // shape (every track, every chord loop, every section).
        let (project, _) = build_round1_project();
        let overlay = ProjectOverlay {
            project_name: "Test Song".into(),
            ..Default::default()
        };
        let bundle = SavedBundle::new(project, overlay);

        let serialized = ron::ser::to_string_pretty(&bundle, ron::ser::PrettyConfig::default())
            .expect("serialize succeeds");
        let parsed: SavedBundle =
            ron::de::from_str(&serialized).expect("parse succeeds");
        assert_eq!(parsed, bundle);
    }

    #[test]
    fn header_peek_reads_bundle_version_only() {
        // The bundle-version header is read via a partial deserialize
        // — confirm it doesn't require the body to be present. This
        // matches `Project::load`'s schema-version peek pattern.
        let header_ron = "(bundle_version: 1, project: (), overlay: ())";
        let header: BundleHeader =
            ron::de::from_str(header_ron).expect("header peek succeeds");
        assert_eq!(header.bundle_version, 1);
    }
}
