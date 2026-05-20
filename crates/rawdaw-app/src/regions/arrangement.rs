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
//!
//! Migrated in C1c to read off [`AppState::project`] +
//! [`AppState::overlay`]; per-block / per-ribbon-cell data pre-built
//! into owned `Vec`s so rsx `for` closures stay `Fn`.

use rinch::prelude::*;

use rawdaw_model::chord::ChordSpec;
use rawdaw_model::id::SectionId;
use rawdaw_model::time::{MusicalTime, PPQ};

use crate::audio::AudioResources;
use crate::chord_display::{absolute_label, roman_label};
use crate::parts::rgba;
use crate::state::AppState;
use crate::theme;

/// Beats per bar baked into the round-1 fixture. Switches to the model
/// `tempo_map.beats_per_bar_at(...)` lookup when the arrangement starts
/// honoring mid-arrangement time-signature changes.
const BEATS_PER_BAR: u32 = 4;

/// Look up the [`SectionId`] of the currently-selected arrangement
/// block. Reads `AppState::selected_idx` — when called inside an rsx
/// style expression the rinch effect tracker wires the read into the
/// surrounding closure and re-evaluates the style on selection changes.
fn current_selected_section_id() -> Option<SectionId> {
    let app = use_store::<AppState>();
    let idx = app.selected_idx.get()?;
    let project = app.project.get();
    project.arrangement.sections.get(idx).map(|sr| sr.section)
}

/// One arrangement block, pre-resolved against model + overlay.
#[derive(Clone, Default, PartialEq)]
struct ArrangementBlockData {
    idx: usize,
    section_id: SectionId,
    section_name: String,
    color: String,
    variant: String,
    default_variant: String,
    start_bar: u32,
    bars: u32,
}

fn build_arrangement_blocks() -> Vec<ArrangementBlockData> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let overlay = app.overlay.get();
    project
        .arrangement
        .sections
        .iter()
        .enumerate()
        .filter_map(|(idx, sr)| {
            let section = project.sections.get(&sr.section)?;
            let bars = effective_duration_bars(section, sr.variant.as_str());
            let color = overlay
                .section_color
                .get(&sr.section)
                .cloned()
                .unwrap_or_else(|| theme::TEXT2.to_string());
            Some(ArrangementBlockData {
                idx,
                section_id: sr.section,
                section_name: section.name.clone(),
                color,
                variant: sr.variant.as_str().to_string(),
                default_variant: section.default_variant.as_str().to_string(),
                start_bar: musical_time_to_bars(sr.start),
                bars,
            })
        })
        .collect()
}

fn effective_duration_bars(section: &rawdaw_model::section::Section, variant: &str) -> u32 {
    use rawdaw_model::id::VariantId;
    let variant_id = VariantId::from(variant);
    section
        .variants
        .get(&variant_id)
        .and_then(|v| v.duration_bars)
        .unwrap_or(section.base.duration_bars)
}

fn musical_time_to_bars(t: MusicalTime) -> u32 {
    let ticks_per_bar = PPQ * BEATS_PER_BAR as i64;
    (t.as_ticks() / ticks_per_bar).max(0) as u32
}

fn arrangement_total_bars() -> u32 {
    let blocks = build_arrangement_blocks();
    blocks.iter().map(|b| b.start_bar + b.bars).max().unwrap_or(0)
}

#[component]
pub fn Arrangement() -> NodeHandle {
    let total_bars = arrangement_total_bars();
    let section_style = format!(
        "flex: 1; min-width: 0; display: flex; flex-direction: column; \
         background: {bg};",
        bg = theme::BG0,
    );

    rsx! {
        section { style: {section_style.clone()},
            Ruler { total_bars: total_bars }
            ChordRibbon { total_bars: total_bars }
            SectionLane { total_bars: total_bars }
            LaneFiller { total_bars: total_bars }
        }
    }
}

/// Engine-driven playhead position as a percent of `total_bars`.
///
/// Reads `AudioResources::playhead_samples` (a `Signal<u64>`) via the
/// shared `AudioResources::playhead_position` helper; the `.get()`
/// inside subscribes any rsx attribute closure that calls this
/// function. With the cpal stream paused (default until phase E6) the
/// signal stays at 0 and the playhead sits at bar 1.
fn playhead_percent(total_bars: u32) -> f32 {
    let audio = use_store::<AudioResources>();
    let total = total_bars.max(1) as f64;
    (audio.playhead_position().bars_f64 / total * 100.0) as f32
}

// ─── Timeline ruler ───────────────────────────────────────────────────────

#[component]
fn Ruler(total_bars: u32) -> NodeHandle {
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
            // Playhead head — 1px vertical line. The style closure reads
            // `playhead_percent()`, which calls `Signal::get` on the
            // engine-driven playhead; the rsx attribute is a Fn effect
            // closure, so it re-evaluates whenever the signal updates.
            div {
                style: format!(
                    "position: absolute; left: {p}%; top: 0; bottom: 0; \
                     width: 1px; background: {acc}; transform: translateX(-0.5px);",
                    p = playhead_percent(total_bars), acc = theme::ACCENT,
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
                    key: format!("{}-{}", cell.section_id.get(), cell.bar),
                    bar: cell.bar,
                    total_bars: total_bars,
                    roman: cell.roman.to_string(),
                    absolute: cell.absolute.to_string(),
                    color: cell.color.to_string(),
                    is_first_of_loop: cell.is_first_of_loop,
                    section_id: cell.section_id,
                }
            }
        }
    }
}

/// One ribbon cell per chord per loop-iteration per arrangement block.
#[derive(Clone, PartialEq, Eq)]
struct RibbonCellData {
    bar: u32,
    roman: String,
    absolute: String,
    color: String,
    section_id: SectionId,
    is_first_of_loop: bool,
}

fn build_ribbon_cells() -> Vec<RibbonCellData> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let overlay = app.overlay.get();
    let blocks = build_arrangement_blocks();

    let mut cells = Vec::new();
    for block in &blocks {
        let Some(section) = project.sections.get(&block.section_id) else {
            continue;
        };
        // Round-1 fixture uses the section's first chord loop as its
        // base-variant loop; if a variant overrides the chord_loops list
        // it would honor that too. For now, base only.
        let Some((_, loop_id)) = section.base.chord_loops.first() else {
            continue;
        };
        let Some(loop_data) = project.chord_loops.get(loop_id) else {
            continue;
        };
        let color = overlay
            .chord_loop_color
            .get(loop_id)
            .cloned()
            .unwrap_or_else(|| theme::TEXT2.to_string());
        let effective_scale = section
            .base
            .scale_override
            .clone()
            .unwrap_or_else(|| project.default_key.clone());

        // Tile the loop across the block, one chord per bar (round-1).
        let mut bar = block.start_bar;
        let block_end = block.start_bar + block.bars;
        'outer: loop {
            for (ei, ev) in loop_data.events.iter().enumerate() {
                if bar >= block_end {
                    break 'outer;
                }
                let (roman, absolute) = match &ev.chord {
                    ChordSpec::Functional {
                        roman,
                        suffix,
                        in_key,
                    } => {
                        let s = in_key.as_ref().unwrap_or(&effective_scale);
                        (
                            roman_label(*roman, &suffix.quality),
                            absolute_label(*roman, &suffix.quality, s),
                        )
                    }
                    ChordSpec::Absolute { root, suffix } => (
                        String::new(),
                        format!(
                            "{}{}",
                            crate::chord_display::pitch_class_name(*root),
                            crate::chord_display::quality_suffix(&suffix.quality),
                        ),
                    ),
                };
                cells.push(RibbonCellData {
                    bar,
                    roman,
                    absolute,
                    color: color.clone(),
                    section_id: block.section_id,
                    is_first_of_loop: ei == 0,
                });
                bar += 1;
            }
            if bar >= block_end {
                break;
            }
        }
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
    section_id: SectionId,
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
    // closure that captures the same `section_id` `Copy`. Selection
    // highlight: emphasize when the currently-selected arrangement
    // block references this section.
    let color_for_bg = color.clone();
    let color_for_stripe = color;

    rsx! {
        div {
            style: {
                let emphasized = current_selected_section_id() == Some(section_id);
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
                        let emphasized = current_selected_section_id() == Some(section_id);
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
                    let emphasized = current_selected_section_id() == Some(section_id);
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
                    let emphasized = current_selected_section_id() == Some(section_id);
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
fn SectionLane(total_bars: u32) -> NodeHandle {
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
            // Section blocks. `build_arrangement_blocks()` is called
            // inside the for source closure so the iteration stays a
            // `Fn` callable that builds a fresh `Vec` on each render.
            for block in build_arrangement_blocks() {
                SectionBlock {
                    key: block.idx,
                    block: block,
                    total_bars: total_bars,
                }
            }
            // Playhead vertical line. Reactive via `playhead_percent()`
            // — see Ruler's playhead for the closure-tracking rationale.
            div {
                style: format!(
                    "position: absolute; left: {p}%; top: 0; bottom: 0; \
                     width: 1px; background: {acc}; opacity: 0.85; \
                     transform: translateX(-0.5px); pointer-events: none;",
                    p = playhead_percent(total_bars), acc = theme::ACCENT,
                ),
            }
        }
    }
}

#[component]
fn SectionBlock(block: ArrangementBlockData, total_bars: u32) -> NodeHandle {
    let app = use_store::<AppState>();
    let ArrangementBlockData {
        idx,
        section_id,
        section_name,
        color,
        variant,
        default_variant,
        start_bar,
        bars,
    } = block;

    let left_pct = start_bar as f32 / total_bars as f32 * 100.0;
    let width_pct = bars as f32 / total_bars as f32 * 100.0;

    let block_static = format!(
        "position: absolute; left: {l}%; top: 6px; \
         width: {w}%; bottom: 6px; \
         border-left: 3px solid {col}; border-radius: 3px; \
         cursor: pointer; overflow: hidden;",
        l = left_pct, w = width_pct, col = color,
    );

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

    let color_for_style = color;

    rsx! {
        div {
            style: {
                let sel = app.selected_idx.get();
                let is_selected = sel == Some(idx);
                // is_linked: highlight every block that references the
                // same section as the selected block. Reads model
                // signals through `current_selected_section_id`.
                let is_linked = !is_selected
                    && current_selected_section_id() == Some(section_id);
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
                {section_name.clone()}
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
fn LaneFiller(total_bars: u32) -> NodeHandle {
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
            // Playhead vertical line. Reactive via `playhead_percent()`
            // — see Ruler's playhead for the closure-tracking rationale.
            div {
                style: format!(
                    "position: absolute; left: {p}%; top: 0; bottom: 0; \
                     width: 1px; background: {acc}; opacity: 0.7; \
                     transform: translateX(-0.5px); pointer-events: none;",
                    p = playhead_percent(total_bars), acc = theme::ACCENT,
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
