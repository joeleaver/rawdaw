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
//! ## File layout (post-CL1 + post-P1 + post-S2)
//!
//! - `mod.rs` (this file): Library shell + SearchBar + shared
//!   header / `+ new …` primitives.
//! - `chord_loops.rs`: the CL1 interactive Chord Loops group —
//!   create / rename / delete / duplicate / pick color, plus
//!   selection writes into [`AppState::selected_chord_loop`]. CL2's
//!   chord-loop editor reads that signal and renders the open loop.
//! - `patterns.rs`: the P1 interactive Patterns group — same CRUD
//!   shape, with two `+` affordances (pitched + drum) since
//!   `PatternBody` kind is locked at creation. Selection writes
//!   [`AppState::selected_pattern`]; the P2 pattern editor reads
//!   that signal.
//! - `sections.rs`: the S2 interactive Sections group — same CRUD
//!   shape as `chord_loops.rs`. Selection writes
//!   [`AppState::selected_section`]; S3's meta-bar work wires the
//!   section editor open path off it.

use rinch::prelude::*;

use crate::parts::Icon;
use crate::theme;

mod chord_loops;
mod patterns;
mod sections;

use chord_loops::ChordLoopsGroup;
use patterns::PatternsGroup;
use sections::SectionsGroup;

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

    rsx! {
        div { style: {wrap_style.clone()},
            div { style: {inner_style.clone()},
                Icon { glyph: "search", size: 12.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
                span {
                    style: "font-size: 12px; color: rgba(232,234,238,0.28); flex: 1;",
                    "Search library"
                }
                span { style: {kbd_style.clone()}, "⌘K" }
            }
        }
    }
}

// ─── Shared chrome (used by every group sub-module) ───────────────────────

/// Shared outer chrome for every Library group. `pub(super)` so the
/// per-group sub-modules can reuse it.
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
    let count_str = count.to_string();

    rsx! {
        button {
            r#type: "button",
            style: {header_style.to_string()},
            Icon { glyph: "chevron-d", size: 12.0, stroke: {chev_stroke.clone()}, stroke_width: 1.6 }
            Icon { glyph: {glyph.clone()}, size: 13.0, stroke: {icon_stroke.clone()}, stroke_width: 1.6 }
            span { style: {title_style.to_string()}, {title.clone()} }
            span { style: {count_style.to_string()}, {count_str.clone()} }
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
    rsx! {
        button {
            r#type: "button",
            style: {btn_style.to_string()},
            onclick: move || onclick.invoke(),
            Icon { glyph: "plus", size: 12.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
            span { {label.clone()} }
        }
    }
}
