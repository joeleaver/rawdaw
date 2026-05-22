//! Shared `#[cfg(test)]` fixtures for the `section_actions` modules.
//!
//! Mirrors `pattern_actions::test_support` — small helpers that build
//! a `Project` in the exact shape each test needs without forcing
//! callers to repeat the boilerplate.

use rawdaw_model::id::{SectionId, VariantId};
use rawdaw_model::pitch::PitchClass;
use rawdaw_model::project::Project;
use rawdaw_model::scale::Scale;
use rawdaw_model::section::SectionRef;
use rawdaw_model::time::MusicalTime;

/// Build a fresh `Project` keyed in C major with no sections, tracks,
/// patterns, or arrangement steps.
pub(super) fn empty_project() -> Project {
    Project::new(Scale::major(PitchClass::C))
}

/// Append an arrangement step that references `section_id` with the
/// default variant. The `start` field is left at `MusicalTime::ZERO`
/// since none of the S1 tests exercise start recomputation — that's
/// S4's responsibility (`arrangement_actions::recompute_starts`).
pub(super) fn project_with_arrangement_step(project: &mut Project, section_id: SectionId) {
    let id = project.id_allocators.alloc_section_ref();
    project.arrangement.sections.push(SectionRef {
        id,
        section: section_id,
        variant: VariantId::base(),
        start: MusicalTime::ticks(0),
    });
}
