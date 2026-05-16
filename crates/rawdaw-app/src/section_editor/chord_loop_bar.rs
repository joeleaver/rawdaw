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
//! Don't lowercase, uppercase, or small-caps them. The fixture stores
//! the canonical case (`I`, `V`, `vi`, `IV`); the cell renders the
//! string verbatim. The font sets `font-feature-settings: "tnum"` for
//! tabular alignment with the absolute label below.
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

use crate::fixture;
use crate::parts::rgba;
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
    // each tick. The lookups are cheap (linear scan of CHORD_LOOPS) and
    // run on mount + on any signal read inside the for body (currently
    // none).
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
    let r = fixture::round1();
    match fixture::chord_loop_by_name(&r, loop_name.as_str()) {
        Some(loop_data) => build_cells(loop_data, duration_bars),
        None => Vec::new(),
    }
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

fn build_cells(loop_data: &fixture::ChordLoop, duration_bars: u32) -> Vec<ChordCellData> {
    if loop_data.events.is_empty() || duration_bars == 0 {
        return Vec::new();
    }
    let mut cells = Vec::with_capacity(duration_bars as usize);
    let mut bar = 0u32;
    'outer: loop {
        for (ei, ev) in loop_data.events.iter().enumerate() {
            if bar >= duration_bars {
                break 'outer;
            }
            cells.push(ChordCellData {
                bar,
                roman: ev.roman.to_string(),
                quality: ev.quality.to_string(),
                absolute: ev.absolute.to_string(),
                color: loop_data.color.to_string(),
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
    // is the empty string for round-1/2 fixtures (the case carries the
    // quality), but mirror the concat so a future `m7`/`maj7` quality
    // string renders next to the numeral.
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

    fn fake_loop(events: &'static [fixture::ChordEvent], color: &'static str) -> fixture::ChordLoop {
        fixture::ChordLoop {
            id: "test",
            name: "test",
            color,
            length_bars: events.len() as u32,
            events,
        }
    }

    static FOUR_EVENTS: &[fixture::ChordEvent] = &[
        fixture::ChordEvent { roman: "I",  quality: "", absolute: "C"  },
        fixture::ChordEvent { roman: "V",  quality: "", absolute: "G"  },
        fixture::ChordEvent { roman: "vi", quality: "", absolute: "Am" },
        fixture::ChordEvent { roman: "IV", quality: "", absolute: "F"  },
    ];

    #[test]
    fn one_iteration_yields_one_first_of_loop_stripe() {
        let cl = fake_loop(FOUR_EVENTS, "#000000");
        let cells = build_cells(&cl, 4);
        assert_eq!(cells.len(), 4);
        let firsts = cells.iter().filter(|c| c.is_first_of_loop).count();
        assert_eq!(firsts, 1);
        assert_eq!(cells[0].roman, "I");
        assert_eq!(cells[3].roman, "IV");
    }

    #[test]
    fn two_iterations_yield_two_first_of_loop_stripes() {
        let cl = fake_loop(FOUR_EVENTS, "#000000");
        let cells = build_cells(&cl, 8);
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
        let cl = fake_loop(FOUR_EVENTS, "#000000");
        let cells = build_cells(&cl, 6);
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
        let empty = fake_loop(&[], "#000000");
        assert!(build_cells(&empty, 4).is_empty());

        let cl = fake_loop(FOUR_EVENTS, "#000000");
        assert!(build_cells(&cl, 0).is_empty());
    }

    #[test]
    fn roman_case_is_preserved() {
        // Principle 3: case carries chord quality. Don't lowercase
        // major or uppercase minor.
        let cl = fake_loop(FOUR_EVENTS, "#000000");
        let cells = build_cells(&cl, 4);
        let romans: Vec<&str> = cells.iter().map(|c| c.roman.as_str()).collect();
        assert_eq!(romans, vec!["I", "V", "vi", "IV"]);
    }
}
