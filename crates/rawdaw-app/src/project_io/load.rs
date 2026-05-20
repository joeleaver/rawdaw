//! Read + version-check a [`SavedBundle`] from disk and apply it to
//! the running app.

use std::fs;
use std::path::Path;
use std::rc::Rc;

use rawdaw_model::project::{LoadError as ProjectLoadError, SCHEMA_VERSION};

use crate::audio::AudioResources;
use crate::state::AppState;

use super::bundle::{BundleHeader, SavedBundle, BUNDLE_VERSION};

/// Read `path` as RON and deserialize into a [`SavedBundle`],
/// version-checked at two layers:
///
/// 1. **Bundle version** — peek `bundle_version` via a header-only
///    deserialize and compare to [`BUNDLE_VERSION`]. Mismatch
///    surfaces [`LoadBundleError::UnsupportedBundleVersion`].
/// 2. **Project schema version** — the inner `Project` deserialize
///    runs [`rawdaw_model::project::Project::load`]'s schema check
///    implicitly because we round-trip the full body through
///    `ron::de::from_str`. A mismatch surfaces as
///    [`LoadBundleError::Project`] wrapping the typed model error.
///
/// On error the caller's live project is **not** touched; the live
/// state only changes after a successful parse + version check + a
/// follow-up call to [`apply_loaded_bundle`].
pub fn load_from_path(path: &Path) -> Result<SavedBundle, LoadBundleError> {
    let raw = fs::read_to_string(path).map_err(LoadBundleError::Io)?;
    // Peek bundle_version first. A mismatch is more helpful to
    // surface than a deep parse error inside the body.
    let header: BundleHeader = ron::de::from_str(&raw).map_err(LoadBundleError::Parse)?;
    if header.bundle_version != BUNDLE_VERSION {
        return Err(LoadBundleError::UnsupportedBundleVersion {
            found: header.bundle_version,
            expected: BUNDLE_VERSION,
        });
    }
    let bundle: SavedBundle = ron::de::from_str(&raw).map_err(LoadBundleError::Parse)?;
    // Extra defense: the inner `Project` already carries its own
    // schema_version field that serde checks during decode (every
    // missing-field error surfaces as `Parse`). Validate it against
    // the current model SCHEMA_VERSION too so the user gets a
    // version-shape error rather than a confusing partial parse.
    if bundle.project.schema_version != SCHEMA_VERSION {
        return Err(LoadBundleError::Project(
            ProjectLoadError::UnsupportedSchemaVersion {
                found: bundle.project.schema_version,
                expected: SCHEMA_VERSION,
            },
        ));
    }
    Ok(bundle)
}

/// Install a freshly-loaded bundle as the live project + overlay.
/// Routes the project half through
/// [`AudioResources::apply_project_edit`] (C2's edit pump) so the
/// engine drains its song queue, re-realizes, and re-arms without
/// resetting the playhead. The overlay half is written directly to
/// the AppState signal — overlays don't drive audio, so no engine
/// coordination is needed.
///
/// Lives here (not on `AppState`) so the AppState struct stays free
/// of project_io knowledge; callers (the TopBar file menu) bring the
/// pieces together.
pub fn apply_loaded_bundle(
    app: &AppState,
    audio: &AudioResources,
    bundle: SavedBundle,
) -> Result<(), String> {
    let SavedBundle {
        bundle_version: _,
        project,
        overlay,
    } = bundle;
    // Drive the project replacement through the C2 pump so the
    // engine drains + re-realizes + re-arms in lockstep with the
    // host swap. The closure replaces the inner Project wholesale —
    // semantically the same as a struct-update but uses the existing
    // mutation entry point so the audio side never lies about the
    // project state (the load-path drift trap flagged in
    // [[project-next-session-pickup]]).
    let new_rc = audio.apply_project_edit(move |p| *p = project)?;
    app.project.set(new_rc);
    app.overlay.set(Rc::new(overlay));
    Ok(())
}

#[derive(Debug)]
pub enum LoadBundleError {
    Io(std::io::Error),
    Parse(ron::de::SpannedError),
    UnsupportedBundleVersion { found: u32, expected: u32 },
    /// The inner [`rawdaw_model::project::Project`] surfaced a typed
    /// load error — currently only `UnsupportedSchemaVersion`. Kept
    /// distinct from [`LoadBundleError::UnsupportedBundleVersion`]
    /// (bundle format) so the user can tell whether the file's outer
    /// shape or the inner model is out of date.
    Project(ProjectLoadError),
}

impl core::fmt::Display for LoadBundleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "failed to read project file: {e}"),
            Self::Parse(e) => write!(f, "failed to parse project file: {e}"),
            Self::UnsupportedBundleVersion { found, expected } => write!(
                f,
                "unsupported project bundle version: file is v{found}, \
                 this build understands v{expected}",
            ),
            Self::Project(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LoadBundleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
            Self::UnsupportedBundleVersion { .. } => None,
            Self::Project(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rawdaw_model::fixtures::build_round1_project;

    use crate::overlay::ProjectOverlay;

    use crate::project_io::save::save_to_path;

    #[test]
    fn load_rejects_unsupported_bundle_version() {
        // A future-version bundle file fails at the header peek
        // before any heavy parsing. Pins the version-check contract
        // — without it, a v2 file would attempt a v1 parse and
        // surface a confusing field-level error.
        let dir = tempdir();
        let path = dir.join("future.rawd");
        let future_ron = "(bundle_version: 99, project: (), overlay: ())";
        std::fs::write(&path, future_ron).unwrap();

        match load_from_path(&path) {
            Err(LoadBundleError::UnsupportedBundleVersion {
                found: 99,
                expected: 1,
            }) => {}
            other => panic!("expected UnsupportedBundleVersion(99, 1), got {other:?}"),
        }
    }

    #[test]
    fn load_round_trips_a_valid_bundle() {
        // The happy-path inverse of `save_to_path`: write → load →
        // assert structural equality.
        let dir = tempdir();
        let path = dir.join("ok.rawd");

        let (project, _) = build_round1_project();
        let bundle = SavedBundle::new(project, ProjectOverlay::default());
        save_to_path(&bundle, &path).expect("save succeeds");

        let loaded = load_from_path(&path).expect("load succeeds");
        assert_eq!(loaded, bundle);
    }

    #[test]
    fn load_surfaces_parse_errors_with_readable_message() {
        // RON parse errors carry span info; surface them through
        // Display so the user sees something actionable.
        let dir = tempdir();
        let path = dir.join("broken.rawd");
        std::fs::write(&path, "this is not RON").unwrap();

        let err = load_from_path(&path).unwrap_err();
        assert!(
            matches!(err, LoadBundleError::Parse(_)),
            "expected Parse, got {err:?}"
        );
        // Display should not panic.
        let msg = err.to_string();
        assert!(msg.contains("parse"), "msg: {msg}");
    }

    fn tempdir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "rawdaw-c3-load-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        ));
        std::fs::create_dir_all(&p).expect("create temp dir");
        p
    }
}
