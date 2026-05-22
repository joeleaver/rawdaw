//! Arrangement step CRUD actions called from the arrangement view and
//! the section editor's "open in arrangement" affordances.
//!
//! Free-functions that mutate a [`Project`] — the calling component
//! wraps them in [`crate::state::AppState::apply_project_edit`].
//! Mirrors [`crate::section_actions`] (S1) and
//! [`crate::pattern_actions`] / [`crate::chord_loop_actions`]; this is
//! the **S4** phase of `docs/section-arrangement-editing-plan.md`.
//!
//! File layout (per S0 decision 8 of the plan):
//! - `arrangement_actions/mod.rs` (this file): step CRUD primitives
//!   (`append_step` / `insert_step_after` / `remove_step` /
//!   `move_step` / `duplicate_step` / `set_step_section` /
//!   `set_step_variant`) + their tests.
//! - `arrangement_actions/recompute_starts.rs`: pure
//!   bars-to-musical-time conversion + the canonical
//!   `recompute_starts` walker that every mutator calls after editing.
//! - `arrangement_actions/test_support.rs`: shared `#[cfg(test)]`
//!   fixtures.
//!
//! **Phase state.** S4 ships every primitive without a UI consumer;
//! S5 (the arrangement view interactive editor) is where they get
//! wired. Per-fn / per-use `#[allow(dead_code)]` /
//! `#[allow(unused_imports)]` attributes keep the warning surface
//! honest as the wiring lands.
//!
//! **Deviations from the S4 plan sketch:**
//! - `duplicate_step` returns `Option<SectionRefId>` (not
//!   `SectionRefId`) so UI handlers can no-op on a missing index
//!   without panicking — matches
//!   [`crate::section_actions::duplicate_section`]'s `Option` shape.
//! - `insert_step_after` clamps `index >= len` to "append" instead of
//!   refusing, since the arrangement-view's "insert after this block"
//!   path can race against a concurrent delete; clamping is the safer
//!   user-visible behavior.

#![allow(dead_code)]

mod recompute_starts;

#[cfg(test)]
mod test_support;

#[allow(unused_imports)]
pub use recompute_starts::{recompute_starts, step_duration};

use rawdaw_model::id::{SectionId, SectionRefId, VariantId};
use rawdaw_model::project::Project;
use rawdaw_model::section::SectionRef;
use rawdaw_model::time::MusicalTime;

use self::recompute_starts::recompute_starts as recompute;

/// Append a new arrangement step referencing `section_id @ variant` to
/// the end of `project.arrangement.sections`. Allocates a fresh
/// [`SectionRefId`], pushes the [`SectionRef`] with `start` left at
/// [`MusicalTime::ZERO`], and calls [`recompute_starts`] so the new
/// step's start lands at the correct cumulative position.
///
/// **Empty-sections gate.** The caller (S5's `+ append` button) is
/// expected to refuse the click when `project.sections` is empty; this
/// helper does not validate the `section_id` and a dangling reference
/// will contribute zero duration (see [`recompute_starts`]).
pub fn append_step(
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
    recompute(project);
    id
}

/// Insert a new arrangement step immediately after `index`. When
/// `index >= len`, the step is appended to the end (matching the
/// behavior of `append_step`). Allocates a fresh [`SectionRefId`] and
/// calls [`recompute_starts`].
pub fn insert_step_after(
    project: &mut Project,
    index: usize,
    section_id: SectionId,
    variant: VariantId,
) -> SectionRefId {
    let len = project.arrangement.sections.len();
    let insert_at = (index + 1).min(len);
    let id = project.id_allocators.alloc_section_ref();
    project.arrangement.sections.insert(
        insert_at,
        SectionRef {
            id,
            section: section_id,
            variant,
            start: MusicalTime::ZERO,
        },
    );
    recompute(project);
    id
}

/// Remove the arrangement step at `index` and recompute the remaining
/// steps' starts. Silently no-ops on an out-of-bounds index — UI
/// handlers can't safely panic when the user double-clicks a "delete"
/// affordance against a freshly-deleted step.
pub fn remove_step(project: &mut Project, index: usize) {
    if index >= project.arrangement.sections.len() {
        return;
    }
    project.arrangement.sections.remove(index);
    recompute(project);
}

/// Move the arrangement step at `from` to the position `to`. After the
/// operation, the element previously at `from` is at index `to` (the
/// remove-then-insert convention — `to` is interpreted as the
/// destination index in the post-move state). Silently no-ops when
/// `from == to`, when either index is out of bounds, or when the
/// arrangement is empty.
///
/// Called from S5's drag-to-move handler. Snap-to-bar logic lives in
/// the UI layer; this helper just commits the index change.
pub fn move_step(project: &mut Project, from: usize, to: usize) {
    let len = project.arrangement.sections.len();
    if from == to || from >= len || to >= len {
        return;
    }
    let step = project.arrangement.sections.remove(from);
    project.arrangement.sections.insert(to, step);
    recompute(project);
}

/// Duplicate the arrangement step at `index`, inserting the clone
/// immediately after the source. Returns the new step's
/// [`SectionRefId`], or `None` if `index` is out of bounds.
///
/// The clone reuses the source's `section` and `variant`; `start` is
/// recomputed for the entire arrangement.
pub fn duplicate_step(project: &mut Project, index: usize) -> Option<SectionRefId> {
    let source = project.arrangement.sections.get(index)?;
    let section = source.section;
    let variant = source.variant.clone();
    let id = project.id_allocators.alloc_section_ref();
    project.arrangement.sections.insert(
        index + 1,
        SectionRef {
            id,
            section,
            variant,
            start: MusicalTime::ZERO,
        },
    );
    recompute(project);
    Some(id)
}

/// Replace the step at `index` with a reference to `section_id @
/// variant`. The step's [`SectionRefId`] is preserved (so per-step
/// identity stays stable through a section swap). Silently no-ops on
/// an out-of-bounds index. Recomputes starts so any duration delta
/// propagates to later steps.
pub fn set_step_section(
    project: &mut Project,
    index: usize,
    section_id: SectionId,
    variant: VariantId,
) {
    let Some(step) = project.arrangement.sections.get_mut(index) else {
        return;
    };
    step.section = section_id;
    step.variant = variant;
    recompute(project);
}

/// Replace just the `variant` on the step at `index`. The step's
/// `section` is left unchanged. Used by S5's variant-chip picker on
/// each `SectionBlock`. Silently no-ops on an out-of-bounds index.
pub fn set_step_variant(project: &mut Project, index: usize, variant: VariantId) {
    let Some(step) = project.arrangement.sections.get_mut(index) else {
        return;
    };
    step.variant = variant;
    recompute(project);
}

#[cfg(test)]
mod tests {
    use super::test_support::{
        empty_project, insert_section_with_bars, insert_section_with_variant_override,
    };
    use super::*;

    use rawdaw_model::id::{SectionId, VariantId};
    use rawdaw_model::time::PPQ;

    const DEFAULT_BEATS_PER_BAR: i64 = 4;

    fn bars_to_ticks(bars: u32) -> i64 {
        bars as i64 * DEFAULT_BEATS_PER_BAR * PPQ
    }

    // ---------- append_step ----------

    #[test]
    fn append_step_to_empty_arrangement_starts_at_zero() {
        let mut project = empty_project();
        let sid = insert_section_with_bars(&mut project, 4);
        let _ = append_step(&mut project, sid, VariantId::base());
        assert_eq!(project.arrangement.sections.len(), 1);
        assert_eq!(
            project.arrangement.sections[0].start,
            MusicalTime::ZERO,
        );
    }

    #[test]
    fn append_step_with_existing_starts_after_previous() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        append_step(&mut project, a, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        assert_eq!(
            project.arrangement.sections[1].start.as_ticks(),
            bars_to_ticks(4),
        );
    }

    #[test]
    fn append_step_returns_unique_section_ref_ids() {
        let mut project = empty_project();
        let sid = insert_section_with_bars(&mut project, 4);
        let id_a = append_step(&mut project, sid, VariantId::base());
        let id_b = append_step(&mut project, sid, VariantId::base());
        assert_ne!(id_a, id_b);
    }

    // ---------- insert_step_after ----------

    #[test]
    fn insert_step_after_zero_places_at_index_one() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        let c = insert_section_with_bars(&mut project, 1);
        append_step(&mut project, a, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        // Insert c after the first step → new order: a, c, b.
        let new_id = insert_step_after(&mut project, 0, c, VariantId::base());
        let order: Vec<_> = project
            .arrangement
            .sections
            .iter()
            .map(|s| s.section)
            .collect();
        assert_eq!(order, vec![a, c, b]);
        assert_eq!(project.arrangement.sections[1].id, new_id);
    }

    #[test]
    fn insert_step_after_at_end_appends() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        append_step(&mut project, a, VariantId::base());
        // index == len-1 → insert at len (i.e., append).
        insert_step_after(&mut project, 0, b, VariantId::base());
        let order: Vec<_> = project
            .arrangement
            .sections
            .iter()
            .map(|s| s.section)
            .collect();
        assert_eq!(order, vec![a, b]);
    }

    #[test]
    fn insert_step_after_out_of_bounds_appends() {
        // Deviation note: insert_step_after clamps oob to "append" so
        // the UI's "insert after this block" path is forgiving against
        // races with a concurrent delete.
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        append_step(&mut project, a, VariantId::base());
        insert_step_after(&mut project, 99, b, VariantId::base());
        let order: Vec<_> = project
            .arrangement
            .sections
            .iter()
            .map(|s| s.section)
            .collect();
        assert_eq!(order, vec![a, b]);
    }

    #[test]
    fn insert_step_after_recomputes_later_starts() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        let c = insert_section_with_bars(&mut project, 1);
        append_step(&mut project, a, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        // Insert c between a and b. Expected starts: 0, c-end=1, b-end=...
        // a@0, c@bars_to_ticks(4)=4 bars, b@bars_to_ticks(4+1)=5 bars.
        insert_step_after(&mut project, 0, c, VariantId::base());
        assert_eq!(
            project.arrangement.sections[2].start.as_ticks(),
            bars_to_ticks(5),
        );
    }

    // ---------- remove_step ----------

    #[test]
    fn remove_first_step_shifts_subsequent_starts() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        append_step(&mut project, a, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        // Pre-remove: b starts at 4 bars. Post-remove: b is the only
        // step and starts at 0.
        remove_step(&mut project, 0);
        assert_eq!(project.arrangement.sections.len(), 1);
        assert_eq!(
            project.arrangement.sections[0].start,
            MusicalTime::ZERO,
        );
    }

    #[test]
    fn remove_last_step_leaves_earlier_starts_unchanged() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        append_step(&mut project, a, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        remove_step(&mut project, 1);
        assert_eq!(project.arrangement.sections.len(), 1);
        assert_eq!(
            project.arrangement.sections[0].start,
            MusicalTime::ZERO,
        );
    }

    #[test]
    fn remove_middle_step_shifts_only_later_starts() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        let c = insert_section_with_bars(&mut project, 1);
        append_step(&mut project, a, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        append_step(&mut project, c, VariantId::base());
        // Remove b. After: a@0, c@4 bars (was 6 bars).
        remove_step(&mut project, 1);
        assert_eq!(project.arrangement.sections.len(), 2);
        assert_eq!(
            project.arrangement.sections[1].start.as_ticks(),
            bars_to_ticks(4),
        );
    }

    #[test]
    fn remove_out_of_bounds_is_noop() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        append_step(&mut project, a, VariantId::base());
        remove_step(&mut project, 99);
        assert_eq!(project.arrangement.sections.len(), 1);
    }

    #[test]
    fn remove_from_empty_arrangement_is_noop() {
        let mut project = empty_project();
        remove_step(&mut project, 0);
        assert!(project.arrangement.sections.is_empty());
    }

    // ---------- move_step ----------

    #[test]
    fn move_step_forward_shifts_intermediate_starts() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        let c = insert_section_with_bars(&mut project, 1);
        append_step(&mut project, a, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        append_step(&mut project, c, VariantId::base());
        // Move a (idx 0) to the end (idx 2). New order: b, c, a.
        move_step(&mut project, 0, 2);
        let order: Vec<_> = project
            .arrangement
            .sections
            .iter()
            .map(|s| s.section)
            .collect();
        assert_eq!(order, vec![b, c, a]);
        // a's start is now 2 + 1 = 3 bars.
        assert_eq!(
            project.arrangement.sections[2].start.as_ticks(),
            bars_to_ticks(3),
        );
    }

    #[test]
    fn move_step_backward_shifts_intermediate_starts() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        let c = insert_section_with_bars(&mut project, 1);
        append_step(&mut project, a, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        append_step(&mut project, c, VariantId::base());
        // Move c (idx 2) to position 0. New order: c, a, b.
        move_step(&mut project, 2, 0);
        let order: Vec<_> = project
            .arrangement
            .sections
            .iter()
            .map(|s| s.section)
            .collect();
        assert_eq!(order, vec![c, a, b]);
        assert_eq!(
            project.arrangement.sections[1].start.as_ticks(),
            bars_to_ticks(1),
        );
    }

    #[test]
    fn move_step_to_same_index_is_noop() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        append_step(&mut project, a, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        let before = project.arrangement.sections.clone();
        move_step(&mut project, 1, 1);
        assert_eq!(project.arrangement.sections, before);
    }

    #[test]
    fn move_step_with_oob_from_is_noop() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        append_step(&mut project, a, VariantId::base());
        let before = project.arrangement.sections.clone();
        move_step(&mut project, 99, 0);
        assert_eq!(project.arrangement.sections, before);
    }

    #[test]
    fn move_step_with_oob_to_is_noop() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        append_step(&mut project, a, VariantId::base());
        let before = project.arrangement.sections.clone();
        move_step(&mut project, 0, 99);
        assert_eq!(project.arrangement.sections, before);
    }

    // ---------- duplicate_step ----------

    #[test]
    fn duplicate_step_clones_section_and_variant() {
        let mut project = empty_project();
        let (a, fill) = insert_section_with_variant_override(&mut project, 4, 2);
        append_step(&mut project, a, fill.clone());
        let new_id = duplicate_step(&mut project, 0).unwrap();
        let clone = &project.arrangement.sections[1];
        assert_eq!(clone.section, a);
        assert_eq!(clone.variant, fill);
        assert_eq!(clone.id, new_id);
    }

    #[test]
    fn duplicate_step_inserts_immediately_after_source() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        append_step(&mut project, a, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        duplicate_step(&mut project, 0);
        let order: Vec<_> = project
            .arrangement
            .sections
            .iter()
            .map(|s| s.section)
            .collect();
        assert_eq!(order, vec![a, a, b]);
    }

    #[test]
    fn duplicate_step_returns_fresh_section_ref_id() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let original_id = append_step(&mut project, a, VariantId::base());
        let cloned_id = duplicate_step(&mut project, 0).unwrap();
        assert_ne!(original_id, cloned_id);
    }

    #[test]
    fn duplicate_out_of_bounds_returns_none() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        append_step(&mut project, a, VariantId::base());
        assert_eq!(duplicate_step(&mut project, 99), None);
    }

    // ---------- set_step_section / set_step_variant ----------

    #[test]
    fn set_step_section_recomputes_starts_when_duration_differs() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        let big = insert_section_with_bars(&mut project, 16);
        append_step(&mut project, a, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        // Pre: b starts at 4 bars. Swap a → big (16 bars); b now starts at 16.
        set_step_section(&mut project, 0, big, VariantId::base());
        assert_eq!(
            project.arrangement.sections[1].start.as_ticks(),
            bars_to_ticks(16),
        );
    }

    #[test]
    fn set_step_variant_no_shift_when_duration_same() {
        // Variant has duration_bars: None → falls back to base. Step
        // start of the next step should NOT change.
        let mut project = empty_project();
        let sid = insert_section_with_bars(&mut project, 4);
        let no_dur = VariantId::new("no-dur");
        project
            .sections
            .get_mut(&sid)
            .unwrap()
            .variants
            .insert(no_dur.clone(), Default::default());
        let b = insert_section_with_bars(&mut project, 2);
        append_step(&mut project, sid, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        let before = project.arrangement.sections[1].start;
        set_step_variant(&mut project, 0, no_dur);
        assert_eq!(project.arrangement.sections[1].start, before);
    }

    #[test]
    fn set_step_variant_shifts_starts_when_override_duration_differs() {
        let mut project = empty_project();
        let (sid, fill) = insert_section_with_variant_override(&mut project, 8, 2);
        let b = insert_section_with_bars(&mut project, 4);
        append_step(&mut project, sid, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        // Pre: b @ 8 bars (using base of 8). Swap variant → fill (2 bars).
        set_step_variant(&mut project, 0, fill);
        assert_eq!(
            project.arrangement.sections[1].start.as_ticks(),
            bars_to_ticks(2),
        );
    }

    #[test]
    fn set_step_section_out_of_bounds_is_noop() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        append_step(&mut project, a, VariantId::base());
        let before = project.arrangement.sections.clone();
        set_step_section(&mut project, 99, b, VariantId::base());
        assert_eq!(project.arrangement.sections, before);
    }

    #[test]
    fn set_step_variant_out_of_bounds_is_noop() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        append_step(&mut project, a, VariantId::base());
        let before = project.arrangement.sections.clone();
        set_step_variant(&mut project, 99, VariantId::new("ignored"));
        assert_eq!(project.arrangement.sections, before);
    }

    #[test]
    fn set_step_section_preserves_section_ref_id() {
        // Swapping the section under a step keeps the SectionRefId
        // stable — important for any per-step state the UI keeps
        // keyed off the ref id.
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        let id_before = append_step(&mut project, a, VariantId::base());
        set_step_section(&mut project, 0, b, VariantId::base());
        let id_after = project.arrangement.sections[0].id;
        assert_eq!(id_before, id_after);
    }

    // ---------- cross-helper sanity ----------

    #[test]
    fn first_step_always_at_zero_after_any_mutation() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 2);
        let c = insert_section_with_bars(&mut project, 1);
        append_step(&mut project, a, VariantId::base());
        append_step(&mut project, b, VariantId::base());
        insert_step_after(&mut project, 0, c, VariantId::base());
        move_step(&mut project, 2, 0);
        duplicate_step(&mut project, 0);
        remove_step(&mut project, 1);
        assert!(!project.arrangement.sections.is_empty());
        assert_eq!(
            project.arrangement.sections[0].start,
            MusicalTime::ZERO,
            "first step must always start at MusicalTime::ZERO",
        );
    }

    #[test]
    fn missing_section_does_not_panic_on_append() {
        // Defensive: even if the UI ever passes a dangling section_id,
        // the append helper must not panic. The step is appended with
        // a zero-duration contribution.
        let mut project = empty_project();
        append_step(&mut project, SectionId::new(9999), VariantId::base());
        assert_eq!(project.arrangement.sections.len(), 1);
        assert_eq!(
            project.arrangement.sections[0].start,
            MusicalTime::ZERO,
        );
    }
}
