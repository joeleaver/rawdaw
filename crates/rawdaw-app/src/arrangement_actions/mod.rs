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

#[cfg(test)]
mod tests;

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

/// Insert a new arrangement step *at* `index` (i.e., before the step
/// currently at `index`). When `index >= len`, the step is appended to
/// the end. Used by S5's "Insert section before…" context-menu item,
/// which can't use [`insert_step_after`] because `index == 0` would
/// underflow to `usize::MAX`.
pub fn insert_step_at(
    project: &mut Project,
    index: usize,
    section_id: SectionId,
    variant: VariantId,
) -> SectionRefId {
    let len = project.arrangement.sections.len();
    let insert_at = index.min(len);
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

