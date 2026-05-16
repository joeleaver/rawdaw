//! Section editor — the round-2 surface.
//!
//! Translates `docs/design/mockups/round-2/`. Replaces the arrangement
//! view's middle row when `EditorMode::SectionEditor` is active; the
//! top bar persists across modes.
//!
//! ## Module layout
//!
//! - `header` — top header strip (back / breadcrumb / name / Done).
//! - `variant_tabs` — `base default variant · stripped · + New variant`.
//! - `meta_bar` — Duration / Scale / Chord loops with `↳ base`
//!   inheritance markers on non-base variants.
//! - `activations` — Activations header (cells land in Phase 4).
//!
//! Phase 2 ships the chrome with stock primitives; the activation cells
//! and custom-SVG primitives come in Phases 3–6.

use rinch::prelude::*;

use crate::fixture;
use crate::state::{AppState, EditorMode};
use crate::theme;

mod activations;
mod cell;
mod chord_loop_bar;
mod header;
mod meta_bar;
mod variant_tabs;

use activations::ActivationsHeader;
use cell::CellList;
use header::SectionEditorHeader;
use meta_bar::SectionMetaBar;
use variant_tabs::VariantTabs;

/// Root of the section-editor surface. Reads the current
/// `EditorMode::SectionEditor { section_key, variant }` out of the
/// shared `AppState` store and renders the editor for it.
///
/// If the mode somehow isn't `SectionEditor` we render an inert
/// "no section selected" message — by construction the parent only
/// mounts us in section-editor mode, but the fallback keeps the
/// component total.
#[component]
pub fn SectionEditor() -> NodeHandle {
    let app = use_store::<AppState>();
    // `section_key` is captured once at mount — within a section-editor
    // session it doesn't change; only `variant` changes via tab clicks,
    // and that's read reactively by VariantTabs / SectionMetaBar
    // internals. Returning to the arrangement re-mounts this component.
    let section_key = match app.editor_mode.get() {
        EditorMode::SectionEditor { section_key, .. } => section_key,
        EditorMode::Arrangement => String::new(),
    };

    let r = fixture::round1();
    let section = fixture::section_by_key(&r, section_key.as_str());
    let (section_name, section_color, default_variant) = match section {
        Some(s) => (
            s.name.to_string(),
            s.color.to_string(),
            s.default_variant.to_string(),
        ),
        None => (
            "(unknown section)".to_string(),
            theme::TEXT2.to_string(),
            "base".to_string(),
        ),
    };

    let surface_style = format!(
        "flex: 1; min-height: 0; display: flex; flex-direction: column; \
         background: {bg}; color: {text}; \
         font-family: {font}; font-size: 13px;",
        bg = theme::BG0,
        text = theme::TEXT0,
        font = theme::FONT_SANS,
    );

    rsx! {
        div { style: {surface_style.clone()},
            SectionEditorHeader {
                section_color: section_color.clone(),
                section_name: section_name,
            }
            VariantTabs {
                section_name_key: section_key.clone(),
                section_color: section_color,
                default_variant: default_variant,
            }
            SectionMetaBar {
                section_name_key: section_key,
            }
            ActivationsHeader { }
            CellList { }
        }
    }
}
