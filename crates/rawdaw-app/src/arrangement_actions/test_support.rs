//! Shared `#[cfg(test)]` fixtures for the `arrangement_actions` modules.
//!
//! Mirrors [`crate::section_actions::test_support`] — small helpers
//! that build a [`Project`] in the exact shape each test needs so
//! callers don't repeat the same boilerplate.

use rawdaw_model::id::{SectionId, SectionRefId, VariantId};
use rawdaw_model::pitch::PitchClass;
use rawdaw_model::project::Project;
use rawdaw_model::scale::Scale;
use rawdaw_model::section::{Section, SectionBody, SectionRef, SectionVariantOverride};
use rawdaw_model::time::MusicalTime;
use std::collections::BTreeMap;

/// Build a fresh `Project` keyed in C major with no sections, tracks,
/// patterns, or arrangement steps.
pub(super) fn empty_project() -> Project {
    Project::new(Scale::major(PitchClass::C))
}

/// Insert a section with the given `duration_bars` and no variants.
/// Returns the freshly-allocated [`SectionId`] so the caller can wire
/// it into an arrangement step.
pub(super) fn insert_section_with_bars(project: &mut Project, bars: u32) -> SectionId {
    let id = project.id_allocators.alloc_section();
    let section = Section {
        id,
        name: format!("s{}", id.get()),
        base: SectionBody {
            duration_bars: bars,
            scale_override: None,
            chord_loops: Vec::new(),
            activations: BTreeMap::new(),
        },
        variants: BTreeMap::new(),
        default_variant: VariantId::base(),
    };
    project.sections.insert(id, section);
    id
}

/// Insert a section with `bars` for the base and a single non-default
/// variant whose `duration_bars` overrides to `variant_bars`. Returns
/// `(section_id, variant_id)`. The variant id is `"fill"`.
pub(super) fn insert_section_with_variant_override(
    project: &mut Project,
    bars: u32,
    variant_bars: u32,
) -> (SectionId, VariantId) {
    let id = insert_section_with_bars(project, bars);
    let variant = VariantId::new("fill");
    project
        .sections
        .get_mut(&id)
        .unwrap()
        .variants
        .insert(
            variant.clone(),
            SectionVariantOverride {
                duration_bars: Some(variant_bars),
                scale_override: None,
                chord_loops: None,
                activations: BTreeMap::new(),
            },
        );
    (id, variant)
}

/// Append a bare arrangement step that references `section_id` with
/// the given `variant`. `start` is left at [`MusicalTime::ZERO`] so
/// tests can verify a subsequent `recompute_starts` call writes the
/// correct value instead of trusting an already-correct seed.
///
/// Use this in tests of `recompute_starts` itself or when seeding a
/// known-bad pre-state. Tests of the CRUD wrappers should use those
/// wrappers (which call `recompute_starts` themselves).
pub(super) fn append_bare_step(
    project: &mut Project,
    section_id: SectionId,
    variant: VariantId,
) -> SectionRefId {
    let id = project.id_allocators.alloc_section_ref();
    project.arrangement.sections.push(SectionRef {
        id,
        section: section_id,
        variant,
        start: MusicalTime::ZERO,
    });
    id
}
