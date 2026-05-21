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
//!
//! ## Default-fill = absence of entry
//!
//! Bars not covered by any entry render as the pattern's
//! `default_variant`. The `merge_range_*` helpers therefore take a
//! `default_variant` argument: when the user picks "merge with left"
//! on a default-fill bar they expect the gap to adopt the adjacent
//! override's variant id; when they pick it on an explicit entry
//! that's next to a default-fill, they expect the entry to dissolve
//! back into the default. The four segment-merge cases land in
//! [`merge_range_left`] / [`merge_range_right`].

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

/// Extend the immediate-left segment's variant over the segment
/// containing `bar`, treating default-fill ranges as first-class
/// segments. Four cases:
/// 1. `bar` is in the first segment → no-op (no left neighbor).
/// 2. Source segment is default-fill, left is an explicit entry →
///    insert a new entry over the default-fill range carrying the
///    left's variant id.
/// 3. Source segment is an explicit entry, left is default-fill →
///    drop the source entry (its bars revert to default-fill, which
///    is the "left's variant" semantically).
/// 4. Source + left are both explicit entries → rewrite the source
///    entry's variant (and let `merge_adjacent` collapse).
///
/// Same-variant left + source is a no-op in every case.
pub(super) fn merge_range_left(
    schedule: Vec<(BarRange, VariantId)>,
    default_variant: &VariantId,
    bar: u32,
) -> Vec<(BarRange, VariantId)> {
    let Some((src_range, src_effective)) =
        resolve_segment(&schedule, default_variant, bar)
    else {
        return schedule;
    };
    if src_range.start == 0 {
        return schedule;
    }
    let target_effective = effective_variant_at(&schedule, default_variant, src_range.start - 1);
    if target_effective == src_effective {
        return schedule;
    }
    adopt_target(schedule, default_variant, src_range, target_effective)
}

/// Symmetric counterpart to [`merge_range_left`]. The "right neighbor"
/// of the source segment is the bar at `src_range.end`. If no segment
/// covers `src_range.end` (the source segment runs to the end of the
/// schedule's covered range with no entry beyond), the helper no-ops:
/// without `total_bars` plumbed through we can't tell whether there's
/// a trailing default-fill segment to merge into, so we conservatively
/// no-op.
pub(super) fn merge_range_right(
    schedule: Vec<(BarRange, VariantId)>,
    default_variant: &VariantId,
    bar: u32,
) -> Vec<(BarRange, VariantId)> {
    let Some((src_range, src_effective)) =
        resolve_segment(&schedule, default_variant, bar)
    else {
        return schedule;
    };
    // Does any entry start at or after src_range.end? If yes, we know
    // there's a segment to the right (either covered or a default-fill
    // gap before it); if no, there are no more entries to anchor a
    // right neighbor — no-op.
    if !schedule.iter().any(|(r, _)| r.start >= src_range.end) {
        return schedule;
    }
    let target_effective = effective_variant_at(&schedule, default_variant, src_range.end);
    if target_effective == src_effective {
        return schedule;
    }
    adopt_target(schedule, default_variant, src_range, target_effective)
}

/// Drop the entire range containing `bar` from the schedule. No-op if
/// `bar` is uncovered. "Clear this range" context-menu action —
/// strictly stronger than `set_variant_for_bar(..., None)` which only
/// uncovers a single bar. On a default-fill bar this is already a
/// no-op (the bar is already cleared).
pub(super) fn clear_range_at_bar(
    schedule: Vec<(BarRange, VariantId)>,
    bar: u32,
) -> Vec<(BarRange, VariantId)> {
    schedule
        .into_iter()
        .filter(|(r, _)| !r.contains(bar))
        .collect()
}

// ---------- internal helpers ----------

/// Identify the segment containing `bar`. Returns `(range, effective_vid)`.
/// For a covered bar that's the entry's range + its vid. For an
/// uncovered bar that's the maximal default-fill range containing
/// `bar` paired with `default_variant`. Returns `None` only if the
/// schedule is empty AND we have no way to anchor the source range —
/// which actually still produces a default-fill segment, so this
/// always returns `Some` in practice; the `Option` shape is kept so
/// future bounds-checks (e.g., bar past `total_bars`) can refuse.
fn resolve_segment(
    schedule: &[(BarRange, VariantId)],
    default_variant: &VariantId,
    bar: u32,
) -> Option<(BarRange, VariantId)> {
    if let Some((r, v)) = schedule.iter().find(|(r, _)| r.contains(bar)) {
        return Some((*r, v.clone()));
    }
    // Uncovered — walk left/right to find the maximal default-fill
    // range containing `bar`. Left edge = previous entry's `end`
    // (or 0); right edge = next entry's `start` (or `u32::MAX` as a
    // sentinel; merge_right guards against the "no next entry" case
    // separately by checking the schedule contains any entry past
    // src_range.end).
    let start = schedule
        .iter()
        .map(|(r, _)| r.end)
        .filter(|&e| e <= bar)
        .max()
        .unwrap_or(0);
    let end = schedule
        .iter()
        .map(|(r, _)| r.start)
        .filter(|&s| s > bar)
        .min()
        .unwrap_or(u32::MAX);
    Some((BarRange::new(start, end), default_variant.clone()))
}

/// Variant covering `bar` — either the explicit entry's vid or the
/// pattern's default_variant for uncovered bars.
fn effective_variant_at(
    schedule: &[(BarRange, VariantId)],
    default_variant: &VariantId,
    bar: u32,
) -> VariantId {
    schedule
        .iter()
        .find(|(r, _)| r.contains(bar))
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| default_variant.clone())
}

/// Rewrite `src_range` to carry `target_vid`. If `target_vid` equals
/// `default_variant`, the rewrite is "drop any entries that cover
/// src_range so it falls back to default-fill." Otherwise we drop any
/// covering entries and insert a fresh single entry for `src_range`.
/// The `merge_adjacent` pass collapses neighbours that now share a vid.
fn adopt_target(
    schedule: Vec<(BarRange, VariantId)>,
    default_variant: &VariantId,
    src_range: BarRange,
    target_vid: VariantId,
) -> Vec<(BarRange, VariantId)> {
    let mut out: Vec<(BarRange, VariantId)> = schedule
        .into_iter()
        .filter(|(r, _)| !ranges_overlap(*r, src_range))
        .collect();
    if target_vid != *default_variant {
        out.push((src_range, target_vid));
    }
    out.sort_by_key(|(r, _)| r.start);
    merge_adjacent(out)
}

fn ranges_overlap(a: BarRange, b: BarRange) -> bool {
    a.start < b.end && b.start < a.end
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

    fn main() -> VariantId {
        v("main")
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
    fn merge_range_left_adjacent_explicit_collapses_ranges() {
        let schedule = vec![
            (BarRange::new(0, 4), v("A")),
            (BarRange::new(4, 8), v("B")),
        ];
        let result = merge_range_left(schedule, &main(), 5);
        assert_eq!(result, vec![(BarRange::new(0, 8), v("A"))]);
    }

    #[test]
    fn merge_range_left_first_range_is_noop() {
        let schedule = vec![
            (BarRange::new(0, 4), v("A")),
            (BarRange::new(4, 8), v("B")),
        ];
        let result = merge_range_left(schedule.clone(), &main(), 2);
        assert_eq!(result, schedule);
    }

    #[test]
    fn merge_range_left_default_fill_adopts_left_variant() {
        // schedule covers 0..2 with "fill"; bar 3 is default-fill;
        // segment containing 3 is 2..(end-of-schedule). Merge-left
        // should make 2..(...) adopt "fill" (extending the entry).
        let schedule = vec![(BarRange::new(0, 2), v("fill"))];
        let result = merge_range_left(schedule, &main(), 3);
        // Default-fill segment is 2..u32::MAX (no right edge). After
        // adopting "fill", the entry becomes 0..u32::MAX (collapsed).
        assert_eq!(result, vec![(BarRange::new(0, u32::MAX), v("fill"))]);
    }

    #[test]
    fn merge_range_left_explicit_at_zero_is_noop() {
        // Source is the entry at 0..2; left is default-fill but there
        // are no bars before 0 — no-op.
        let schedule = vec![(BarRange::new(0, 2), v("fill"))];
        let result = merge_range_left(schedule.clone(), &main(), 0);
        assert_eq!(result, schedule);
    }

    #[test]
    fn merge_range_left_explicit_into_default_fill_drops_entry() {
        // Schedule: 0..2 default-fill, 2..4 "fill". Merge-left at bar 2
        // should make 2..4 adopt default ("main") — i.e., the entry
        // drops out so the schedule becomes empty (or just lacks that
        // range).
        let schedule = vec![(BarRange::new(2, 4), v("fill"))];
        let result = merge_range_left(schedule, &main(), 2);
        assert_eq!(result, Vec::new());
    }

    #[test]
    fn merge_range_left_same_variant_is_noop() {
        // Source default-fill, left is "main" (= default) — both
        // already effectively "main" → no-op.
        let schedule = vec![(BarRange::new(0, 2), v("main"))]; // same as default
        // Edge case: an entry whose vid equals default_variant is a
        // weird-but-legal shape. Bar 3 is uncovered; left segment
        // covers 0..2 with vid "main", same effective vid as our
        // default-fill source → no-op.
        let result = merge_range_left(schedule.clone(), &main(), 3);
        assert_eq!(result, schedule);
    }

    #[test]
    fn merge_range_right_adjacent_explicit_collapses_ranges() {
        let schedule = vec![
            (BarRange::new(0, 4), v("A")),
            (BarRange::new(4, 8), v("B")),
        ];
        let result = merge_range_right(schedule, &main(), 1);
        assert_eq!(result, vec![(BarRange::new(0, 8), v("B"))]);
    }

    #[test]
    fn merge_range_right_last_range_is_noop() {
        let schedule = vec![
            (BarRange::new(0, 4), v("A")),
            (BarRange::new(4, 8), v("B")),
        ];
        let result = merge_range_right(schedule.clone(), &main(), 5);
        assert_eq!(result, schedule);
    }

    #[test]
    fn merge_range_right_default_fill_adopts_right_variant() {
        // Schedule: 4..8 = "fill"; bars 0..4 are default-fill.
        // Merge-right at bar 1 should fold 0..4 into the right's
        // "fill" entry.
        let schedule = vec![(BarRange::new(4, 8), v("fill"))];
        let result = merge_range_right(schedule, &main(), 1);
        assert_eq!(result, vec![(BarRange::new(0, 8), v("fill"))]);
    }

    #[test]
    fn merge_range_right_explicit_into_default_fill_drops_entry() {
        // Schedule: 0..2 = "fill"; bars >= 2 are default-fill but
        // there are NO entries past 0..2. merge_right_at_bar=1
        // conservatively no-ops because we can't tell if there's a
        // trailing default-fill segment to merge into (no
        // total_bars passed in).
        let schedule = vec![(BarRange::new(0, 2), v("fill"))];
        let result = merge_range_right(schedule.clone(), &main(), 1);
        assert_eq!(result, schedule, "no entries past source → conservative no-op");
    }

    #[test]
    fn merge_range_right_explicit_with_trailing_entry_drops_source() {
        // Schedule: 0..2 = "fill"; 4..8 = "fill". Bar 1 is in 0..2;
        // its right segment is 2..4 (default-fill, since the gap is
        // uncovered). After merge_right, 0..2 adopts default → entry
        // dissolves. The 4..8 entry stays.
        let schedule = vec![
            (BarRange::new(0, 2), v("fill")),
            (BarRange::new(4, 8), v("fill")),
        ];
        let result = merge_range_right(schedule, &main(), 1);
        assert_eq!(result, vec![(BarRange::new(4, 8), v("fill"))]);
    }

    #[test]
    fn merge_range_right_default_fill_at_end_no_op() {
        // schedule has 0..2 = "fill"; bar 3 is default-fill. No entry
        // exists past 0..2, so merge_right has nothing on the right
        // to fold into.
        let schedule = vec![(BarRange::new(0, 2), v("fill"))];
        let result = merge_range_right(schedule.clone(), &main(), 3);
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
