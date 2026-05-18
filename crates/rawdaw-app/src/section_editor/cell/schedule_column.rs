//! Cell column 3 — variant schedule.
//!
//! Wraps the standalone [`ScheduleTimeline`](crate::parts::ScheduleTimeline)
//! primitive and adds:
//!
//! - a section header captioned "Variant schedule";
//! - a legend listing the non-default segments (filter `!is_default`);
//! - a small italic hint about sub-range editing, shown only when the
//!   activation has at least one non-default segment.
//!
//! ## Segment builder — render-time gap-fill
//!
//! The fixture stores only the deviations from the pattern's default
//! variant: a sparse `[ScheduleEntry]` with `(start_bar, end_bar,
//! Option<variant_id>)`. The render-time builder walks that list,
//! sorts by `start_bar` defensively, and gap-fills the missing ranges
//! with implicit-default segments. No phantom default entry ever
//! lives in the model side — see the round-2 README port-time notes
//! and `CLAUDE.md` rule 1.

use rinch::prelude::*;

use crate::fixture::ScheduleEntry;
use crate::parts::{
    rgba, Icon, ScheduleSegment, ScheduleSegmentStyle, ScheduleTimeline,
};
use crate::theme;

#[component]
pub fn ScheduleColumn(
    schedule: Vec<ScheduleEntry>,
    total_bars: u32,
    pattern_color: String,
    pattern_default_variant: String,
    silent: bool,
) -> NodeHandle {
    let col_style =
        "padding: 12px 14px; display: flex; flex-direction: column; gap: 8px;".to_string();
    let title_style = format!(
        "font-size: 10.5px; letter-spacing: 0.6px; text-transform: uppercase; \
         color: {text2}; font-weight: 600;",
        text2 = theme::TEXT2,
    );
    // Build segments for the timeline + the legend display.
    let segments = build_segments(&schedule, total_bars, pattern_default_variant.as_str());
    let legend_entries: Vec<LegendEntry> = segments
        .iter()
        .filter_map(LegendEntry::from_segment)
        .collect();
    let show_hint = !legend_entries.is_empty();
    let segments_for_timeline = segments.clone();
    let pattern_color_for_timeline = pattern_color.clone();
    let pattern_color_for_legend = pattern_color.clone();
    let legend_for_iter = legend_entries.clone();

    // Same `Fn`-source rule as parts::ScheduleTimeline — `.clone()`
    // inside the for source rebuilds the Vec from the captured
    // borrow on each render tick.
    rsx! {
        div { style: {col_style.clone()},
            div { style: {title_style.clone()}, "Variant schedule" }
            ScheduleTimeline {
                total_bars: total_bars,
                segments: segments_for_timeline,
                pattern_color: pattern_color_for_timeline,
                silent: silent,
            }
            if show_hint {
                ScheduleHint { }
            }
            for entry in legend_for_iter.clone() {
                LegendChip {
                    key: entry.start_bar,
                    entry: entry,
                    pattern_color: pattern_color_for_legend.clone(),
                }
            }
        }
    }
}

/// Build the fully-populated segment list from the sparse fixture
/// schedule. Sorts entries by `start_bar` (defensive — the fixture
/// already orders them, but we don't want a future re-order to silently
/// break rendering), then walks the bar axis filling
/// implicit-default segments around the explicit entries.
pub(crate) fn build_segments(
    schedule: &[ScheduleEntry],
    total_bars: u32,
    default_variant: &str,
) -> Vec<ScheduleSegment> {
    let mut sorted: Vec<ScheduleEntry> = schedule.to_vec();
    sorted.sort_by_key(|e| e.start_bar);

    let mut out: Vec<ScheduleSegment> = Vec::with_capacity(sorted.len() * 2 + 1);
    let mut cursor: u32 = 0;
    for entry in sorted {
        if entry.end_bar <= entry.start_bar {
            // Defensive: zero-or-negative span is a fixture bug;
            // skip rather than emit a zero-width block.
            continue;
        }
        let start = entry.start_bar.min(total_bars);
        let end = entry.end_bar.min(total_bars);
        if start > cursor {
            out.push(default_segment(cursor, start, default_variant));
        }
        if end > start {
            out.push(match entry.variant {
                None => ScheduleSegment {
                    start_bar: start,
                    end_bar: end,
                    style: ScheduleSegmentStyle::Silent,
                    label: "silent".to_string(),
                },
                Some(v) if v == default_variant => default_segment(start, end, default_variant),
                Some(v) => ScheduleSegment {
                    start_bar: start,
                    end_bar: end,
                    style: ScheduleSegmentStyle::NonDefault,
                    label: v.to_string(),
                },
            });
        }
        cursor = end.max(cursor);
    }
    if cursor < total_bars {
        out.push(default_segment(cursor, total_bars, default_variant));
    }
    out
}

fn default_segment(start_bar: u32, end_bar: u32, label: &str) -> ScheduleSegment {
    ScheduleSegment {
        start_bar,
        end_bar,
        style: ScheduleSegmentStyle::DefaultFill,
        label: label.to_string(),
    }
}

#[component]
fn ScheduleHint() -> NodeHandle {
    let style = format!(
        "font-size: 10.5px; color: {text3}; font-style: italic;",
        text3 = theme::TEXT3,
    );
    rsx! {
        div { style: {style.clone()},
            "click a sub-range to assign / silence a pattern variant"
        }
    }
}

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub(crate) struct LegendEntry {
    pub start_bar: u32,
    pub end_bar: u32,
    pub kind: LegendKind,
    pub variant_label: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) enum LegendKind {
    #[default]
    Named,
    Silent,
}

impl LegendEntry {
    fn from_segment(seg: &ScheduleSegment) -> Option<LegendEntry> {
        match seg.style {
            ScheduleSegmentStyle::NonDefault => Some(LegendEntry {
                start_bar: seg.start_bar,
                end_bar: seg.end_bar,
                kind: LegendKind::Named,
                variant_label: seg.label.clone(),
            }),
            ScheduleSegmentStyle::Silent => Some(LegendEntry {
                start_bar: seg.start_bar,
                end_bar: seg.end_bar,
                kind: LegendKind::Silent,
                variant_label: "silent".to_string(),
            }),
            ScheduleSegmentStyle::DefaultFill => None,
        }
    }
}

#[component]
fn LegendChip(entry: LegendEntry, pattern_color: String) -> NodeHandle {
    let range_label = if entry.end_bar - entry.start_bar == 1 {
        format!("bar {}", entry.start_bar + 1)
    } else {
        // Both endpoints are 1-indexed in the UI; end_bar is exclusive
        // internally, so the display end is `end_bar` (not end_bar+1).
        format!("bar {}–{}", entry.start_bar + 1, entry.end_bar)
    };
    let style = match entry.kind {
        LegendKind::Named => format!(
            "display: inline-flex; align-items: center; gap: 6px; \
             padding: 2px 8px; border-radius: 3px; \
             background: {bg}; border: 1px solid {border}; \
             color: {text1}; font-size: 11px;",
            bg = rgba(pattern_color.as_str(), 0.14),
            border = rgba(pattern_color.as_str(), 0.40),
            text1 = theme::TEXT1,
        ),
        LegendKind::Silent => format!(
            "display: inline-flex; align-items: center; gap: 6px; \
             padding: 2px 8px; border-radius: 3px; \
             background: transparent; border: 1px dashed {border}; \
             color: {text2}; font-size: 11px;",
            border = theme::TEXT3,
            text2 = theme::TEXT2,
        ),
    };
    let label = format!("{} · {}", entry.variant_label, range_label);
    let icon_stroke = match entry.kind {
        LegendKind::Named => theme::TEXT1.to_string(),
        LegendKind::Silent => theme::TEXT3.to_string(),
    };
    let icon_size = 9.0_f32;
    let icon_w = 1.6_f32;
    let glyph = match entry.kind {
        LegendKind::Named => "dot".to_string(),
        LegendKind::Silent => "minus".to_string(), // closest available "eye-off" stand-in
    };

    rsx! {
        div { style: {style.clone()},
            Icon { glyph: glyph, size: icon_size,
                   stroke: {icon_stroke.clone()}, stroke_width: icon_w }
            span { {label.clone()} }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(start: u32, end: u32, variant: Option<&'static str>) -> ScheduleEntry {
        ScheduleEntry {
            start_bar: start,
            end_bar: end,
            variant: variant.map(|s| s.to_string()),
        }
    }

    #[test]
    fn no_entries_yields_single_default_segment() {
        let segments = build_segments(&[], 4, "main");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start_bar, 0);
        assert_eq!(segments[0].end_bar, 4);
        assert_eq!(segments[0].style, ScheduleSegmentStyle::DefaultFill);
        assert_eq!(segments[0].label, "main");
    }

    #[test]
    fn chorus_drums_fill_at_bar_7_gaps_default() {
        // Real fixture: chorus drums has a single (7..8, Some("fill"))
        // override. Expect default 0..7 then fill at 7..8.
        let segments = build_segments(&[entry(7, 8, Some("fill"))], 8, "main");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start_bar, 0);
        assert_eq!(segments[0].end_bar, 7);
        assert_eq!(segments[0].style, ScheduleSegmentStyle::DefaultFill);
        assert_eq!(segments[1].start_bar, 7);
        assert_eq!(segments[1].end_bar, 8);
        assert_eq!(segments[1].style, ScheduleSegmentStyle::NonDefault);
        assert_eq!(segments[1].label, "fill");
    }

    #[test]
    fn stripped_lead_silent_at_bar_3_gaps_default() {
        // Real fixture: stripped lead schedule has a single
        // (3..4, None) silent sub-range. Expect default 0..3 then
        // silent at 3..4.
        let segments = build_segments(&[entry(3, 4, None)], 4, "main");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].style, ScheduleSegmentStyle::DefaultFill);
        assert_eq!(segments[1].style, ScheduleSegmentStyle::Silent);
        assert_eq!(segments[1].start_bar, 3);
        assert_eq!(segments[1].end_bar, 4);
    }

    #[test]
    fn entry_naming_the_default_variant_renders_as_default() {
        // Sanity guard: if an entry's variant happens to equal the
        // pattern's default, treat it as a default segment, not a
        // named override. Keeps the legend honest.
        let segments = build_segments(&[entry(2, 3, Some("main"))], 4, "main");
        // 0..2 default, 2..3 also default (no NonDefault), 3..4 default.
        // The builder doesn't merge adjacent defaults today — three
        // segments is the expected shape; the legend filters them out.
        assert_eq!(segments.len(), 3);
        for s in segments.iter() {
            assert_eq!(s.style, ScheduleSegmentStyle::DefaultFill);
        }
    }

    #[test]
    fn entries_clamped_to_total_bars() {
        // An entry running past total_bars should clip, not panic.
        let segments = build_segments(&[entry(2, 10, Some("fill"))], 4, "main");
        assert_eq!(segments.last().unwrap().end_bar, 4);
    }

    #[test]
    fn legend_filters_default_fills() {
        let segments = build_segments(&[entry(7, 8, Some("fill"))], 8, "main");
        let legend: Vec<LegendEntry> = segments
            .iter()
            .filter_map(LegendEntry::from_segment)
            .collect();
        assert_eq!(legend.len(), 1);
        assert_eq!(legend[0].kind, LegendKind::Named);
        assert_eq!(legend[0].variant_label, "fill");
        assert_eq!(legend[0].start_bar, 7);
    }

    #[test]
    fn legend_silent_carries_silent_label() {
        let segments = build_segments(&[entry(3, 4, None)], 4, "main");
        let legend: Vec<LegendEntry> = segments
            .iter()
            .filter_map(LegendEntry::from_segment)
            .collect();
        assert_eq!(legend.len(), 1);
        assert_eq!(legend[0].kind, LegendKind::Silent);
        assert_eq!(legend[0].variant_label, "silent");
    }
}
