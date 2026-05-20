//! Write a [`SavedBundle`] to disk as pretty-printed RON.

use std::fs;
use std::path::Path;

use super::bundle::SavedBundle;

/// Write `bundle` to `path` as pretty-printed RON. Overwrites any
/// existing file at the path.
///
/// On success the file contains a single RON value with the bundle
/// fields in declaration order
/// (`bundle_version`, `project`, `overlay`); see [`SavedBundle`].
///
/// Errors:
/// - [`SaveBundleError::Ron`] — serde failed to encode (rare;
///   indicates a malformed `Project` / `ProjectOverlay`).
/// - [`SaveBundleError::Io`] — filesystem write failed (permission
///   denied, disk full, parent directory missing). The file is left
///   in whatever partial state the OS chose; callers should not
///   assume atomicity.
pub fn save_to_path(bundle: &SavedBundle, path: &Path) -> Result<(), SaveBundleError> {
    let serialized = ron::ser::to_string_pretty(bundle, ron::ser::PrettyConfig::default())
        .map_err(SaveBundleError::Ron)?;
    fs::write(path, serialized).map_err(SaveBundleError::Io)
}

#[derive(Debug)]
pub enum SaveBundleError {
    Ron(ron::Error),
    Io(std::io::Error),
}

impl core::fmt::Display for SaveBundleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ron(e) => write!(f, "failed to serialize project bundle: {e}"),
            Self::Io(e) => write!(f, "failed to write project file: {e}"),
        }
    }
}

impl std::error::Error for SaveBundleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Ron(e) => Some(e),
            Self::Io(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rawdaw_model::fixtures::build_round1_project;

    use crate::overlay::ProjectOverlay;

    #[test]
    fn save_then_load_round_trips_via_temp_file() {
        // End-to-end disk round-trip — pinned here in addition to
        // the in-memory round-trip in `bundle.rs` to surface any
        // path / encoding wrinkle that only appears at the
        // filesystem boundary.
        let dir = tempdir();
        let path = dir.join("round-trip.rawd");

        let (project, _) = build_round1_project();
        let bundle = SavedBundle::new(project, ProjectOverlay::default());
        save_to_path(&bundle, &path).expect("save succeeds");

        let raw = std::fs::read_to_string(&path).expect("read succeeds");
        let parsed: SavedBundle = ron::de::from_str(&raw).expect("parse succeeds");
        assert_eq!(parsed, bundle);
    }

    /// Test-only temp directory. We use the system tmp dir with a
    /// per-process unique name so parallel tests don't collide. The
    /// directory is left behind on exit — the OS reaps `/tmp` on
    /// reboot, and we don't want a test panic to interfere with a
    /// dropguard cleanup.
    fn tempdir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "rawdaw-c3-test-{}-{}",
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
