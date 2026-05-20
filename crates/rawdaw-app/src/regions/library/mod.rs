//! Library panel — Patterns, Chord Loops, Sections.
//!
//! Translates `docs/design/mockups/round-1/components/library.jsx`.
//! Always visible, drag-droppable in a later round. For round 1 the
//! groups render fully expanded; the toggle interaction lands when
//! per-group state becomes real (round 2).
//!
//! Per principle 4 (variants are tabs, not tree nodes), the Sections
//! group lists section *templates* only — variant chips and the
//! variant tab strip live in the inspector / section editor.
//!
//! Migrated in C1c to read off [`AppState::project`] +
//! [`AppState::overlay`]; pre-build the per-row data into owned `Vec`s
//! per group so the `for` source closures stay `Fn` and the model
//! borrow doesn't outlive the rsx invocation.
//!
//! ## File layout (post-CL1)
//!
//! - `mod.rs` (this file): Library shell + SearchBar + Patterns +
//!   Sections groups (both still read-only) + shared row / header /
//!   `+ new …` primitives.
//! - `chord_loops.rs`: the CL1 interactive Chord Loops group —
//!   create / rename / delete / duplicate / pick color, plus
//!   selection writes into [`AppState::selected_chord_loop`]. CL2's
//!   chord-loop editor reads that signal and renders the open loop.
//!
//! Patterns + Sections grow the same kind of CRUD when their
//! Tier-1 plans land; mirror this module's structure (own sub-file).

use rinch::prelude::*;

use crate::parts::{rgba, Icon};
use crate::state::AppState;
use crate::theme;

mod chord_loops;

use chord_loops::ChordLoopsGroup;

#[component]
pub fn Library() -> NodeHandle {
    let pane_style = format!(
        "width: 260px; flex: 0 0 260px; \
         background: {bg}; border-right: 1px solid {line}; \
         display: flex; flex-direction: column; min-height: 0;",
        bg = theme::BG1,
        line = theme::LINE,
    );

    rsx! {
        aside { style: {pane_style.clone()},
            SearchBar { }
            div {
                style: "flex: 1; overflow-y: auto; min-height: 0;",
                PatternsGroup    { }
                ChordLoopsGroup  { }
                SectionsGroup    { }
            }
        }
    }
}

#[component]
fn SearchBar() -> NodeHandle {
    let wrap_style = format!(
        "padding: 8px; border-bottom: 1px solid {line};",
        line = theme::LINE,
    );
    let inner_style = format!(
        "display: flex; align-items: center; gap: 6px; \
         padding: 5px 8px; border-radius: 4px; \
         background: {bg0}; border: 1px solid {line};",
        bg0 = theme::BG0, line = theme::LINE,
    );
    let kbd_style = format!(
        "font-size: 10px; color: rgba(232,234,238,0.28); \
         padding: 1px 4px; border: 1px solid {line}; border-radius: 3px; \
         font-family: ui-monospace, SFMono-Regular, monospace;",
        line = theme::LINE,
    );
    let stroke = "rgba(232,234,238,0.42)".to_string();
    let sz = 12.0_f32;
    let sw = 1.6_f32;

    rsx! {
        div { style: {wrap_style.clone()},
            div { style: {inner_style.clone()},
                Icon { glyph: "search", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
                span {
                    style: "font-size: 12px; color: rgba(232,234,238,0.28); flex: 1;",
                    "Search library"
                }
                span { style: {kbd_style.clone()}, "⌘K" }
            }
        }
    }
}

// ─── Group shells (one inline per category to keep `for` data-driven) ────

/// One library row, pre-resolved against model + overlay. Owned `String`
/// fields satisfy rsx `for`'s `Clone + PartialEq + 'static` bound and
/// keep the row constructors agnostic of the source model types.
#[derive(Clone, PartialEq)]
struct LibraryRowData {
    key: String,
    color: String,
    name: String,
    meta: String,
}

#[component]
fn PatternsGroup() -> NodeHandle {
    let rows = build_pattern_rows();
    let count = rows.len() as u32;
    rsx! {
        div { style: {group_outer_style()},
            GroupHeader { title: "Patterns", count: count, glyph: "pattern" }
            div { style: "padding-bottom: 4px;",
                for row in rows.clone() {
                    LibraryRow {
                        key: row.key,
                        color: row.color,
                        name: row.name,
                        meta: row.meta,
                        highlighted: false,
                    }
                }
                NewRow { label: "new pattern", onclick: move || {} }
            }
        }
    }
}

fn build_pattern_rows() -> Vec<LibraryRowData> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let overlay = app.overlay.get();
    project
        .patterns
        .values()
        .map(|p| LibraryRowData {
            key: p.name.clone(),
            color: overlay
                .pattern_color
                .get(&p.id)
                .cloned()
                .unwrap_or_else(|| theme::TEXT2.to_string()),
            name: p.name.clone(),
            meta: overlay.pattern_meta.get(&p.id).cloned().unwrap_or_default(),
        })
        .collect()
}

#[component]
fn SectionsGroup() -> NodeHandle {
    let rows = build_section_rows();
    let count = rows.len() as u32;
    rsx! {
        div { style: {group_outer_style()},
            GroupHeader { title: "Sections", count: count, glyph: "section" }
            div { style: "padding-bottom: 4px;",
                for row in rows.clone() {
                    LibraryRow {
                        key: row.key,
                        color: row.color,
                        name: row.name,
                        meta: row.meta,
                        highlighted: false,
                    }
                }
                NewRow { label: "new section", onclick: move || {} }
            }
        }
    }
}

fn build_section_rows() -> Vec<LibraryRowData> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let overlay = app.overlay.get();
    project
        .sections
        .values()
        .map(|s| {
            // Variant count: base + each named variant override.
            let variant_count = 1 + s.variants.len();
            let meta = if variant_count > 1 {
                format!("{} bars · {} variants", s.base.duration_bars, variant_count)
            } else {
                format!("{} bars", s.base.duration_bars)
            };
            LibraryRowData {
                key: s.name.clone(),
                color: overlay
                    .section_color
                    .get(&s.id)
                    .cloned()
                    .unwrap_or_else(|| theme::TEXT2.to_string()),
                name: s.name.clone(),
                meta,
            }
        })
        .collect()
}

/// Shared outer chrome for every Library group. `pub(super)` so the
/// `chord_loops` submodule can reuse it.
pub(super) fn group_outer_style() -> String {
    format!("border-bottom: 1px solid {line};", line = theme::LINE)
}

// ─── Header + Row + New-row helpers ───────────────────────────────────────

#[component]
pub(super) fn GroupHeader(title: String, count: u32, glyph: String) -> NodeHandle {
    let header_style = "display: flex; align-items: center; gap: 6px; \
         width: 100%; padding: 8px 10px; \
         background: transparent; border: 0; \
         color: rgba(232,234,238,0.62); cursor: pointer; \
         text-align: left;";
    let title_style = "font-size: 11px; letter-spacing: 0.6px; \
         text-transform: uppercase; font-weight: 600; \
         color: rgba(232,234,238,0.62); flex: 1;";
    let count_style = "font-size: 11px; color: rgba(232,234,238,0.28); \
         font-feature-settings: \"tnum\" 1;";
    let chev_stroke = "rgba(232,234,238,0.42)".to_string();
    let icon_stroke = "rgba(232,234,238,0.62)".to_string();
    let sw = 1.6_f32;
    let chev_sz = 12.0_f32;
    let icon_sz = 13.0_f32;
    let count_str = count.to_string();

    rsx! {
        button {
            r#type: "button",
            style: {header_style.to_string()},
            Icon { glyph: "chevron-d", size: chev_sz, stroke: {chev_stroke.clone()}, stroke_width: sw }
            Icon { glyph: {glyph.clone()}, size: icon_sz, stroke: {icon_stroke.clone()}, stroke_width: sw }
            span { style: {title_style.to_string()}, {title.clone()} }
            span { style: {count_style.to_string()}, {count_str.clone()} }
        }
    }
}

#[component]
fn LibraryRow(
    color: String,
    name: String,
    meta: String,
    highlighted: bool,
) -> NodeHandle {
    let bg = if highlighted {
        rgba(color.as_str(), 0.10)
    } else {
        "transparent".to_string()
    };
    let border_left = if highlighted {
        format!("2px solid {}", color)
    } else {
        "2px solid transparent".to_string()
    };
    let row_style = format!(
        "display: flex; align-items: center; gap: 8px; \
         padding: 4px 8px; \
         background: {bg}; border-left: {border_left}; \
         cursor: grab; min-height: 28px;",
    );
    let swatch_style = format!(
        "width: 10px; height: 10px; border-radius: 2px; flex: 0 0 auto; \
         background: {color}; border: 1px solid {border};",
        color = color, border = rgba(color.as_str(), 0.6),
    );
    let stack_style = "display: flex; flex-direction: column; \
         min-width: 0; gap: 0; flex: 1;";
    let name_style = "font-size: 12.5px; color: rgba(232,234,238,0.96); \
         font-weight: 500; overflow: hidden; text-overflow: ellipsis; \
         white-space: nowrap; line-height: 1.25;";
    let meta_style = "font-size: 10.5px; color: rgba(232,234,238,0.42); \
         line-height: 1.25; font-feature-settings: \"tnum\" 1;";

    rsx! {
        div { style: {row_style.clone()},
            span { style: {swatch_style.clone()} }
            div { style: {stack_style.to_string()},
                div { style: {name_style.to_string()}, {name.clone()} }
                div { style: {meta_style.to_string()}, {meta.clone()} }
            }
        }
    }
}

#[component]
pub(super) fn NewRow(label: String, onclick: Callback) -> NodeHandle {
    let btn_style = "display: flex; align-items: center; gap: 6px; \
         margin: 4px 10px 8px; padding: 4px 6px; \
         background: transparent; border: 0; \
         color: rgba(232,234,238,0.42); cursor: pointer; \
         border-radius: 3px; font-size: 11px;";
    let stroke = "rgba(232,234,238,0.42)".to_string();
    let sz = 12.0_f32;
    let sw = 1.6_f32;
    rsx! {
        button {
            r#type: "button",
            style: {btn_style.to_string()},
            onclick: move || onclick.invoke(),
            Icon { glyph: "plus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            span { {label.clone()} }
        }
    }
}
