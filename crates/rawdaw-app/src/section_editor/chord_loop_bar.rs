//! Section meta bar's chord-loop strip.
//!
//! Mirrors round-2's `ChordLoopBar`
//! (`mockups/round-2/components/section-editor.jsx` ~line 270). Tiles
//! `section.chord_loops[0]` across the section's bar count — one cell
//! per bar — showing the chord's Roman+quality label and absolute name.
//! The first cell of each loop iteration carries a heavier left stripe
//! in the loop's identity color so the loop boundary reads at a glance.
//!
//! ## Case preservation (UI principle 3)
//!
//! Roman numerals encode quality through case (`I` major, `vi` minor).
//! Don't lowercase, uppercase, or small-caps them. `chord_display::
//! roman_label` returns the canonical case (`I`, `V`, `vi`, `IV`); the
//! cell renders the string verbatim. The font sets
//! `font-feature-settings: "tnum"` for tabular alignment with the
//! absolute label below.
//!
//! ## Multi-loop schedule (round-3 deferred)
//!
//! A section can in principle host a multi-loop schedule
//! (`Vec<(BarRange, ChordLoopRef)>`), so e.g. bars 1–4 use one loop and
//! 5–8 use another. Neither the JS mockup nor this port renders that
//! yet — both render `chord_loops[0]` for the whole section. The
//! round-3 design pass owns the multi-loop UI; see
//! `docs/design/mockups/round-2/README.md` and Phase 3 of
//! `docs/round-2-port-plan.md`.

use rinch::prelude::*;

use rawdaw_model::chord::ChordSpec;

use crate::chord_display::{absolute_label, pitch_class_name, quality_suffix, roman_label};
use crate::parts::rgba;
use crate::state::AppState;
use crate::theme;

#[component]
pub fn ChordLoopBar(loop_name: String, duration_bars: u32) -> NodeHandle {
    let outer_style = format!(
        "display: flex; gap: 2px; height: 38px; box-sizing: border-box; \
         border-radius: 4px; overflow: hidden; \
         background: {bg0}; border: 1px solid {line}; padding: 2px;",
        bg0 = theme::BG0,
        line = theme::LINE,
    );

    // The `for` source inside rsx must be a `Fn() -> Vec<T>` callable —
    // we can't pre-compute a Vec and consume it once, since the
    // generated effect re-invokes the source. Following the round-1
    // arrangement pattern (`for cell in build_ribbon_cells()`), the
    // iteration source calls a free helper that re-runs the lookup on
    // each tick.
    rsx! {
        div { style: {outer_style.clone()},
            for cell in build_cells_by_name(loop_name.clone(), duration_bars) {
                ChordCell {
                    key: cell.bar,
                    roman: cell.roman,
                    quality: cell.quality,
                    absolute: cell.absolute,
                    color: cell.color,
                    is_first_of_loop: cell.is_first_of_loop,
                }
            }
        }
    }
}

fn build_cells_by_name(loop_name: String, duration_bars: u32) -> Vec<ChordCellData> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let overlay = app.overlay.get();
    let Some(loop_data) = project.chord_loops.values().find(|cl| cl.name == loop_name) else {
        return Vec::new();
    };
    let color = overlay
        .chord_loop_color
        .get(&loop_data.id)
        .cloned()
        .unwrap_or_else(|| theme::TEXT2.to_string());
    // The chord loop floats with the section's scale by default; round-1
    // chord loops carry no `key` override, so use the project default.
    // Section scale overrides aren't yet plumbed through this helper —
    // when they are, propagate the (section_id, scale) through alongside
    // the loop name.
    let scale = loop_data
        .key
        .clone()
        .unwrap_or_else(|| project.default_key.clone());

    // Pre-resolve each event's labels through chord_display so build_cells
    // only deals with already-formatted strings.
    let resolved: Vec<(String, String)> = loop_data
        .events
        .iter()
        .map(|ev| match &ev.chord {
            ChordSpec::Functional {
                roman,
                suffix,
                in_key,
            } => {
                let s = in_key.as_ref().unwrap_or(&scale);
                (
                    roman_label(*roman, &suffix.quality),
                    absolute_label(*roman, &suffix.quality, s),
                )
            }
            ChordSpec::Absolute { root, suffix } => (
                String::new(),
                format!(
                    "{}{}",
                    pitch_class_name(*root),
                    quality_suffix(&suffix.quality),
                ),
            ),
        })
        .collect();

    build_cells(&resolved, color.as_str(), duration_bars)
}

#[derive(Clone, PartialEq, Eq)]
struct ChordCellData {
    bar: u32,
    roman: String,
    quality: String,
    absolute: String,
    color: String,
    is_first_of_loop: bool,
}

/// Pure tiling: takes pre-resolved `(roman_label, absolute_label)`
/// pairs for one chord-loop iteration, plus the loop's color and the
/// target bar count, and produces one `ChordCellData` per bar by
/// repeating the loop. The `(roman, absolute)` strings are already
/// quality-coded — see `chord_display::roman_label`.
///
/// Tests construct the resolved pairs directly to exercise the
/// tiling logic without depending on the chord_display or model
/// machinery.
fn build_cells(events: &[(String, String)], color: &str, duration_bars: u32) -> Vec<ChordCellData> {
    if events.is_empty() || duration_bars == 0 {
        return Vec::new();
    }
    let mut cells = Vec::with_capacity(duration_bars as usize);
    let mut bar = 0u32;
    'outer: loop {
        for (ei, (roman, absolute)) in events.iter().enumerate() {
            if bar >= duration_bars {
                break 'outer;
            }
            cells.push(ChordCellData {
                bar,
                roman: roman.clone(),
                quality: String::new(),
                absolute: absolute.clone(),
                color: color.to_string(),
                is_first_of_loop: ei == 0,
            });
            bar += 1;
        }
    }
    cells
}

#[component]
fn ChordCell(
    roman: String,
    quality: String,
    absolute: String,
    color: String,
    is_first_of_loop: bool,
) -> NodeHandle {
    let bg_fill = rgba(color.as_str(), 0.10);
    let border_soft = rgba(color.as_str(), 0.30);
    // The whole cell carries a soft outline in the loop's color; the
    // first cell of each loop iteration replaces the left edge with a
    // 2px solid stripe at full saturation. Drawing the stripe via
    // `border-left` (rather than a positioned overlay) keeps the cell
    // content centered without measurement.
    let left_border = if is_first_of_loop {
        format!("2px solid {col}", col = color)
    } else {
        format!("1px solid {bc}", bc = border_soft)
    };
    let cell_style = format!(
        "flex: 1; min-width: 0; \
         background: {bg}; \
         border: 1px solid {bc}; border-left: {lb}; \
         border-radius: 2px; box-sizing: border-box; \
         display: flex; flex-direction: column; \
         align-items: center; justify-content: center; \
         gap: 1px; padding: 2px 4px;",
        bg = bg_fill,
        bc = border_soft,
        lb = left_border,
    );

    let roman_style = format!(
        "font-family: inherit; font-feature-settings: \"tnum\" 1; \
         font-weight: 600; font-size: 12px; letter-spacing: 0.5px; \
         color: {text}; line-height: 1;",
        text = theme::TEXT0,
    );
    let abs_style = format!(
        "font-size: 9px; color: {text2}; \
         font-feature-settings: \"tnum\" 1; line-height: 1;",
        text2 = theme::TEXT2,
    );

    // Mockup concatenates roman + quality into a single label. quality
    // is the empty string for round-1/2 chord-loop events (the case
    // carries the quality), but mirror the concat so a future
    // `m7`/`maj7` quality string renders next to the numeral.
    let label = format!("{roman}{quality}");

    rsx! {
        div { style: {cell_style.clone()},
            span { style: {roman_style.clone()}, {label.clone()} }
            span { style: {abs_style.clone()}, {absolute.clone()} }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn four_events() -> Vec<(String, String)> {
        vec![
            ("I".into(), "C".into()),
            ("V".into(), "G".into()),
            ("vi".into(), "Am".into()),
            ("IV".into(), "F".into()),
        ]
    }

    #[test]
    fn one_iteration_yields_one_first_of_loop_stripe() {
        let cells = build_cells(&four_events(), "#000000", 4);
        assert_eq!(cells.len(), 4);
        let firsts = cells.iter().filter(|c| c.is_first_of_loop).count();
        assert_eq!(firsts, 1);
        assert_eq!(cells[0].roman, "I");
        assert_eq!(cells[3].roman, "IV");
    }

    #[test]
    fn two_iterations_yield_two_first_of_loop_stripes() {
        let cells = build_cells(&four_events(), "#000000", 8);
        assert_eq!(cells.len(), 8);
        let firsts: Vec<u32> = cells
            .iter()
            .filter(|c| c.is_first_of_loop)
            .map(|c| c.bar)
            .collect();
        assert_eq!(firsts, vec![0, 4]);
    }

    #[test]
    fn partial_iteration_truncates_to_duration() {
        let cells = build_cells(&four_events(), "#000000", 6);
        assert_eq!(cells.len(), 6);
        // Second iteration was cut after two events.
        let firsts: Vec<u32> = cells
            .iter()
            .filter(|c| c.is_first_of_loop)
            .map(|c| c.bar)
            .collect();
        assert_eq!(firsts, vec![0, 4]);
        assert_eq!(cells[5].roman, "V");
    }

    #[test]
    fn empty_loop_or_zero_duration_yields_no_cells() {
        assert!(build_cells(&Vec::new(), "#000000", 4).is_empty());
        assert!(build_cells(&four_events(), "#000000", 0).is_empty());
    }

    #[test]
    fn roman_case_is_preserved() {
        // Principle 3: case carries chord quality. Don't lowercase
        // major or uppercase minor.
        let cells = build_cells(&four_events(), "#000000", 4);
        let romans: Vec<&str> = cells.iter().map(|c| c.roman.as_str()).collect();
        assert_eq!(romans, vec!["I", "V", "vi", "IV"]);
    }
}
