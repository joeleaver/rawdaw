//! Initial-project factory — the one-shot entry point that produces the
//! `(Project, ProjectOverlay)` pair seeded into [`crate::state::AppState`]
//! at app boot.
//!
//! C1b of the composition-writability milestone introduces this module as
//! the replacement for `fixture::round1()`'s static singleton. For now
//! `fixture/` still exists and rebuilds the Round1 view independently on
//! first call; subsequent C1 commits migrate every read site to the
//! `AppState` signals and then delete `fixture/` entirely.
//!
//! The round-1 project content itself comes from
//! `rawdaw_model::fixtures::build_round1_project()` (single source of
//! truth, shared with the model crate's tests). This module layers the
//! UI-only overlay on top.

use rawdaw_model::fixtures::build_round1_project;
use rawdaw_model::project::Project;

use crate::overlay::ProjectOverlay;

pub(crate) mod overlay;

/// Build the initial project + UI overlay that the app boots into.
///
/// Today this returns the round-1 demo project. Tier-1 work (chord-loop
/// editor, section editor, etc.) will replace the body with either a
/// "last opened project" file lookup or a "minimum sensible empty
/// project" depending on user choice. This function's signature is the
/// stable contract for that future swap.
pub fn build_initial() -> (Project, ProjectOverlay) {
    let (project, keys) = build_round1_project();
    let overlay = overlay::build_round1_overlay(&keys);
    (project, overlay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_initial_returns_round1_with_populated_overlay() {
        let (project, overlay) = build_initial();
        // Round-1 demo project must boot with the expected four tracks
        // and three sections — these counts are pinned by
        // `rawdaw_model::fixtures` tests too, repeated here as the
        // app-side contract.
        assert_eq!(project.tracks.len(), 4);
        assert_eq!(project.sections.len(), 3);

        // Overlay populates one color entry per model item.
        assert_eq!(overlay.pattern_color.len(), project.patterns.len());
        assert_eq!(overlay.section_color.len(), project.sections.len());
        assert_eq!(overlay.chord_loop_color.len(), project.chord_loops.len());
    }
}
