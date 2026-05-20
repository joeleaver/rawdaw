//! Pure mutations on `Vec<(BarRange, ChordLoopId)>` — the
//! `Section.base.chord_loops` schedule shape.
//!
//! Lives in its own sub-module so the UI side (`mod.rs`, ~570
//! lines of rsx + commit handlers) stays under the 700-line cap.
//! Every function here is pure (no model lookups, no signal reads,
//! no edit-pump calls) — the UI's `commit_*` wrappers route them
//! through `apply_project_edit`.
//!
//! Schedule invariant: entries are sorted by `BarRange::start`, do
//! not overlap, and never have adjacent same-`ChordLoopId` entries
//! (those merge via [`merge_adjacent`]).

use rawdaw_model::id::ChordLoopId;
use rawdaw_model::time::BarRange;

/// Replace coverage of `bar` in `schedule` with `new_loop` (or
/// uncover when `None`). Splits any range covering `bar` into
/// before/after slices, inserts the new single-bar entry, then
/// merges adjacent ranges sharing a loop id. Resulting schedule is
/// sorted by `start` with non-overlapping ranges.
pub(crate) fn set_loop_for_bar(
    schedule: Vec<(BarRange, ChordLoopId)>,
    bar: u32,
    new_loop: Option<ChordLoopId>,
) -> Vec<(BarRange, ChordLoopId)> {
    let mut split = Vec::with_capacity(schedule.len() + 2);
    for (range, loop_id) in schedule {
        if range.contains(bar) {
            if range.start < bar {
                split.push((BarRange::new(range.start, bar), loop_id));
            }
            if bar + 1 < range.end {
                split.push((BarRange::new(bar + 1, range.end), loop_id));
            }
        } else {
            split.push((range, loop_id));
        }
    }
    if let Some(id) = new_loop {
        split.push((BarRange::new(bar, bar + 1), id));
    }
    split.sort_by_key(|(r, _)| r.start);
    merge_adjacent(split)
}

/// Extend the immediate-left neighbor's loop over the range
/// containing `bar`. No-op if `bar` is uncovered, if `bar` is in the
/// first range (no left neighbor), or if the left neighbor already
/// uses the same loop id. CL4.x context-menu shortcut for the
/// common case of "remove this distinct range and let the left
/// section spread into it." Same auto-merge semantics as
/// [`set_loop_for_bar`] once the loop id matches.
pub(crate) fn merge_range_left(
    schedule: Vec<(BarRange, ChordLoopId)>,
    bar: u32,
) -> Vec<(BarRange, ChordLoopId)> {
    let Some(covering_idx) = schedule.iter().position(|(r, _)| r.contains(bar)) else {
        return schedule;
    };
    if covering_idx == 0 {
        return schedule;
    }
    let left_loop = schedule[covering_idx - 1].1;
    if schedule[covering_idx].1 == left_loop {
        return schedule;
    }
    let mut out = schedule;
    out[covering_idx].1 = left_loop;
    merge_adjacent(out)
}

/// Symmetric counterpart to [`merge_range_left`]. Extends the
/// immediate-right neighbor's loop over the range containing `bar`.
pub(crate) fn merge_range_right(
    schedule: Vec<(BarRange, ChordLoopId)>,
    bar: u32,
) -> Vec<(BarRange, ChordLoopId)> {
    let Some(covering_idx) = schedule.iter().position(|(r, _)| r.contains(bar)) else {
        return schedule;
    };
    if covering_idx + 1 >= schedule.len() {
        return schedule;
    }
    let right_loop = schedule[covering_idx + 1].1;
    if schedule[covering_idx].1 == right_loop {
        return schedule;
    }
    let mut out = schedule;
    out[covering_idx].1 = right_loop;
    merge_adjacent(out)
}

/// Uncover the entire range containing `bar` (drop the covering
/// entry from the schedule). No-op if `bar` is uncovered. CL4.x's
/// "Clear this range" context-menu action — strictly stronger than
/// `set_loop_for_bar(..., None)` which only uncovers a single bar.
pub(crate) fn clear_range_at_bar(
    schedule: Vec<(BarRange, ChordLoopId)>,
    bar: u32,
) -> Vec<(BarRange, ChordLoopId)> {
    schedule
        .into_iter()
        .filter(|(r, _)| !r.contains(bar))
        .collect()
}

fn merge_adjacent(
    schedule: Vec<(BarRange, ChordLoopId)>,
) -> Vec<(BarRange, ChordLoopId)> {
    let mut out: Vec<(BarRange, ChordLoopId)> = Vec::with_capacity(schedule.len());
    for (range, loop_id) in schedule {
        if let Some(last) = out.last_mut()
            && last.1 == loop_id
            && last.0.end == range.start
        {
            last.0 = BarRange::new(last.0.start, range.end);
            continue;
        }
        out.push((range, loop_id));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u64) -> ChordLoopId {
        ChordLoopId::new(n)
    }

    #[test]
    fn set_loop_into_uncovered_section() {
        let result = set_loop_for_bar(Vec::new(), 2, Some(id(1)));
        assert_eq!(result, vec![(BarRange::new(2, 3), id(1))]);
    }

    #[test]
    fn set_same_loop_at_boundary_merges() {
        // Existing: bars 0–4 = loop A. Set bar 2 to A → still one range 0–4.
        let schedule = vec![(BarRange::new(0, 4), id(1))];
        let result = set_loop_for_bar(schedule, 2, Some(id(1)));
        assert_eq!(result, vec![(BarRange::new(0, 4), id(1))]);
    }

    #[test]
    fn set_different_loop_splits_range() {
        // Existing: bars 0–4 = loop A. Set bar 2 to B → split into
        // 0–2=A, 2–3=B, 3–4=A.
        let schedule = vec![(BarRange::new(0, 4), id(1))];
        let result = set_loop_for_bar(schedule, 2, Some(id(2)));
        assert_eq!(
            result,
            vec![
                (BarRange::new(0, 2), id(1)),
                (BarRange::new(2, 3), id(2)),
                (BarRange::new(3, 4), id(1)),
            ]
        );
    }

    #[test]
    fn set_none_removes_coverage() {
        // Existing: bars 0–4 = loop A. Clear bar 2 → 0–2=A, 3–4=A.
        let schedule = vec![(BarRange::new(0, 4), id(1))];
        let result = set_loop_for_bar(schedule, 2, None);
        assert_eq!(
            result,
            vec![
                (BarRange::new(0, 2), id(1)),
                (BarRange::new(3, 4), id(1)),
            ]
        );
    }

    #[test]
    fn set_loop_at_start_of_range() {
        // Existing: bars 0–4 = A. Set bar 0 to B → 0–1=B, 1–4=A.
        let schedule = vec![(BarRange::new(0, 4), id(1))];
        let result = set_loop_for_bar(schedule, 0, Some(id(2)));
        assert_eq!(
            result,
            vec![
                (BarRange::new(0, 1), id(2)),
                (BarRange::new(1, 4), id(1)),
            ]
        );
    }

    #[test]
    fn set_loop_at_end_of_range() {
        // Existing: bars 0–4 = A. Set bar 3 to B → 0–3=A, 3–4=B.
        let schedule = vec![(BarRange::new(0, 4), id(1))];
        let result = set_loop_for_bar(schedule, 3, Some(id(2)));
        assert_eq!(
            result,
            vec![
                (BarRange::new(0, 3), id(1)),
                (BarRange::new(3, 4), id(2)),
            ]
        );
    }

    #[test]
    fn merges_two_adjacent_ranges_of_same_loop() {
        // Existing: 0–2=A, 3–4=A (gap at bar 2). Set bar 2 to A →
        // 0–4=A (single merged range).
        let schedule = vec![
            (BarRange::new(0, 2), id(1)),
            (BarRange::new(3, 4), id(1)),
        ];
        let result = set_loop_for_bar(schedule, 2, Some(id(1)));
        assert_eq!(result, vec![(BarRange::new(0, 4), id(1))]);
    }

    #[test]
    fn multi_loop_section_preserves_uncovered_ranges() {
        // Existing: 0–4=A, 4–8=B. Set bar 1 to C → 0–1=A, 1–2=C, 2–4=A, 4–8=B.
        let schedule = vec![
            (BarRange::new(0, 4), id(1)),
            (BarRange::new(4, 8), id(2)),
        ];
        let result = set_loop_for_bar(schedule, 1, Some(id(3)));
        assert_eq!(
            result,
            vec![
                (BarRange::new(0, 1), id(1)),
                (BarRange::new(1, 2), id(3)),
                (BarRange::new(2, 4), id(1)),
                (BarRange::new(4, 8), id(2)),
            ]
        );
    }

    // ─── CL4.x: merge_range_left / merge_range_right / clear_range_at_bar ──

    #[test]
    fn merge_range_left_adjacent_collapses_ranges() {
        // Adjacent: 0–4=A, 4–8=B. Merge bar 5 (in B) left → 0–8=A.
        let schedule = vec![
            (BarRange::new(0, 4), id(1)),
            (BarRange::new(4, 8), id(2)),
        ];
        let result = merge_range_left(schedule, 5);
        assert_eq!(result, vec![(BarRange::new(0, 8), id(1))]);
    }

    #[test]
    fn merge_range_left_with_gap_preserves_gap() {
        // Non-adjacent: 0–2=A, gap 2–4, 4–8=B. Merge bar 5 left → A
        // applied to 4–8 without bridging the gap.
        let schedule = vec![
            (BarRange::new(0, 2), id(1)),
            (BarRange::new(4, 8), id(2)),
        ];
        let result = merge_range_left(schedule, 5);
        assert_eq!(
            result,
            vec![
                (BarRange::new(0, 2), id(1)),
                (BarRange::new(4, 8), id(1)),
            ]
        );
    }

    #[test]
    fn merge_range_left_first_range_is_noop() {
        // No left neighbor for the first range.
        let schedule = vec![
            (BarRange::new(0, 4), id(1)),
            (BarRange::new(4, 8), id(2)),
        ];
        let result = merge_range_left(schedule.clone(), 2);
        assert_eq!(result, schedule);
    }

    #[test]
    fn merge_range_left_same_loop_is_noop() {
        // The covering range already uses the left neighbor's loop;
        // nothing to do.
        let schedule = vec![
            (BarRange::new(0, 2), id(1)),
            (BarRange::new(2, 4), id(1)),
            (BarRange::new(4, 8), id(2)),
        ];
        // merge_adjacent normally prevents this state from arising,
        // but the helper still no-ops if presented with it.
        let result = merge_range_left(schedule.clone(), 3);
        assert_eq!(result, schedule);
    }

    #[test]
    fn merge_range_left_uncovered_bar_is_noop() {
        // bar 3 is in the gap between two ranges; nothing covers it.
        let schedule = vec![
            (BarRange::new(0, 2), id(1)),
            (BarRange::new(4, 8), id(2)),
        ];
        let result = merge_range_left(schedule.clone(), 3);
        assert_eq!(result, schedule);
    }

    #[test]
    fn merge_range_right_adjacent_collapses_ranges() {
        // Adjacent: 0–4=A, 4–8=B. Merge bar 1 (in A) right → 0–8=B.
        let schedule = vec![
            (BarRange::new(0, 4), id(1)),
            (BarRange::new(4, 8), id(2)),
        ];
        let result = merge_range_right(schedule, 1);
        assert_eq!(result, vec![(BarRange::new(0, 8), id(2))]);
    }

    #[test]
    fn merge_range_right_last_range_is_noop() {
        // No right neighbor for the last range.
        let schedule = vec![
            (BarRange::new(0, 4), id(1)),
            (BarRange::new(4, 8), id(2)),
        ];
        let result = merge_range_right(schedule.clone(), 5);
        assert_eq!(result, schedule);
    }

    #[test]
    fn merge_range_right_uncovered_bar_is_noop() {
        let schedule = vec![
            (BarRange::new(0, 2), id(1)),
            (BarRange::new(4, 8), id(2)),
        ];
        let result = merge_range_right(schedule.clone(), 3);
        assert_eq!(result, schedule);
    }

    #[test]
    fn clear_range_at_bar_removes_entire_range() {
        // 0–4=A, 4–8=B. Clear bar 5 (in B) → only 0–4=A remains.
        let schedule = vec![
            (BarRange::new(0, 4), id(1)),
            (BarRange::new(4, 8), id(2)),
        ];
        let result = clear_range_at_bar(schedule, 5);
        assert_eq!(result, vec![(BarRange::new(0, 4), id(1))]);
    }

    #[test]
    fn clear_range_at_bar_uncovered_is_noop() {
        let schedule = vec![
            (BarRange::new(0, 2), id(1)),
            (BarRange::new(4, 8), id(2)),
        ];
        let result = clear_range_at_bar(schedule.clone(), 3);
        assert_eq!(result, schedule);
    }

    #[test]
    fn clear_range_at_bar_removes_only_matching_range() {
        // Multi-bar middle range cleared; surrounding ranges preserved.
        let schedule = vec![
            (BarRange::new(0, 2), id(1)),
            (BarRange::new(2, 6), id(2)),
            (BarRange::new(6, 8), id(3)),
        ];
        let result = clear_range_at_bar(schedule, 4);
        assert_eq!(
            result,
            vec![
                (BarRange::new(0, 2), id(1)),
                (BarRange::new(6, 8), id(3)),
            ]
        );
    }
}
