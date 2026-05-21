//! Activation mutations on a section.
//!
//! Wraps the per-`ActivationEntry` mutations + the pure variant-
//! schedule helpers in `section_editor/cell/variant_schedule.rs`. Each
//! function takes a `Project` mutably; callers wrap in
//! `AppState::apply_project_edit`.
//!
//! ## Scope (P4 MVP)
//!
//! These helpers edit `Section.base.activations` only. Variant-
//! context editing — writing into `SectionVariantOverride.activations`
//! when the user is on a non-base section variant — is queued for
//! P4.x. Callers should refuse the edit when `variant_id != "base"`
//! and `variant_id != section.default_variant` (the call sites
//! enforce this with an `eprintln!`; here we just mutate the base map
//! unconditionally).
//!
//! ## Bar-range invariants
//!
//! Schedule mutations route through the pure helpers in
//! `cell/variant_schedule.rs` and preserve their invariants
//! (sorted by start, non-overlapping, no adjacent same-variant
//! entries).

use rawdaw_model::activation::{ActivationEntry, RealizationParams};
use rawdaw_model::id::{ActivationEntryId, PatternId, SectionId, TrackId, VariantId};
use rawdaw_model::project::Project;

use super::variant_schedule;

/// Bind `pattern_ref` to `(section, track)` in the base activations
/// map. If an entry already exists, its `pattern_ref` is rewritten
/// and the `variant_schedule` is cleared (variants are pattern-
/// specific — keeping stale variant ids when the pattern changes
/// produces stranded entries that the resolver silently drops, but
/// the user surface should be deterministic). If no entry exists,
/// one is allocated with default realization params.
///
/// `pattern_ref = None` keeps the entry but marks the track as
/// "placed but silent" (matches the existing model semantics —
/// rendering shows the row but no events realize). Use
/// [`remove_activation`] to fully delete the entry.
pub fn set_activation_pattern(
    project: &mut Project,
    section_id: SectionId,
    track_id: TrackId,
    pattern_ref: Option<PatternId>,
) {
    let Some(section) = project.sections.get_mut(&section_id) else { return };
    let entry = section
        .base
        .activations
        .entry(track_id)
        .or_insert_with(|| ActivationEntry {
            id: ActivationEntryId::new(0),
            pattern_ref: None,
            variant_schedule: Vec::new(),
            realization: RealizationParams::default(),
            per_note_overrides: Vec::new(),
        });
    if entry.pattern_ref != pattern_ref {
        entry.variant_schedule.clear();
    }
    entry.pattern_ref = pattern_ref;
}

/// Drop the activation entry for `(section, track)` from the base
/// map. Returns `true` if an entry was removed. The track row then
/// renders as the `CellInherit` placeholder.
#[allow(dead_code)]
pub fn remove_activation(
    project: &mut Project,
    section_id: SectionId,
    track_id: TrackId,
) -> bool {
    let Some(section) = project.sections.get_mut(&section_id) else { return false };
    section.base.activations.remove(&track_id).is_some()
}

/// Set the variant pinned at `bar` for `(section, track)`'s activation.
/// `new_variant = None` uncovers `bar` (falls back to the pattern's
/// default variant at realization time). Schedule mutation goes
/// through `variant_schedule::set_variant_for_bar`.
pub fn set_activation_variant_for_bar(
    project: &mut Project,
    section_id: SectionId,
    track_id: TrackId,
    bar: u32,
    new_variant: Option<VariantId>,
) {
    let Some(section) = project.sections.get_mut(&section_id) else { return };
    let Some(entry) = section.base.activations.get_mut(&track_id) else { return };
    let schedule = std::mem::take(&mut entry.variant_schedule);
    entry.variant_schedule = variant_schedule::set_variant_for_bar(schedule, bar, new_variant);
}

/// Merge the range containing `bar` with its left neighbor.
/// See `variant_schedule::merge_range_left` for semantics.
pub fn merge_activation_variant_left(
    project: &mut Project,
    section_id: SectionId,
    track_id: TrackId,
    bar: u32,
) {
    let Some(section) = project.sections.get_mut(&section_id) else { return };
    let Some(entry) = section.base.activations.get_mut(&track_id) else { return };
    let schedule = std::mem::take(&mut entry.variant_schedule);
    entry.variant_schedule = variant_schedule::merge_range_left(schedule, bar);
}

/// Merge the range containing `bar` with its right neighbor.
pub fn merge_activation_variant_right(
    project: &mut Project,
    section_id: SectionId,
    track_id: TrackId,
    bar: u32,
) {
    let Some(section) = project.sections.get_mut(&section_id) else { return };
    let Some(entry) = section.base.activations.get_mut(&track_id) else { return };
    let schedule = std::mem::take(&mut entry.variant_schedule);
    entry.variant_schedule = variant_schedule::merge_range_right(schedule, bar);
}

/// Drop the entire range containing `bar` from the variant schedule.
/// The uncovered bars fall back to the pattern's default variant.
pub fn clear_activation_variant_range(
    project: &mut Project,
    section_id: SectionId,
    track_id: TrackId,
    bar: u32,
) {
    let Some(section) = project.sections.get_mut(&section_id) else { return };
    let Some(entry) = section.base.activations.get_mut(&track_id) else { return };
    let schedule = std::mem::take(&mut entry.variant_schedule);
    entry.variant_schedule = variant_schedule::clear_range_at_bar(schedule, bar);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_model::pattern::PatternBody;
    use rawdaw_model::pitch::PitchClass;
    use rawdaw_model::scale::Scale;
    use rawdaw_model::section::{Section, SectionBody};
    use rawdaw_model::time::BarRange;
    use std::collections::BTreeMap;

    fn empty_project() -> Project {
        Project::new(Scale::major(PitchClass::C))
    }

    fn empty_section(project: &mut Project) -> SectionId {
        let sid = project.id_allocators.alloc_section();
        project.sections.insert(
            sid,
            Section {
                id: sid,
                name: "test".into(),
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
        sid
    }

    fn empty_pitched_pattern(project: &mut Project) -> PatternId {
        super::super::create_pitched_pattern(project)
    }

    #[test]
    fn set_pattern_creates_entry_when_missing() {
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let pid = empty_pitched_pattern(&mut project);
        let tid = TrackId::new(1);
        set_activation_pattern(&mut project, sid, tid, Some(pid));
        let entry = project
            .sections
            .get(&sid)
            .unwrap()
            .base
            .activations
            .get(&tid)
            .expect("entry created");
        assert_eq!(entry.pattern_ref, Some(pid));
        assert!(entry.variant_schedule.is_empty());
    }

    #[test]
    fn set_pattern_rewrite_clears_variant_schedule() {
        // Variants live per-pattern; switching patterns should wipe the
        // schedule (stranded ids would silently fall back at render).
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let pid_a = empty_pitched_pattern(&mut project);
        let pid_b = empty_pitched_pattern(&mut project);
        let tid = TrackId::new(1);
        set_activation_pattern(&mut project, sid, tid, Some(pid_a));
        // Pin a variant on bar 0 with pattern A bound.
        let entry = project
            .sections
            .get_mut(&sid)
            .unwrap()
            .base
            .activations
            .get_mut(&tid)
            .unwrap();
        entry
            .variant_schedule
            .push((BarRange::new(0, 1), VariantId::new("main")));
        // Swap to pattern B; schedule clears.
        set_activation_pattern(&mut project, sid, tid, Some(pid_b));
        let entry = project
            .sections
            .get(&sid)
            .unwrap()
            .base
            .activations
            .get(&tid)
            .unwrap();
        assert_eq!(entry.pattern_ref, Some(pid_b));
        assert!(entry.variant_schedule.is_empty());
    }

    #[test]
    fn set_pattern_same_id_preserves_variant_schedule() {
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let pid = empty_pitched_pattern(&mut project);
        let tid = TrackId::new(1);
        set_activation_pattern(&mut project, sid, tid, Some(pid));
        let entry = project
            .sections
            .get_mut(&sid)
            .unwrap()
            .base
            .activations
            .get_mut(&tid)
            .unwrap();
        entry
            .variant_schedule
            .push((BarRange::new(0, 1), VariantId::new("main")));
        // Re-bind same pattern; schedule survives.
        set_activation_pattern(&mut project, sid, tid, Some(pid));
        let entry = project
            .sections
            .get(&sid)
            .unwrap()
            .base
            .activations
            .get(&tid)
            .unwrap();
        assert_eq!(entry.variant_schedule.len(), 1);
    }

    #[test]
    fn set_pattern_none_keeps_entry_but_silences() {
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let pid = empty_pitched_pattern(&mut project);
        let tid = TrackId::new(1);
        set_activation_pattern(&mut project, sid, tid, Some(pid));
        set_activation_pattern(&mut project, sid, tid, None);
        let entry = project
            .sections
            .get(&sid)
            .unwrap()
            .base
            .activations
            .get(&tid)
            .expect("entry preserved");
        assert!(entry.pattern_ref.is_none());
    }

    #[test]
    fn remove_activation_drops_the_entry() {
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let pid = empty_pitched_pattern(&mut project);
        let tid = TrackId::new(1);
        set_activation_pattern(&mut project, sid, tid, Some(pid));
        assert!(remove_activation(&mut project, sid, tid));
        assert!(!project
            .sections
            .get(&sid)
            .unwrap()
            .base
            .activations
            .contains_key(&tid));
    }

    #[test]
    fn set_activation_variant_for_bar_routes_through_schedule_helper() {
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let pid = empty_pitched_pattern(&mut project);
        let tid = TrackId::new(1);
        set_activation_pattern(&mut project, sid, tid, Some(pid));
        set_activation_variant_for_bar(&mut project, sid, tid, 1, Some(VariantId::new("fill")));
        let entry = project
            .sections
            .get(&sid)
            .unwrap()
            .base
            .activations
            .get(&tid)
            .unwrap();
        assert_eq!(
            entry.variant_schedule,
            vec![(BarRange::new(1, 2), VariantId::new("fill"))],
        );
    }

    #[test]
    fn schedule_mutations_no_op_when_no_activation() {
        // Setting a variant for a bar on an inherit row (no entry yet)
        // should not panic and should not create a phantom entry.
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let tid = TrackId::new(1);
        set_activation_variant_for_bar(&mut project, sid, tid, 0, Some(VariantId::new("fill")));
        assert!(!project
            .sections
            .get(&sid)
            .unwrap()
            .base
            .activations
            .contains_key(&tid));
    }

    #[test]
    fn pattern_body_kind_check_unused_but_keeps_imports_live() {
        // Sanity: the round-trip pattern setup uses the rawdaw-model
        // PatternBody enum, exercised here just to keep the import
        // path explicit. (PatternBody is used in tests by pattern that
        // exercises a typed match — keeps imports live across edits.)
        let mut project = empty_project();
        let pid = empty_pitched_pattern(&mut project);
        let body = &project.patterns.get(&pid).unwrap().body;
        assert!(matches!(body, PatternBody::Pitched(_)));
    }
}
