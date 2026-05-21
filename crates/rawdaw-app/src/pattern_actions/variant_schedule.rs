//! Pure mutations on `Vec<(BarRange, VariantId)>` — the shape of
//! `ActivationEntry.variant_schedule`.
//!
//! Mirrors `section_editor/chord_loop_bar/schedule.rs` (the CL4 chord-
//! loop helper). Same split/insert/merge contract; same invariants:
//! - entries sorted by `BarRange::start`;
//! - non-overlapping ranges;
//! - no adjacent entries sharing the same `VariantId` (those merge via
//!   [`merge_adjacent`]).
//!
//! Lives next to its consumer (`activations.rs`) so the UI side
//! (`section_editor/cell/schedule_column.rs`) imports the
//! `apply_project_edit`-friendly wrappers, not the pure helpers
//! directly.
//!
//! Silence is modeled as the sentinel `VariantId("__silent__")` — see
//! `SILENT_VARIANT_SENTINEL` in `cell/mod.rs`. The helpers here treat
//! that id like any other; only the resolution + display layer
//! interprets it specially.

use rawdaw_model::id::VariantId;
use rawdaw_model::time::BarRange;

/// Replace coverage of `bar` in `schedule` with `new_variant` (or
/// uncover when `None`). Splits any range covering `bar` into
/// before/after slices, inserts the new single-bar entry, then merges
/// adjacent ranges sharing a variant id. Resulting schedule is sorted
/// by `start` with non-overlapping ranges.
pub(super) fn set_variant_for_bar(
    schedule: Vec<(BarRange, VariantId)>,
    bar: u32,
    new_variant: Option<VariantId>,
) -> Vec<(BarRange, VariantId)> {
    let mut split = Vec::with_capacity(schedule.len() + 2);
    for (range, vid) in schedule {
        if range.contains(bar) {
            if range.start < bar {
                split.push((BarRange::new(range.start, bar), vid.clone()));
            }
            if bar + 1 < range.end {
                split.push((BarRange::new(bar + 1, range.end), vid));
            }
        } else {
            split.push((range, vid));
        }
    }
    if let Some(vid) = new_variant {
        split.push((BarRange::new(bar, bar + 1), vid));
    }
    split.sort_by_key(|(r, _)| r.start);
    merge_adjacent(split)
}

/// Extend the immediate-left neighbor's variant over the range
/// containing `bar`. No-op if `bar` is uncovered, if `bar` is in the
/// first range (no left neighbor), or if the left neighbor already
/// uses the same variant id.
pub(super) fn merge_range_left(
    schedule: Vec<(BarRange, VariantId)>,
    bar: u32,
) -> Vec<(BarRange, VariantId)> {
    let Some(covering_idx) = schedule.iter().position(|(r, _)| r.contains(bar)) else {
        return schedule;
    };
    if covering_idx == 0 {
        return schedule;
    }
    let left_vid = schedule[covering_idx - 1].1.clone();
    if schedule[covering_idx].1 == left_vid {
        return schedule;
    }
    let mut out = schedule;
    out[covering_idx].1 = left_vid;
    merge_adjacent(out)
}

/// Symmetric counterpart to [`merge_range_left`].
pub(super) fn merge_range_right(
    schedule: Vec<(BarRange, VariantId)>,
    bar: u32,
) -> Vec<(BarRange, VariantId)> {
    let Some(covering_idx) = schedule.iter().position(|(r, _)| r.contains(bar)) else {
        return schedule;
    };
    if covering_idx + 1 >= schedule.len() {
        return schedule;
    }
    let right_vid = schedule[covering_idx + 1].1.clone();
    if schedule[covering_idx].1 == right_vid {
        return schedule;
    }
    let mut out = schedule;
    out[covering_idx].1 = right_vid;
    merge_adjacent(out)
}

/// Drop the entire range containing `bar` from the schedule. No-op if
/// `bar` is uncovered. "Clear this range" context-menu action —
/// strictly stronger than `set_variant_for_bar(..., None)` which only
/// uncovers a single bar.
pub(super) fn clear_range_at_bar(
    schedule: Vec<(BarRange, VariantId)>,
    bar: u32,
) -> Vec<(BarRange, VariantId)> {
    schedule
        .into_iter()
        .filter(|(r, _)| !r.contains(bar))
        .collect()
}

fn merge_adjacent(
    schedule: Vec<(BarRange, VariantId)>,
) -> Vec<(BarRange, VariantId)> {
    let mut out: Vec<(BarRange, VariantId)> = Vec::with_capacity(schedule.len());
    for (range, vid) in schedule {
        if let Some(last) = out.last_mut()
            && last.1 == vid
            && last.0.end == range.start
        {
            last.0 = BarRange::new(last.0.start, range.end);
            continue;
        }
        out.push((range, vid));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> VariantId {
        VariantId::from(s)
    }

    #[test]
    fn set_variant_into_uncovered_section() {
        let result = set_variant_for_bar(Vec::new(), 2, Some(v("fill")));
        assert_eq!(result, vec![(BarRange::new(2, 3), v("fill"))]);
    }

    #[test]
    fn set_same_variant_at_boundary_merges() {
        let schedule = vec![(BarRange::new(0, 4), v("A"))];
        let result = set_variant_for_bar(schedule, 2, Some(v("A")));
        assert_eq!(result, vec![(BarRange::new(0, 4), v("A"))]);
    }

    #[test]
    fn set_different_variant_splits_range() {
        let schedule = vec![(BarRange::new(0, 4), v("A"))];
        let result = set_variant_for_bar(schedule, 2, Some(v("B")));
        assert_eq!(
            result,
            vec![
                (BarRange::new(0, 2), v("A")),
                (BarRange::new(2, 3), v("B")),
                (BarRange::new(3, 4), v("A")),
            ]
        );
    }

    #[test]
    fn set_none_removes_coverage() {
        let schedule = vec![(BarRange::new(0, 4), v("A"))];
        let result = set_variant_for_bar(schedule, 2, None);
        assert_eq!(
            result,
            vec![
                (BarRange::new(0, 2), v("A")),
                (BarRange::new(3, 4), v("A")),
            ]
        );
    }

    #[test]
    fn set_variant_at_start_of_range() {
        let schedule = vec![(BarRange::new(0, 4), v("A"))];
        let result = set_variant_for_bar(schedule, 0, Some(v("B")));
        assert_eq!(
            result,
            vec![
                (BarRange::new(0, 1), v("B")),
                (BarRange::new(1, 4), v("A")),
            ]
        );
    }

    #[test]
    fn set_variant_at_end_of_range() {
        let schedule = vec![(BarRange::new(0, 4), v("A"))];
        let result = set_variant_for_bar(schedule, 3, Some(v("B")));
        assert_eq!(
            result,
            vec![
                (BarRange::new(0, 3), v("A")),
                (BarRange::new(3, 4), v("B")),
            ]
        );
    }

    #[test]
    fn merges_two_adjacent_ranges_of_same_variant() {
        let schedule = vec![
            (BarRange::new(0, 2), v("A")),
            (BarRange::new(3, 4), v("A")),
        ];
        let result = set_variant_for_bar(schedule, 2, Some(v("A")));
        assert_eq!(result, vec![(BarRange::new(0, 4), v("A"))]);
    }

    #[test]
    fn multi_variant_section_preserves_uncovered_ranges() {
        let schedule = vec![
            (BarRange::new(0, 4), v("A")),
            (BarRange::new(4, 8), v("B")),
        ];
        let result = set_variant_for_bar(schedule, 1, Some(v("C")));
        assert_eq!(
            result,
            vec![
                (BarRange::new(0, 1), v("A")),
                (BarRange::new(1, 2), v("C")),
                (BarRange::new(2, 4), v("A")),
                (BarRange::new(4, 8), v("B")),
            ]
        );
    }

    // ─── merge_range_left / merge_range_right / clear_range_at_bar ─────

    #[test]
    fn merge_range_left_adjacent_collapses_ranges() {
        let schedule = vec![
            (BarRange::new(0, 4), v("A")),
            (BarRange::new(4, 8), v("B")),
        ];
        let result = merge_range_left(schedule, 5);
        assert_eq!(result, vec![(BarRange::new(0, 8), v("A"))]);
    }

    #[test]
    fn merge_range_left_first_range_is_noop() {
        let schedule = vec![
            (BarRange::new(0, 4), v("A")),
            (BarRange::new(4, 8), v("B")),
        ];
        let result = merge_range_left(schedule.clone(), 2);
        assert_eq!(result, schedule);
    }

    #[test]
    fn merge_range_left_uncovered_bar_is_noop() {
        let schedule = vec![
            (BarRange::new(0, 2), v("A")),
            (BarRange::new(4, 8), v("B")),
        ];
        let result = merge_range_left(schedule.clone(), 3);
        assert_eq!(result, schedule);
    }

    #[test]
    fn merge_range_right_adjacent_collapses_ranges() {
        let schedule = vec![
            (BarRange::new(0, 4), v("A")),
            (BarRange::new(4, 8), v("B")),
        ];
        let result = merge_range_right(schedule, 1);
        assert_eq!(result, vec![(BarRange::new(0, 8), v("B"))]);
    }

    #[test]
    fn merge_range_right_last_range_is_noop() {
        let schedule = vec![
            (BarRange::new(0, 4), v("A")),
            (BarRange::new(4, 8), v("B")),
        ];
        let result = merge_range_right(schedule.clone(), 5);
        assert_eq!(result, schedule);
    }

    #[test]
    fn clear_range_at_bar_removes_entire_range() {
        let schedule = vec![
            (BarRange::new(0, 4), v("A")),
            (BarRange::new(4, 8), v("B")),
        ];
        let result = clear_range_at_bar(schedule, 5);
        assert_eq!(result, vec![(BarRange::new(0, 4), v("A"))]);
    }

    #[test]
    fn clear_range_at_bar_uncovered_is_noop() {
        let schedule = vec![
            (BarRange::new(0, 2), v("A")),
            (BarRange::new(4, 8), v("B")),
        ];
        let result = clear_range_at_bar(schedule.clone(), 3);
        assert_eq!(result, schedule);
    }
}
