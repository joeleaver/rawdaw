//! Arrangement — the centerpiece. Ruler · chord-loop ribbon · section
//! lane.
//!
//! Translates `docs/design/mockups/round-1/components/arrangement.jsx`.
//!
//! Round 1 chooses **percent-based horizontal positioning** rather than
//! the mockup's `ResizeObserver`-driven pixel widths. With a fixed
//! `total_bars`, `left: (bar / total_bars * 100)%` and
//! `width: (bars / total_bars * 100)%` lay everything out without
//! reading the layout-engine's computed width. The Rinch port to a
//! real timeline ruler (with zoom controls + horizontal scroll) will
//! reintroduce pixel math, but that's a Rinch gap we're leaving for
//! later.
//!
//! Per the round-1 README port-time note, what was many `<line>` ticks
//! in the JSX is one `<path>` with `M`/`L` commands here. The Vello
//! paint pass only tessellates once.

use rinch::prelude::*;

use crate::fixture::{self, ChordEvent};
use crate::parts::rgba;
use crate::state::AppState;
use crate::theme;

/// Look up the section key of the currently-selected SectionRef. Reads
/// `AppState::selected_idx` — when called inside an rsx style expression
/// the rinch effect tracker wires the read into the surrounding closure
/// and re-evaluates the style on selection changes.
fn current_selected_section_key() -> String {
    let app = use_store::<AppState>();
    let Some(idx) = app.selected_idx.get() else {
        return String::new();
    };
    let r = fixture::round1();
    r.arrangement
        .get(idx)
        .map(|b| b.section_key.to_string())
        .unwrap_or_default()
}

#[component]
pub fn Arrangement() -> NodeHandle {
    let r = fixture::round1();
    let section_style = format!(
        "flex: 1; min-width: 0; display: flex; flex-direction: column; \
         background: {bg};",
        bg = theme::BG0,
    );
    let playhead_pct = playhead_percent();

    rsx! {
        section { style: {section_style.clone()},
            Ruler { total_bars: r.total_bars, playhead_pct: playhead_pct }
            ChordRibbon { total_bars: r.total_bars }
            SectionLane {
                total_bars: r.total_bars,
                playhead_pct: playhead_pct,
            }
            LaneFiller { total_bars: r.total_bars, playhead_pct: playhead_pct }
        }
    }
}

/// Fixture playhead position as a percent of total_bars. Bar 5 ·
/// Beat 2 → bar_index 4 + 0.25 = 4.25 → 4.25 / 24 ≈ 17.71%.
fn playhead_percent() -> f32 {
    let r = fixture::round1();
    let bars = (r.project.playhead_bar as f32 - 1.0) + (r.project.playhead_beat as f32 - 1.0) / 4.0;
    bars / r.total_bars as f32 * 100.0
}

// ─── Timeline ruler ───────────────────────────────────────────────────────

#[component]
fn Ruler(total_bars: u32, playhead_pct: f32) -> NodeHandle {
    let height = theme::H_RULER;
    let style = format!(
        "height: {h}px; flex: 0 0 {h}px; position: relative; \
         background: {bg}; border-bottom: 1px solid {line};",
        h = height, bg = theme::BG1, line = theme::LINE,
    );

    // Single SVG path with all major + minor tick lines (port-time note).
    // Coordinate system: x in [0, total_bars*4] (so 4 minor ticks per bar);
    // y in [0, height]. The SVG itself spans 100% width via preserveAspectRatio.
    let mut tick_d = String::new();
    let minor_y = (height as f32 - 2.0).to_string();
    let major_y = (height as f32 - 8.0).to_string();
    let bottom_y = height.to_string();
    let beats = total_bars * 4;
    for i in 0..=beats {
        let x = i;
        let is_bar_boundary = i % 4 == 0;
        let y_top = if is_bar_boundary { major_y.as_str() } else { minor_y.as_str() };
        // For minor ticks at non-bar-boundaries draw a shorter line; here we
        // simplify by drawing every tick to the same major depth visually,
        // which the screenshot test will validate. (Minor ticks at the
        // mockup's 0.5 alpha + shorter length is round-2 polish.)
        tick_d.push_str(&format!("M {x} {y_top} L {x} {bottom_y} "));
    }

    // Bar number labels every 4 bars.
    rsx! {
        div { style: {style.clone()},
            svg {
                viewBox: format!("0 0 {beats} {height}"),
                preserveAspectRatio: "none",
                fill: "none",
                stroke: theme::TEXT2,
                stroke-width: "1",
                style: "width: 100%; height: 100%; display: block;",
                path { d: {tick_d.clone()} }
            }
            // Bar number labels rendered as positioned spans (one per 4 bars).
            for bar in (0..total_bars).filter(|b| b % 4 == 0).collect::<Vec<u32>>() {
                BarLabel { bar: bar, total_bars: total_bars }
            }
            // Playhead head — small caret + 1px vertical line.
            div {
                style: format!(
                    "position: absolute; left: {p}%; top: 0; bottom: 0; \
                     width: 1px; background: {acc}; transform: translateX(-0.5px);",
                    p = playhead_pct, acc = theme::ACCENT,
                ),
            }
        }
    }
}

#[component]
fn BarLabel(bar: u32, total_bars: u32) -> NodeHandle {
    let left_pct = bar as f32 / total_bars as f32 * 100.0;
    let label = (bar + 1).to_string();
    let style = format!(
        "position: absolute; left: calc({left_pct}% + 4px); top: 2px; \
         font-size: 10px; color: {fg}; \
         font-feature-settings: \"tnum\" 1; pointer-events: none;",
        fg = theme::TEXT2,
    );
    rsx! {
        span { style: {style.clone()}, {label.clone()} }
    }
}

// ─── Chord-loop ribbon ────────────────────────────────────────────────────

#[component]
fn ChordRibbon(total_bars: u32) -> NodeHandle {
    let style = format!(
        "height: {h}px; flex: 0 0 {h}px; position: relative; \
         background: {bg}; border-bottom: 1px solid {line};",
        h = theme::H_RIBBON, bg = theme::BG1, line = theme::LINE,
    );
    rsx! {
        div { style: {style.clone()},
            for cell in build_ribbon_cells() {
                RibbonCell {
                    key: format!("{}-{}", cell.section_key, cell.bar),
                    bar: cell.bar,
                    total_bars: total_bars,
                    roman: cell.roman.to_string(),
                    absolute: cell.absolute.to_string(),
                    color: cell.color.to_string(),
                    is_first_of_loop: cell.is_first_of_loop,
                    section_key: cell.section_key.to_string(),
                }
            }
        }
    }
}

/// One ribbon cell per chord per loop-iteration per arrangement block.
#[derive(Clone, PartialEq, Eq)]
struct RibbonCellData {
    bar: u32,
    roman: &'static str,
    absolute: &'static str,
    color: &'static str,
    section_key: &'static str,
    is_first_of_loop: bool,
}

fn build_ribbon_cells() -> Vec<RibbonCellData> {
    let r = fixture::round1();
    let mut cells = Vec::new();
    for block in r.arrangement.iter() {
        let Some(section) = fixture::section_by_key(&r, block.section_key) else { continue; };
        let Some(loop_name) = section.chord_loops.first().copied() else { continue; };
        let Some(loop_data) = fixture::chord_loop_by_name(&r, loop_name) else { continue; };

        // Tile the loop across the block, 1 chord per bar (round-1 fixture).
        let mut bar = block.start_bar;
        let block_end = block.start_bar + block.bars;
        'outer: loop {
            for (ei, ev) in loop_data.events.iter().enumerate() {
                if bar >= block_end {
                    break 'outer;
                }
                cells.push(RibbonCellData {
                    bar,
                    roman: ev.roman,
                    absolute: ev.absolute,
                    color: loop_data.color,
                    section_key: block.section_key,
                    is_first_of_loop: ei == 0,
                });
                bar += 1;
            }
            if bar >= block_end {
                break;
            }
        }
        // Suppress unused-var warning when no looping happens.
        let _ = ChordEvent { roman: "", quality: "", absolute: "" };
    }
    cells
}

#[component]
fn RibbonCell(
    bar: u32,
    total_bars: u32,
    roman: String,
    absolute: String,
    color: String,
    is_first_of_loop: bool,
    section_key: String,
) -> NodeHandle {
    let left_pct = bar as f32 / total_bars as f32 * 100.0;
    let width_pct = 1.0 / total_bars as f32 * 100.0;
    let bar_style_base = format!(
        "position: absolute; left: {l}%; top: 0; width: {w}%; height: 100%; \
         border-right: 1px solid {line_soft}; \
         display: flex; flex-direction: column; \
         justify-content: center; align-items: center; \
         padding-top: 3px; gap: 1px;",
        l = left_pct, w = width_pct, line_soft = theme::LINE_SOFT,
    );

    // Each style: expression below becomes a separate `Fn` effect
    // closure that moves its captures (same shape as variant_tabs.rs).
    // Strings used by more than one closure need a per-closure clone.
    // Every helper call reads `AppState::selected_idx`, so the macro's
    // effect tracker re-runs each style on selection changes — the
    // cell itself is never re-mounted.
    let color_for_bg = color.clone();
    let color_for_stripe = color.clone();
    let section_key_for_bg = section_key.clone();
    let section_key_for_stripe = section_key.clone();
    let section_key_for_roman = section_key.clone();
    let section_key_for_abs = section_key;

    rsx! {
        div {
            style: {
                let emphasized = current_selected_section_key() == section_key_for_bg;
                let bg = if emphasized {
                    rgba(color_for_bg.as_str(), 0.10)
                } else {
                    "transparent".to_string()
                };
                format!("{} background: {};", bar_style_base, bg)
            },
            span {
                style: {
                    if !is_first_of_loop {
                        "display: none;".to_string()
                    } else {
                        let emphasized = current_selected_section_key() == section_key_for_stripe;
                        let opa = if emphasized { 1.0 } else { 0.7 };
                        format!(
                            "position: absolute; left: 0; top: 0; width: 2px; \
                             height: 100%; background: {}; opacity: {};",
                            color_for_stripe, opa,
                        )
                    }
                },
            }
            span {
                style: {
                    let emphasized = current_selected_section_key() == section_key_for_roman;
                    let roman_color = if emphasized {
                        "rgba(232,234,238,0.96)"
                    } else {
                        "rgba(232,234,238,0.82)"
                    };
                    format!(
                        "font-family: inherit; font-feature-settings: \"tnum\" 1; \
                         font-weight: 600; font-size: 13.5px; letter-spacing: 0.6px; \
                         color: {}; line-height: 1;",
                        roman_color,
                    )
                },
                {roman.clone()}
            }
            div {
                style: {
                    let emphasized = current_selected_section_key() == section_key_for_abs;
                    let abs_color = if emphasized { theme::TEXT1 } else { theme::TEXT2 };
                    format!(
                        "font-size: 9.5px; color: {}; \
                         font-feature-settings: \"tnum\" 1; letter-spacing: 0.2px; line-height: 1;",
                        abs_color,
                    )
                },
                {absolute.clone()}
            }
        }
    }
}

// ─── Section lane ─────────────────────────────────────────────────────────

#[component]
fn SectionLane(
    total_bars: u32,
    playhead_pct: f32,
) -> NodeHandle {
    let style = format!(
        "height: {h}px; flex: 0 0 {h}px; position: relative; \
         background: {bg}; border-bottom: 1px solid {line};",
        h = theme::H_LANE, bg = theme::BG0, line = theme::LINE,
    );

    // Per-bar guide path (single SVG path, like the ruler).
    let mut guide_d = String::new();
    for i in 0..=total_bars {
        // x in [0, total_bars]; y full-height.
        guide_d.push_str(&format!("M {x} 0 L {x} 100 ", x = i));
    }
    let total_bars_str = total_bars.to_string();

    rsx! {
        div { style: {style.clone()},
            svg {
                viewBox: format!("0 0 {total_bars_str} 100"),
                preserveAspectRatio: "none",
                fill: "none",
                stroke: theme::LINE_SOFT,
                stroke-width: "0.5",
                style: "width: 100%; height: 100%; display: block; \
                        position: absolute; inset: 0; pointer-events: none;",
                path { d: {guide_d.clone()} }
            }
            // Section blocks. The rsx for source must be `Fn() -> Vec<T>`
            // callable; chaining off the fixture's `'static` slice
            // produces a fresh iterator each call. SectionBlock reads
            // selection internally so we don't need to thread is_selected
            // / is_linked props through here.
            for block in fixture::round1().arrangement.iter().cloned() {
                SectionBlock {
                    key: block.idx,
                    idx: block.idx,
                    section_key: block.section_key.to_string(),
                    variant: block.variant.to_string(),
                    start_bar: block.start_bar,
                    bars: block.bars,
                    total_bars: total_bars,
                }
            }
            // Playhead vertical line.
            div {
                style: format!(
                    "position: absolute; left: {p}%; top: 0; bottom: 0; \
                     width: 1px; background: {acc}; opacity: 0.85; \
                     transform: translateX(-0.5px); pointer-events: none;",
                    p = playhead_pct, acc = theme::ACCENT,
                ),
            }
        }
    }
}

#[component]
fn SectionBlock(
    idx: usize,
    section_key: String,
    variant: String,
    start_bar: u32,
    bars: u32,
    total_bars: u32,
) -> NodeHandle {
    let app = use_store::<AppState>();
    let r = fixture::round1();
    let Some(section) = fixture::section_by_key(&r, section_key.as_str()) else {
        return rsx! { span {} };
    };
    let color = section.color.to_string();
    let name = section.name.to_string();
    let default_variant = section.default_variant.to_string();

    let left_pct = start_bar as f32 / total_bars as f32 * 100.0;
    let width_pct = bars as f32 / total_bars as f32 * 100.0;

    // Layout positions / sizes are static; selection-dependent colors,
    // borders and box-shadow re-evaluate inside the rsx style: closure
    // each time `AppState::selected_idx` changes. The static fragment is
    // built once here and concatenated with the reactive fragment
    // inline.
    let block_static = format!(
        "position: absolute; left: {l}%; top: 6px; \
         width: {w}%; bottom: 6px; \
         border-left: 3px solid {col}; border-radius: 3px; \
         cursor: pointer; overflow: hidden;",
        l = left_pct, w = width_pct, col = color,
    );

    // Internal bar guide path within the block (single SVG path).
    let internal_bars = bars.saturating_sub(1);
    let mut inner_d = String::new();
    for i in 1..=internal_bars {
        inner_d.push_str(&format!("M {x} 6 L {x} 94 ", x = i));
    }
    let inner_stroke = rgba(color.as_str(), 0.22);
    let bars_str = bars.to_string();

    let show_variant = variant != default_variant;
    let chip_label = format!("variant: {}", variant);
    let chip_bg = rgba(color.as_str(), 0.32);
    let chip_border = rgba(color.as_str(), 0.55);
    // Always render the chip; collapse via `display: none` when the block
    // uses the section's default variant. This sidesteps the Fn-capture
    // trap that bites when `if show_variant { Component { ... } }` tries
    // to capture String props by move twice (outer if-Fn + inner
    // reactive_component_dom move).
    let chip_style = if show_variant {
        format!(
            "position: absolute; right: 6px; top: 5px; \
             display: inline-flex; align-items: center; \
             height: 16px; padding: 0 5px; border-radius: 2px; \
             font-size: 10px; font-weight: 500; letter-spacing: 0.2px; \
             background: {chip_bg}; color: rgba(232,234,238,0.92); \
             border: 1px solid {chip_border};",
        )
    } else {
        "display: none;".to_string()
    };
    let length_text = format!("{} {}", bars, if bars == 1 { "bar" } else { "bars" });

    let color_for_style = color.clone();
    let section_key_for_style = section_key.clone();
    let _ = section_key; // remaining captures are inside this style closure

    rsx! {
        div {
            style: {
                let sel = app.selected_idx.get();
                let is_selected = sel == Some(idx);
                let is_linked = !is_selected
                    && sel
                        .and_then(|i| fixture::round1().arrangement.get(i))
                        .map(|b| b.section_key == section_key_for_style)
                        .unwrap_or(false);
                let bg = rgba(
                    color_for_style.as_str(),
                    if is_selected {
                        0.20
                    } else if is_linked {
                        0.14
                    } else {
                        0.10
                    },
                );
                let border = if is_selected {
                    format!("1px solid {}", color_for_style)
                } else if is_linked {
                    format!("1px solid {}", rgba(color_for_style.as_str(), 0.55))
                } else {
                    format!("1px solid {}", rgba(color_for_style.as_str(), 0.28))
                };
                let box_shadow = if is_selected {
                    format!("0 0 0 1px {}", rgba(color_for_style.as_str(), 0.30))
                } else {
                    "none".to_string()
                };
                format!(
                    "{block_static} background: {bg}; border: {border}; \
                     box-shadow: {box_shadow};"
                )
            },
            onclick: move || app.set_selected_idx(Some(idx)),
            svg {
                viewBox: format!("0 0 {bars_str} 100"),
                preserveAspectRatio: "none",
                fill: "none",
                stroke: {inner_stroke.clone()},
                stroke-width: "0.5",
                style: "width: 100%; height: 100%; display: block; \
                        position: absolute; inset: 0; pointer-events: none;",
                path { d: {inner_d.clone()} }
            }
            // Name (top-left).
            div {
                style: "position: absolute; left: 8px; top: 6px; \
                        font-size: 12.5px; font-weight: 600; \
                        color: rgba(232,234,238,0.96); letter-spacing: -0.1px;",
                {name.clone()}
            }
            VariantBlockChip {
                label: chip_label,
                style: chip_style,
            }
            // Length readout (bottom-right).
            div {
                style: "position: absolute; right: 6px; bottom: 4px; \
                        font-size: 10px; color: rgba(232,234,238,0.62); \
                        font-feature-settings: \"tnum\" 1; \
                        font-variant-numeric: tabular-nums; letter-spacing: 0.2px;",
                {length_text.clone()}
            }
        }
    }
}

/// Small component for the variant chip inside a SectionBlock. Extracted
/// because rendering a chip conditionally via `if show_variant { ... }`
/// inside the block's rsx triggers the "captured String moves into a
/// Fn closure" pattern. As a separate `#[component]` the chip's own
/// captures stay isolated.
#[component]
fn VariantBlockChip(label: String, style: String) -> NodeHandle {
    rsx! {
        div { style: {style.clone()},
            span { style: "opacity: 0.7; margin-right: 4px;", "variant:" }
            {label.replace("variant: ", "").clone()}
        }
    }
}

// ─── Lane filler (faint future-multi-lane area) ───────────────────────────

#[component]
fn LaneFiller(total_bars: u32, playhead_pct: f32) -> NodeHandle {
    let style = format!(
        "flex: 1; min-height: 0; position: relative; \
         background: {bg}; border-top: 1px solid {line};",
        bg = theme::BG0, line = theme::LINE,
    );

    let mut guide_d = String::new();
    for i in 0..=total_bars {
        guide_d.push_str(&format!("M {x} 0 L {x} 100 ", x = i));
    }

    rsx! {
        div { style: {style.clone()},
            svg {
                viewBox: format!("0 0 {total_bars} 100"),
                preserveAspectRatio: "none",
                fill: "none",
                stroke: theme::LINE_SOFT,
                stroke-width: "0.5",
                style: "width: 100%; height: 100%; display: block; \
                        position: absolute; inset: 0; pointer-events: none;",
                path { d: {guide_d.clone()} }
            }
            div {
                style: format!(
                    "position: absolute; left: {p}%; top: 0; bottom: 0; \
                     width: 1px; background: {acc}; opacity: 0.7; \
                     transform: translateX(-0.5px); pointer-events: none;",
                    p = playhead_pct, acc = theme::ACCENT,
                ),
            }
            div {
                style: "position: absolute; left: 14px; bottom: 10px; \
                        font-size: 10.5px; color: rgba(232,234,238,0.28); \
                        letter-spacing: 0.4px; font-style: italic;",
                "additional lanes (sub-tracks) — round 2"
            }
        }
    }
}
