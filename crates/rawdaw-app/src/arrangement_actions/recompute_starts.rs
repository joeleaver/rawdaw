//! Recompute `SectionRef.start` values from the arrangement's order,
//! honoring per-variant duration overrides. Pure helpers — no signals,
//! no I/O.
//!
//! The v1 arrangement contract is **no gaps, no overlaps**: every
//! step's `start` equals the previous step's `start + step_duration`.
//! See S0 decision 5 in `docs/section-arrangement-editing-plan.md`.
//!
//! [`step_duration`] is exported as the canonical bars-to-musical-time
//! converter for `arrangement_actions` callers; the section + variant
//! lookup logic is intentionally NOT delegated to
//! `realize::variants::effective_duration_bars` (which is
//! `pub(super)` inside `rawdaw-model`) because S4 doesn't need to
//! widen that surface area. The two helpers carry the same one-liner.

#![allow(dead_code)]

use std::collections::BTreeMap;

use rawdaw_model::id::{SectionId, VariantId};
use rawdaw_model::project::Project;
use rawdaw_model::section::{Section, SectionRef};
use rawdaw_model::tempo::TempoMap;
use rawdaw_model::time::MusicalTime;

/// Walk the arrangement assigning each step's `start` to the running
/// cumulative time. The first step lands at [`MusicalTime::ZERO`]; each
/// subsequent step starts where the previous one ends, computed from
/// the previous section's effective `duration_bars` and the tempo map's
/// `beats_per_bar` at the new start.
///
/// Steps referencing a missing section contribute zero duration —
/// `recompute_starts` keeps walking instead of panicking. UI-layer
/// referential integrity (delete-refusal in
/// [`crate::section_actions::delete_section`]) prevents this in
/// practice; the defensive path keeps a stray edit from blowing up the
/// audio thread.
pub fn recompute_starts(project: &mut Project) {
    let mut t = MusicalTime::ZERO;
    let sections = &project.sections;
    let tempo_map = &project.tempo_map;
    for step in &mut project.arrangement.sections {
        step.start = t;
        t = t + step_duration_from(sections, tempo_map, step);
    }
}

/// Effective musical-time duration of a single arrangement step.
/// Resolves the variant override's `duration_bars` if present, else
/// falls back to the base body. Reads `beats_per_bar` from the project's
/// tempo map at the step's stored `start`.
///
/// Returns [`MusicalTime::ZERO`] when the step references a section
/// that doesn't exist in `project.sections`. See [`recompute_starts`]
/// for the defensive-walk rationale.
pub fn step_duration(project: &Project, step: &SectionRef) -> MusicalTime {
    step_duration_from(&project.sections, &project.tempo_map, step)
}

/// Internal split-borrow form of [`step_duration`]. Disjoint borrows on
/// `project.sections`, `project.tempo_map`, and `project.arrangement.
/// sections` let [`recompute_starts`] iterate the arrangement mutably
/// while reading the other two fields immutably.
fn step_duration_from(
    sections: &BTreeMap<SectionId, Section>,
    tempo_map: &TempoMap,
    step: &SectionRef,
) -> MusicalTime {
    let Some(section) = sections.get(&step.section) else {
        return MusicalTime::ZERO;
    };
    let bars = effective_duration_bars(section, &step.variant);
    let beats_per_bar = tempo_map.beats_per_bar_at(step.start);
    MusicalTime::bars(bars as i64, beats_per_bar)
}

/// Local mirror of `realize::variants::effective_duration_bars` —
/// resolves the variant override's `duration_bars` (if `Some`) else
/// returns the base body's `duration_bars`. Kept private here to avoid
/// widening the model's `pub(super)` realize surface.
fn effective_duration_bars(section: &Section, variant: &VariantId) -> u32 {
    section
        .variants
        .get(variant)
        .and_then(|v| v.duration_bars)
        .unwrap_or(section.base.duration_bars)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{
        append_bare_step, empty_project, insert_section_with_bars,
        insert_section_with_variant_override,
    };
    use super::*;

    use rawdaw_model::id::{SectionId, VariantId};
    use rawdaw_model::time::PPQ;

    /// Convenience for the default 4/4 bpb the bare `TempoMap` ships
    /// with — matches `TempoMap::beats_per_bar_at` fallback of 4.
    const DEFAULT_BEATS_PER_BAR: i64 = 4;

    fn bars_to_ticks(bars: u32) -> i64 {
        bars as i64 * DEFAULT_BEATS_PER_BAR * PPQ
    }

    #[test]
    fn recompute_empty_arrangement_is_noop() {
        let mut project = empty_project();
        recompute_starts(&mut project);
        assert!(project.arrangement.sections.is_empty());
    }

    #[test]
    fn recompute_single_step_starts_at_zero() {
        let mut project = empty_project();
        let sid = insert_section_with_bars(&mut project, 4);
        append_bare_step(&mut project, sid, VariantId::base());
        recompute_starts(&mut project);
        assert_eq!(
            project.arrangement.sections[0].start,
            MusicalTime::ZERO,
        );
    }

    #[test]
    fn recompute_multi_step_is_monotonic_non_decreasing() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 8);
        let c = insert_section_with_bars(&mut project, 2);
        append_bare_step(&mut project, a, VariantId::base());
        append_bare_step(&mut project, b, VariantId::base());
        append_bare_step(&mut project, c, VariantId::base());
        recompute_starts(&mut project);
        let starts: Vec<i64> = project
            .arrangement
            .sections
            .iter()
            .map(|s| s.start.as_ticks())
            .collect();
        assert!(starts.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn recompute_uses_base_duration_for_default_variant() {
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let b = insert_section_with_bars(&mut project, 4);
        append_bare_step(&mut project, a, VariantId::base());
        append_bare_step(&mut project, b, VariantId::base());
        recompute_starts(&mut project);
        assert_eq!(
            project.arrangement.sections[1].start.as_ticks(),
            bars_to_ticks(4),
        );
    }

    #[test]
    fn recompute_uses_variant_override_duration() {
        // First step uses an "fill" variant whose duration override is
        // 2 bars (vs base's 8). Second step should start at the override
        // duration, not the base.
        let mut project = empty_project();
        let (a, fill) = insert_section_with_variant_override(&mut project, 8, 2);
        let b = insert_section_with_bars(&mut project, 4);
        append_bare_step(&mut project, a, fill);
        append_bare_step(&mut project, b, VariantId::base());
        recompute_starts(&mut project);
        assert_eq!(
            project.arrangement.sections[1].start.as_ticks(),
            bars_to_ticks(2),
            "variant override duration must drive the next start",
        );
    }

    #[test]
    fn recompute_falls_back_to_base_when_variant_has_no_duration_override() {
        // Variant exists but has duration_bars: None — should fall back
        // to base's 4 bars, NOT collapse to 0.
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        let no_dur = VariantId::new("no-dur");
        project
            .sections
            .get_mut(&a)
            .unwrap()
            .variants
            .insert(no_dur.clone(), Default::default());
        let b = insert_section_with_bars(&mut project, 4);
        append_bare_step(&mut project, a, no_dur);
        append_bare_step(&mut project, b, VariantId::base());
        recompute_starts(&mut project);
        assert_eq!(
            project.arrangement.sections[1].start.as_ticks(),
            bars_to_ticks(4),
        );
    }

    #[test]
    fn recompute_handles_missing_section_gracefully() {
        // A step referencing a section id that isn't in project.sections
        // contributes zero duration. The walk continues so subsequent
        // steps still land at the right cumulative position.
        let mut project = empty_project();
        let a = insert_section_with_bars(&mut project, 4);
        // Step 0: dangling section reference (id never allocated).
        append_bare_step(&mut project, SectionId::new(9999), VariantId::base());
        append_bare_step(&mut project, a, VariantId::base());
        recompute_starts(&mut project);
        assert_eq!(
            project.arrangement.sections[0].start,
            MusicalTime::ZERO,
        );
        assert_eq!(
            project.arrangement.sections[1].start,
            MusicalTime::ZERO,
            "dangling reference contributes zero duration",
        );
    }

    // ---------- step_duration (public helper) ----------

    #[test]
    fn step_duration_uses_base_when_variant_matches_default() {
        let mut project = empty_project();
        let sid = insert_section_with_bars(&mut project, 4);
        append_bare_step(&mut project, sid, VariantId::base());
        let step = &project.arrangement.sections[0];
        assert_eq!(step_duration(&project, step).as_ticks(), bars_to_ticks(4));
    }

    #[test]
    fn step_duration_returns_variant_override_when_set() {
        let mut project = empty_project();
        let (sid, fill) = insert_section_with_variant_override(&mut project, 8, 2);
        append_bare_step(&mut project, sid, fill);
        let step = &project.arrangement.sections[0];
        assert_eq!(step_duration(&project, step).as_ticks(), bars_to_ticks(2));
    }

    #[test]
    fn step_duration_falls_back_to_base_when_variant_omits_duration_override() {
        let mut project = empty_project();
        let sid = insert_section_with_bars(&mut project, 4);
        let no_dur = VariantId::new("no-dur");
        project
            .sections
            .get_mut(&sid)
            .unwrap()
            .variants
            .insert(no_dur.clone(), Default::default());
        append_bare_step(&mut project, sid, no_dur);
        let step = &project.arrangement.sections[0];
        assert_eq!(step_duration(&project, step).as_ticks(), bars_to_ticks(4));
    }

    #[test]
    fn step_duration_returns_zero_for_missing_section() {
        let mut project = empty_project();
        append_bare_step(&mut project, SectionId::new(9999), VariantId::base());
        let step = &project.arrangement.sections[0];
        assert_eq!(step_duration(&project, step), MusicalTime::ZERO);
    }
}
