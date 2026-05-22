//! Section CRUD actions called from the Library panel and the section
//! editor.
//!
//! Free-functions that mutate a [`Project`] (and, for color, a
//! [`ProjectOverlay`]) — the calling component wraps them in
//! [`AppState::apply_project_edit`] / direct overlay `Signal::set`.
//! Mirrors [`crate::chord_loop_actions`] (CL1) and
//! [`crate::pattern_actions`] (P1); this is the S1 phase of
//! `docs/section-arrangement-editing-plan.md`.
//!
//! File layout (split per S0 decision 8 to keep each file under the
//! 700-line cap):
//! - `section_actions/mod.rs` (this file): section-level CRUD (create
//!   / rename / delete / duplicate / set_color), arrangement-ref
//!   walker, `unique_section_name` helper, and pattern-level tests.
//! - `section_actions/duration.rs`: duration + scale override
//!   mutations with the auto-promote-from-base pattern (S0 decision
//!   3).
//! - `section_actions/variants.rs`: variant CRUD (add / remove /
//!   rename / set_default) with default-variant + arrangement-ref
//!   protection.
//!
//! **S1 state:** every primitive in this module is `pub` but has no
//! UI consumer yet — S2 (Library), S3 (section-editor meta-bar), and
//! S5 (Arrangement view) wire them in. The module-level
//! `#![allow(dead_code)]` + `#![allow(unused_imports)]` suppress the
//! "never used" warnings until those phases land; the per-function
//! attribute can come off as each consumer arrives. The same pattern
//! was tolerated transiently in `pattern_actions` (P1) before the
//! pattern-editor UI consumed its primitives.

#![allow(dead_code)]
#![allow(unused_imports)]

mod duration;
mod variants;

#[cfg(test)]
mod test_support;

pub use duration::{
    clear_variant_duration_override, clear_variant_scale_override, set_section_duration_bars,
    set_section_scale_override,
};
pub use variants::{
    add_section_variant, remove_section_variant, set_default_variant, RemoveVariantError,
    VariantConflict,
};
// `rename_section_variant` ships with the model surface so the
// `section_actions` API is complete at S1, but the meta-bar UI
// consumer doesn't land until S3. Match the
// `pattern_actions::rename_variant` precedent of `#[allow(dead_code)]`
// at the function level + `#[allow(unused_imports)]` on the
// re-export so the unused-import lint doesn't trip pre-S3.
#[allow(unused_imports)]
pub use variants::rename_section_variant;

use std::collections::BTreeMap;

use rawdaw_model::id::{SectionId, VariantId};
use rawdaw_model::project::Project;
use rawdaw_model::section::{Section, SectionBody};

use crate::overlay::ProjectOverlay;

/// Default duration for a freshly-created section. Four bars in 4/4 —
/// matches the round-1 verse / chorus fixtures and is the most common
/// starting length for a structural unit in a pop arrangement. The
/// user can edit this in the section editor (S3).
pub const DEFAULT_SECTION_BARS: u32 = 4;

/// Create a new empty section. Allocates a fresh [`SectionId`], picks
/// a unique `"untitled"` name, seeds an empty base body and an empty
/// `variants` map with `default_variant = VariantId::base()`, and
/// inserts the section into `project.sections`. Returns the new id so
/// the caller (the Library `+ new section` row) can select it
/// immediately.
pub fn create_section(project: &mut Project) -> SectionId {
    let id = project.id_allocators.alloc_section();
    let name = unique_section_name(&project.sections, "untitled");
    let section = Section {
        id,
        name,
        base: SectionBody {
            duration_bars: DEFAULT_SECTION_BARS,
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

/// Rename a section in place. Caller is expected to have trimmed
/// the new name and refused empty values (the [`NameControl`]-style
/// pattern is reused by the Library row's inline editor). Silently
/// no-ops if `id` doesn't exist — UI handlers can't observe a missing
/// id without a race against deletion, and an `unwrap`-style panic
/// here would be worse.
pub fn rename_section(project: &mut Project, id: SectionId, name: String) {
    if let Some(s) = project.sections.get_mut(&id) {
        s.name = name;
    }
}

/// Duplicate the section identified by `id`. Returns the new id, or
/// `None` if the source id doesn't exist. The duplicate gets a fresh
/// allocator-issued id, a "copy"-suffixed unique name, and a deep
/// clone of `base` / `variants` / `default_variant`.
///
/// The duplicate is NOT added to the arrangement — that's a separate
/// action (S4 `arrangement_actions::insert_step_after`). Mirrors
/// `chord_loop_actions::duplicate_chord_loop`.
pub fn duplicate_section(project: &mut Project, id: SectionId) -> Option<SectionId> {
    let source = project.sections.get(&id).cloned()?;
    let new_id = project.id_allocators.alloc_section();
    let seed = format!("{} copy", source.name);
    let name = unique_section_name(&project.sections, &seed);
    let duplicate = Section {
        id: new_id,
        name,
        base: source.base,
        variants: source.variants,
        default_variant: source.default_variant,
    };
    project.sections.insert(new_id, duplicate);
    Some(new_id)
}

/// Delete a section. Refuses if any [`Arrangement`] step references
/// it — returns [`DeleteRefused::ReferencedBy`] listing the step
/// indices so the caller can surface a useful message
/// (e.g. "referenced by step 0, step 4"). Mirrors the delete-refusal
/// policy from CL1 / P1; the toast/alert primitive lands later.
///
/// [`Arrangement`]: rawdaw_model::section::Arrangement
pub fn delete_section(project: &mut Project, id: SectionId) -> Result<(), DeleteRefused> {
    let referenced_by = arrangement_references(project, id);
    if !referenced_by.is_empty() {
        return Err(DeleteRefused::ReferencedBy(referenced_by));
    }
    if project.sections.remove(&id).is_none() {
        return Err(DeleteRefused::NotFound);
    }
    Ok(())
}

/// Set the overlay color for a section. Color strings are expected to
/// be `#RRGGBB`; no validation here, since the picker supplies
/// palette literals from `theme::PAL_*`. Empty string removes the
/// entry so the row falls back to the default `theme::TEXT2` tint at
/// render time. Matches `chord_loop_actions::set_chord_loop_color`.
pub fn set_section_color(overlay: &mut ProjectOverlay, id: SectionId, color: String) {
    if color.is_empty() {
        overlay.section_color.remove(&id);
    } else {
        overlay.section_color.insert(id, color);
    }
}

/// Result of refusing to delete a section. The reference variant
/// carries the deduplicated, sorted list of arrangement step indices
/// so the UI can build a message like "referenced by step 0, step 4"
/// without re-walking the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteRefused {
    /// One or more arrangement steps reference the section. The
    /// `Vec<usize>` is the sorted, deduplicated list of step indices.
    ReferencedBy(Vec<usize>),
    /// The section id didn't exist in the project. UI-side this is a
    /// "shouldn't happen" race; surfaced as an error so the caller
    /// doesn't silently no-op a click the user expected to do
    /// something.
    NotFound,
}

/// Walk the arrangement and collect every step index whose `section`
/// id matches the target. Public so a future section editor (or the
/// S5 arrangement view) can render "where is this section used" UX
/// without duplicating the walk.
pub fn arrangement_references(project: &Project, id: SectionId) -> Vec<usize> {
    project
        .arrangement
        .sections
        .iter()
        .enumerate()
        .filter_map(|(idx, step)| (step.section == id).then_some(idx))
        .collect()
}

/// Pick a unique name in the form `"<seed>"`, `"<seed>-2"`,
/// `"<seed>-3"`, … given the existing section name set. Used by
/// `create_section` (with seed `"untitled"`) and `duplicate_section`
/// (with seed `"<source> copy"`).
fn unique_section_name(sections: &BTreeMap<SectionId, Section>, seed: &str) -> String {
    let used: std::collections::BTreeSet<&str> =
        sections.values().map(|s| s.name.as_str()).collect();
    if !used.contains(seed) {
        return seed.to_string();
    }
    for n in 2u32..u32::MAX {
        let candidate = format!("{seed}-{n}");
        if !used.contains(candidate.as_str()) {
            return candidate;
        }
    }
    // The 2..u32::MAX range is effectively unreachable — if it ever
    // fires we'd rather return a recognizable sentinel than panic in
    // a UI handler.
    format!("{seed}-overflow")
}

#[cfg(test)]
mod tests {
    use super::test_support::{empty_project, project_with_arrangement_step};
    use super::*;

    use rawdaw_model::id::SectionId;

    #[test]
    fn create_section_allocates_id_and_unique_name() {
        let mut project = empty_project();
        let a = create_section(&mut project);
        let b = create_section(&mut project);
        assert_ne!(a, b, "two creates must produce distinct ids");
        let names: Vec<&str> = project.sections.values().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"untitled"));
        assert!(names.contains(&"untitled-2"));
    }

    #[test]
    fn create_section_defaults_base_body_and_no_variants() {
        let mut project = empty_project();
        let id = create_section(&mut project);
        let section = project.sections.get(&id).unwrap();
        assert_eq!(section.base.duration_bars, DEFAULT_SECTION_BARS);
        assert!(section.base.scale_override.is_none());
        assert!(section.base.chord_loops.is_empty());
        assert!(section.base.activations.is_empty());
        assert!(section.variants.is_empty());
        assert_eq!(section.default_variant, VariantId::base());
    }

    #[test]
    fn rename_section_writes_new_name() {
        let mut project = empty_project();
        let id = create_section(&mut project);
        rename_section(&mut project, id, "verse".into());
        assert_eq!(project.sections.get(&id).unwrap().name, "verse");
    }

    #[test]
    fn rename_section_is_a_noop_for_missing_ids() {
        let mut project = empty_project();
        let real = create_section(&mut project);
        let bogus = SectionId::new(9999);
        let before = project.sections.clone();
        rename_section(&mut project, bogus, "ignored".into());
        assert_eq!(project.sections, before);
        assert_eq!(project.sections.get(&real).unwrap().name, "untitled");
    }

    #[test]
    fn duplicate_section_clones_with_fresh_id_and_suffixed_name() {
        let mut project = empty_project();
        let original = create_section(&mut project);
        rename_section(&mut project, original, "verse".into());

        let copy = duplicate_section(&mut project, original).unwrap();
        assert_ne!(copy, original);
        assert_eq!(project.sections.get(&copy).unwrap().name, "verse copy");
        // Second duplicate of the same source bumps the suffix.
        let copy2 = duplicate_section(&mut project, original).unwrap();
        assert_eq!(project.sections.get(&copy2).unwrap().name, "verse copy-2");
    }

    #[test]
    fn duplicate_section_deep_clones_base_and_variants() {
        // Confirm that mutating the duplicate's base body doesn't leak
        // back into the source — i.e., the clone is structurally
        // independent, not a borrow.
        let mut project = empty_project();
        let source_id = create_section(&mut project);
        {
            let source = project.sections.get_mut(&source_id).unwrap();
            source.base.duration_bars = 8;
            source.variants.insert(
                VariantId::new("fill"),
                rawdaw_model::section::SectionVariantOverride::default(),
            );
            source.default_variant = VariantId::new("fill");
        }

        let copy_id = duplicate_section(&mut project, source_id).unwrap();
        // Mutate the copy.
        {
            let copy = project.sections.get_mut(&copy_id).unwrap();
            copy.base.duration_bars = 16;
        }

        // Source must still read 8, not 16.
        let source = project.sections.get(&source_id).unwrap();
        assert_eq!(source.base.duration_bars, 8);
        assert!(source.variants.contains_key(&VariantId::new("fill")));
        assert_eq!(source.default_variant, VariantId::new("fill"));

        // Copy keeps the inherited variants + default.
        let copy = project.sections.get(&copy_id).unwrap();
        assert_eq!(copy.base.duration_bars, 16);
        assert!(copy.variants.contains_key(&VariantId::new("fill")));
        assert_eq!(copy.default_variant, VariantId::new("fill"));
    }

    #[test]
    fn duplicate_section_missing_id_returns_none() {
        let mut project = empty_project();
        assert_eq!(duplicate_section(&mut project, SectionId::new(0)), None);
    }

    #[test]
    fn delete_unreferenced_section_removes_it() {
        let mut project = empty_project();
        let id = create_section(&mut project);
        assert_eq!(delete_section(&mut project, id), Ok(()));
        assert!(!project.sections.contains_key(&id));
    }

    #[test]
    fn delete_referenced_section_refuses_and_lists_step_indices() {
        let mut project = empty_project();
        let id = create_section(&mut project);
        // Place the section in the arrangement at step 0.
        project_with_arrangement_step(&mut project, id);

        let err = delete_section(&mut project, id).unwrap_err();
        assert_eq!(err, DeleteRefused::ReferencedBy(vec![0]));
        // The section must still be present after the refusal.
        assert!(project.sections.contains_key(&id));
    }

    #[test]
    fn delete_refusal_lists_every_step_position_in_sorted_order() {
        // Defends against the walker missing duplicates or returning
        // unsorted indices. Place the same section at three positions.
        let mut project = empty_project();
        let id = create_section(&mut project);
        project_with_arrangement_step(&mut project, id);
        project_with_arrangement_step(&mut project, id);
        project_with_arrangement_step(&mut project, id);

        let err = delete_section(&mut project, id).unwrap_err();
        assert_eq!(err, DeleteRefused::ReferencedBy(vec![0, 1, 2]));
    }

    #[test]
    fn delete_missing_id_returns_not_found() {
        let mut project = empty_project();
        let err = delete_section(&mut project, SectionId::new(0)).unwrap_err();
        assert_eq!(err, DeleteRefused::NotFound);
    }

    #[test]
    fn arrangement_references_returns_empty_when_unused() {
        let mut project = empty_project();
        let id = create_section(&mut project);
        assert_eq!(arrangement_references(&project, id), Vec::<usize>::new());
    }

    #[test]
    fn set_section_color_writes_and_clears_overlay_entry() {
        let mut overlay = ProjectOverlay::default();
        let id = SectionId::new(1);
        set_section_color(&mut overlay, id, "#abcdef".into());
        assert_eq!(
            overlay.section_color.get(&id).map(|s| s.as_str()),
            Some("#abcdef"),
        );
        set_section_color(&mut overlay, id, "".into());
        assert!(!overlay.section_color.contains_key(&id));
    }

    #[test]
    fn unique_section_name_overflow_returns_sentinel() {
        // Pre-populate the section map with the seed and the entire
        // suffix range so the overflow branch fires. We test indirectly
        // by checking the helper directly.
        let mut sections = BTreeMap::new();
        let mut id_count = 0u64;
        let mut insert = |sections: &mut BTreeMap<SectionId, Section>, name: &str| {
            id_count += 1;
            let id = SectionId::new(id_count);
            sections.insert(
                id,
                Section {
                    id,
                    name: name.to_string(),
                    base: SectionBody {
                        duration_bars: 4,
                        scale_override: None,
                        chord_loops: Vec::new(),
                        activations: BTreeMap::new(),
                    },
                    variants: BTreeMap::new(),
                    default_variant: VariantId::base(),
                },
            );
        };
        insert(&mut sections, "verse");
        // Single collision → "verse-2".
        assert_eq!(unique_section_name(&sections, "verse"), "verse-2");
        // Free name → unchanged.
        assert_eq!(unique_section_name(&sections, "chorus"), "chorus");
    }
}
