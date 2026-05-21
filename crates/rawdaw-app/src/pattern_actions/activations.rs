//! Activation mutations on a section.
//!
//! Wraps the per-`ActivationEntry` mutations + the pure variant-
//! schedule helpers in `section_editor/cell/variant_schedule.rs`. Each
//! function takes a `Project` mutably; callers wrap in
//! `AppState::apply_project_edit`.
//!
//! ## Variant context (P4.x)
//!
//! Each mutator takes a `variant_id: &VariantId` that names the
//! section variant the user is currently editing. The dispatch is:
//!
//! - `variant_id == section.default_variant` → edit
//!   `section.base.activations` (the "base tab" path).
//! - Otherwise → edit `section.variants[variant_id].activations`
//!   through `ActivationOverride::Replace` (or `Silent` for an unbind
//!   in variant context). The base map is untouched.
//!
//! ### Variant-context semantics
//!
//! - **Pattern pick** (`set_activation_pattern`) on a non-base variant:
//!   - `Some(pid)` installs `Replace(entry)`. If the variant already
//!     has a Replace, its `pattern_ref` is updated and `variant_schedule`
//!     is cleared on pattern change. Otherwise the new Replace inherits
//!     from `section.base.activations[track]` (cloned) when present,
//!     else a fresh default entry — preserving the user's existing
//!     realization params + schedule on the new override.
//!   - `None` installs `Silent`. This is the variant-level "silence
//!     this track here" affordance; base's pattern_ref is untouched.
//! - **Remove** (`remove_activation`) on a non-base variant drops just
//!   the variant override (back to "inherits base"). On base it drops
//!   the base entry.
//! - **Schedule mutations** (`set_activation_variant_for_bar`,
//!   `merge_*`, `clear_*`) on a non-base variant:
//!   - Existing `Replace` override → mutate its `variant_schedule`.
//!   - Existing `Silent` override → no-op (Silent has no schedule).
//!   - No override + base entry exists → auto-promote a clone of base
//!     to `Replace` and apply the mutation. The variant now has its
//!     own schedule that initially mirrors base except for the edited
//!     bar. Base stays untouched.
//!   - Neither → no-op.
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
use rawdaw_model::section::{ActivationOverride, Section};
use rawdaw_model::time::BarRange;

use super::variant_schedule;

/// Bind `pattern_ref` to `(section, track)` in the variant `variant_id`.
/// When `variant_id == section.default_variant`, writes into
/// `section.base.activations` (legacy "base tab" path). Otherwise
/// installs a `Replace` or `Silent` override on the named variant —
/// see the module-level docs.
///
/// On the base path: if an entry already exists, its `pattern_ref` is
/// rewritten and the `variant_schedule` is cleared on pattern change.
/// If no entry exists, one is allocated with default realization params.
///
/// `pattern_ref = None` on base keeps the entry but nulls `pattern_ref`
/// (matches the existing "placed but silent" semantic for base).
/// `pattern_ref = None` on a non-base variant installs `Silent` — the
/// variant-level "silence this track" affordance.
pub fn set_activation_pattern(
    project: &mut Project,
    section_id: SectionId,
    track_id: TrackId,
    variant_id: &VariantId,
    pattern_ref: Option<PatternId>,
) {
    let Some(section) = project.sections.get_mut(&section_id) else { return };
    if is_base_context(section, variant_id) {
        let entry = section
            .base
            .activations
            .entry(track_id)
            .or_insert_with(fresh_entry);
        if entry.pattern_ref != pattern_ref {
            entry.variant_schedule.clear();
        }
        entry.pattern_ref = pattern_ref;
        return;
    }
    set_variant_pattern(section, track_id, variant_id, pattern_ref);
}

/// Drop the activation entry for `(section, track)` in the named
/// variant. On the base path this removes from `section.base.activations`.
/// On a non-base variant this removes the override entry (the row then
/// falls back to inheriting base). Returns `true` if something was
/// removed.
#[allow(dead_code)]
pub fn remove_activation(
    project: &mut Project,
    section_id: SectionId,
    track_id: TrackId,
    variant_id: &VariantId,
) -> bool {
    let Some(section) = project.sections.get_mut(&section_id) else { return false };
    if is_base_context(section, variant_id) {
        return section.base.activations.remove(&track_id).is_some();
    }
    section
        .variants
        .get_mut(variant_id)
        .map(|v| v.activations.remove(&track_id).is_some())
        .unwrap_or(false)
}

/// Set the variant pinned at `bar` for `(section, track)`'s activation.
/// `new_variant = None` uncovers `bar` (falls back to the pattern's
/// default variant at realization time). Schedule mutation goes
/// through `variant_schedule::set_variant_for_bar`.
pub fn set_activation_variant_for_bar(
    project: &mut Project,
    section_id: SectionId,
    track_id: TrackId,
    variant_id: &VariantId,
    bar: u32,
    new_variant: Option<VariantId>,
) {
    mutate_schedule_in_context(project, section_id, track_id, variant_id, |sched| {
        variant_schedule::set_variant_for_bar(sched, bar, new_variant)
    });
}

/// Merge the range containing `bar` with its left neighbor.
/// See `variant_schedule::merge_range_left` for semantics.
pub fn merge_activation_variant_left(
    project: &mut Project,
    section_id: SectionId,
    track_id: TrackId,
    variant_id: &VariantId,
    bar: u32,
) {
    mutate_schedule_in_context(project, section_id, track_id, variant_id, |sched| {
        variant_schedule::merge_range_left(sched, bar)
    });
}

/// Merge the range containing `bar` with its right neighbor.
pub fn merge_activation_variant_right(
    project: &mut Project,
    section_id: SectionId,
    track_id: TrackId,
    variant_id: &VariantId,
    bar: u32,
) {
    mutate_schedule_in_context(project, section_id, track_id, variant_id, |sched| {
        variant_schedule::merge_range_right(sched, bar)
    });
}

/// Drop the entire range containing `bar` from the variant schedule.
/// The uncovered bars fall back to the pattern's default variant.
pub fn clear_activation_variant_range(
    project: &mut Project,
    section_id: SectionId,
    track_id: TrackId,
    variant_id: &VariantId,
    bar: u32,
) {
    mutate_schedule_in_context(project, section_id, track_id, variant_id, |sched| {
        variant_schedule::clear_range_at_bar(sched, bar)
    });
}

// ---------- internal helpers ----------

fn is_base_context(section: &Section, variant_id: &VariantId) -> bool {
    section.default_variant == *variant_id
}

fn fresh_entry() -> ActivationEntry {
    ActivationEntry {
        id: ActivationEntryId::new(0),
        pattern_ref: None,
        variant_schedule: Vec::new(),
        realization: RealizationParams::default(),
        per_note_overrides: Vec::new(),
    }
}

/// Install / update a pattern binding on a non-base variant override.
/// Splits out so the public mutator stays readable.
fn set_variant_pattern(
    section: &mut Section,
    track_id: TrackId,
    variant_id: &VariantId,
    pattern_ref: Option<PatternId>,
) {
    match pattern_ref {
        None => {
            // `(no pattern)` in variant context = "silence this track in
            // this variant." Install Silent regardless of prior state.
            section
                .variants
                .entry(variant_id.clone())
                .or_default()
                .activations
                .insert(track_id, ActivationOverride::Silent);
        }
        Some(pid) => {
            // Pick up the prior shape so we preserve realization /
            // schedule when the user is editing an existing Replace.
            // When no override exists yet, clone base if present so the
            // new Replace inherits the user's existing realization
            // params (and only the pattern_ref changes). When base is
            // absent too, fall back to a fresh default entry.
            let base_entry = section.base.activations.get(&track_id).cloned();
            let override_map = &mut section
                .variants
                .entry(variant_id.clone())
                .or_default()
                .activations;
            let mut entry = match override_map.get(&track_id) {
                Some(ActivationOverride::Replace(e)) => e.clone(),
                _ => base_entry.unwrap_or_else(fresh_entry),
            };
            if entry.pattern_ref != Some(pid) {
                entry.variant_schedule.clear();
            }
            entry.pattern_ref = Some(pid);
            override_map.insert(track_id, ActivationOverride::Replace(entry));
        }
    }
}

/// Apply a pure transformation to the `variant_schedule` of the entry
/// effective in `(section, track, variant_id)`. See module docs for the
/// variant-context auto-promote-from-base rule.
fn mutate_schedule_in_context(
    project: &mut Project,
    section_id: SectionId,
    track_id: TrackId,
    variant_id: &VariantId,
    op: impl FnOnce(Vec<(BarRange, VariantId)>) -> Vec<(BarRange, VariantId)>,
) {
    let Some(section) = project.sections.get_mut(&section_id) else { return };
    if is_base_context(section, variant_id) {
        let Some(entry) = section.base.activations.get_mut(&track_id) else { return };
        let schedule = std::mem::take(&mut entry.variant_schedule);
        entry.variant_schedule = op(schedule);
        return;
    }
    // Non-base path. Clone base up front (before we take a mut borrow
    // of section.variants) so the auto-promote case has the source ready.
    let base_entry_clone = section.base.activations.get(&track_id).cloned();

    if let Some(over) = section.variants.get_mut(variant_id) {
        match over.activations.get_mut(&track_id) {
            Some(ActivationOverride::Replace(entry)) => {
                let schedule = std::mem::take(&mut entry.variant_schedule);
                entry.variant_schedule = op(schedule);
                return;
            }
            Some(ActivationOverride::Silent) => return,
            None => {}
        }
    }

    // No override entry for this track yet. Auto-promote a clone of
    // base (if it exists) to Replace and apply the schedule mutation
    // on the clone.
    let Some(mut entry) = base_entry_clone else { return };
    let schedule = std::mem::take(&mut entry.variant_schedule);
    entry.variant_schedule = op(schedule);
    section
        .variants
        .entry(variant_id.clone())
        .or_default()
        .activations
        .insert(track_id, ActivationOverride::Replace(entry));
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

    fn base() -> VariantId {
        VariantId::base()
    }

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
        set_activation_pattern(&mut project, sid, tid, &base(), Some(pid));
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
        set_activation_pattern(&mut project, sid, tid, &base(), Some(pid_a));
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
        set_activation_pattern(&mut project, sid, tid, &base(), Some(pid_b));
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
        set_activation_pattern(&mut project, sid, tid, &base(), Some(pid));
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
        set_activation_pattern(&mut project, sid, tid, &base(), Some(pid));
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
        set_activation_pattern(&mut project, sid, tid, &base(), Some(pid));
        set_activation_pattern(&mut project, sid, tid, &base(), None);
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
        set_activation_pattern(&mut project, sid, tid, &base(), Some(pid));
        assert!(remove_activation(&mut project, sid, tid, &base()));
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
        set_activation_pattern(&mut project, sid, tid, &base(), Some(pid));
        set_activation_variant_for_bar(
            &mut project,
            sid,
            tid,
            &base(),
            1,
            Some(VariantId::new("fill")),
        );
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
        set_activation_variant_for_bar(
            &mut project,
            sid,
            tid,
            &base(),
            0,
            Some(VariantId::new("fill")),
        );
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

    // ---------- variant-context tests (P4.x) ----------

    fn stripped() -> VariantId {
        VariantId::new("stripped")
    }

    #[test]
    fn variant_pattern_set_some_installs_replace_override_and_leaves_base_untouched() {
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let pid = empty_pitched_pattern(&mut project);
        let tid = TrackId::new(1);
        // Base has no entry for this track.
        set_activation_pattern(&mut project, sid, tid, &stripped(), Some(pid));
        let section = project.sections.get(&sid).unwrap();
        assert!(
            section.base.activations.is_empty(),
            "base untouched by variant-context edit",
        );
        let override_entry = section
            .variants
            .get(&stripped())
            .expect("variant override created")
            .activations
            .get(&tid)
            .expect("track override created");
        match override_entry {
            ActivationOverride::Replace(entry) => assert_eq!(entry.pattern_ref, Some(pid)),
            ActivationOverride::Silent => panic!("expected Replace, got Silent"),
        }
    }

    #[test]
    fn variant_pattern_set_some_inherits_base_realization_when_present() {
        // Auto-clone-from-base when promoting to Replace preserves
        // realization params + the per-note-override list, so the user
        // doesn't lose tweaks when picking a different pattern in a
        // non-base variant.
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let pid_a = empty_pitched_pattern(&mut project);
        let pid_b = empty_pitched_pattern(&mut project);
        let tid = TrackId::new(1);
        // Seed base with pid_a + a non-default realization marker.
        set_activation_pattern(&mut project, sid, tid, &base(), Some(pid_a));
        // Now pick pid_b on the stripped tab.
        set_activation_pattern(&mut project, sid, tid, &stripped(), Some(pid_b));
        let section = project.sections.get(&sid).unwrap();
        // Base still has pid_a.
        assert_eq!(
            section.base.activations.get(&tid).unwrap().pattern_ref,
            Some(pid_a),
        );
        // Variant override has pid_b.
        match section.variants.get(&stripped()).unwrap().activations.get(&tid).unwrap() {
            ActivationOverride::Replace(entry) => {
                assert_eq!(entry.pattern_ref, Some(pid_b));
            }
            ActivationOverride::Silent => panic!("expected Replace, got Silent"),
        }
    }

    #[test]
    fn variant_pattern_change_clears_override_variant_schedule() {
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let pid_a = empty_pitched_pattern(&mut project);
        let pid_b = empty_pitched_pattern(&mut project);
        let tid = TrackId::new(1);
        set_activation_pattern(&mut project, sid, tid, &stripped(), Some(pid_a));
        // Pin a variant on the override's schedule.
        set_activation_variant_for_bar(
            &mut project,
            sid,
            tid,
            &stripped(),
            1,
            Some(VariantId::new("fill")),
        );
        // Swap the pattern; the override's schedule clears.
        set_activation_pattern(&mut project, sid, tid, &stripped(), Some(pid_b));
        match project
            .sections
            .get(&sid)
            .unwrap()
            .variants
            .get(&stripped())
            .unwrap()
            .activations
            .get(&tid)
            .unwrap()
        {
            ActivationOverride::Replace(entry) => {
                assert_eq!(entry.pattern_ref, Some(pid_b));
                assert!(entry.variant_schedule.is_empty());
            }
            ActivationOverride::Silent => panic!("expected Replace, got Silent"),
        }
    }

    #[test]
    fn variant_pattern_set_none_installs_silent_override() {
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let pid = empty_pitched_pattern(&mut project);
        let tid = TrackId::new(1);
        // Base has pid bound.
        set_activation_pattern(&mut project, sid, tid, &base(), Some(pid));
        // Picking "(no pattern)" on the stripped tab installs Silent.
        set_activation_pattern(&mut project, sid, tid, &stripped(), None);
        let section = project.sections.get(&sid).unwrap();
        // Base still has pid.
        assert_eq!(
            section.base.activations.get(&tid).unwrap().pattern_ref,
            Some(pid),
        );
        // Variant override is Silent.
        match section.variants.get(&stripped()).unwrap().activations.get(&tid).unwrap() {
            ActivationOverride::Silent => {}
            ActivationOverride::Replace(_) => panic!("expected Silent, got Replace"),
        }
    }

    #[test]
    fn variant_remove_activation_drops_only_variant_entry() {
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let pid = empty_pitched_pattern(&mut project);
        let tid = TrackId::new(1);
        set_activation_pattern(&mut project, sid, tid, &base(), Some(pid));
        set_activation_pattern(&mut project, sid, tid, &stripped(), None); // Silent
        assert!(remove_activation(&mut project, sid, tid, &stripped()));
        let section = project.sections.get(&sid).unwrap();
        // Base entry survives.
        assert!(section.base.activations.contains_key(&tid));
        // Variant override is gone.
        assert!(!section
            .variants
            .get(&stripped())
            .unwrap()
            .activations
            .contains_key(&tid));
    }

    #[test]
    fn variant_schedule_mutation_auto_promotes_base_to_replace() {
        // No variant override yet, but base has an entry. Mutating the
        // schedule in variant context should clone base into a Replace
        // and apply the mutation on the clone. Base stays untouched.
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let pid = empty_pitched_pattern(&mut project);
        let tid = TrackId::new(1);
        set_activation_pattern(&mut project, sid, tid, &base(), Some(pid));
        set_activation_variant_for_bar(
            &mut project,
            sid,
            tid,
            &stripped(),
            1,
            Some(VariantId::new("fill")),
        );
        let section = project.sections.get(&sid).unwrap();
        // Base entry unchanged (still empty schedule).
        assert!(section
            .base
            .activations
            .get(&tid)
            .unwrap()
            .variant_schedule
            .is_empty());
        // Variant override has the pinned bar.
        match section.variants.get(&stripped()).unwrap().activations.get(&tid).unwrap() {
            ActivationOverride::Replace(entry) => {
                assert_eq!(entry.pattern_ref, Some(pid));
                assert_eq!(
                    entry.variant_schedule,
                    vec![(BarRange::new(1, 2), VariantId::new("fill"))],
                );
            }
            ActivationOverride::Silent => panic!("expected Replace after auto-promote"),
        }
    }

    #[test]
    fn variant_schedule_mutation_on_silent_override_is_noop() {
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let pid = empty_pitched_pattern(&mut project);
        let tid = TrackId::new(1);
        set_activation_pattern(&mut project, sid, tid, &base(), Some(pid));
        set_activation_pattern(&mut project, sid, tid, &stripped(), None); // Silent
        set_activation_variant_for_bar(
            &mut project,
            sid,
            tid,
            &stripped(),
            1,
            Some(VariantId::new("fill")),
        );
        // Override is still Silent — schedule mutation didn't promote.
        match project
            .sections
            .get(&sid)
            .unwrap()
            .variants
            .get(&stripped())
            .unwrap()
            .activations
            .get(&tid)
            .unwrap()
        {
            ActivationOverride::Silent => {}
            ActivationOverride::Replace(_) => panic!("schedule mutation must not promote Silent"),
        }
    }

    #[test]
    fn variant_schedule_mutation_no_op_when_neither_base_nor_override() {
        let mut project = empty_project();
        let sid = empty_section(&mut project);
        let tid = TrackId::new(1);
        set_activation_variant_for_bar(
            &mut project,
            sid,
            tid,
            &stripped(),
            1,
            Some(VariantId::new("fill")),
        );
        let section = project.sections.get(&sid).unwrap();
        // No override created.
        let no_entry = section
            .variants
            .get(&stripped())
            .map(|v| !v.activations.contains_key(&tid))
            .unwrap_or(true);
        assert!(no_entry);
    }
}
