//! Step-CRUD tests for `arrangement_actions`. Carved out of `mod.rs`
//! at S5 close-out so `mod.rs` stays under the 700-line cap. Pure
//! tests — no production code lives here.

#![cfg(test)]

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

// ---------- insert_step_at ----------

#[test]
fn insert_step_at_zero_prepends() {
    let mut project = empty_project();
    let a = insert_section_with_bars(&mut project, 4);
    let b = insert_section_with_bars(&mut project, 2);
    append_step(&mut project, a, VariantId::base());
    insert_step_at(&mut project, 0, b, VariantId::base());
    let order: Vec<_> = project
        .arrangement
        .sections
        .iter()
        .map(|s| s.section)
        .collect();
    assert_eq!(order, vec![b, a]);
    // New first step lands at zero; original a shifts to b's duration.
    assert_eq!(
        project.arrangement.sections[0].start,
        MusicalTime::ZERO,
    );
    assert_eq!(
        project.arrangement.sections[1].start.as_ticks(),
        bars_to_ticks(2),
    );
}

#[test]
fn insert_step_at_len_appends() {
    let mut project = empty_project();
    let a = insert_section_with_bars(&mut project, 4);
    let b = insert_section_with_bars(&mut project, 2);
    append_step(&mut project, a, VariantId::base());
    let len = project.arrangement.sections.len();
    insert_step_at(&mut project, len, b, VariantId::base());
    let order: Vec<_> = project
        .arrangement
        .sections
        .iter()
        .map(|s| s.section)
        .collect();
    assert_eq!(order, vec![a, b]);
}

#[test]
fn insert_step_at_out_of_bounds_appends() {
    let mut project = empty_project();
    let a = insert_section_with_bars(&mut project, 4);
    let b = insert_section_with_bars(&mut project, 2);
    append_step(&mut project, a, VariantId::base());
    insert_step_at(&mut project, 99, b, VariantId::base());
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
