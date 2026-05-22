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

use rawdaw_model::id::SectionId;

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
/// `EditorMode::SectionEditor { section_id, variant }` out of the
/// shared `AppState` store and renders the editor for it.
///
/// If the mode somehow isn't `SectionEditor` we render an inert
/// "no section selected" message — by construction the parent only
/// mounts us in section-editor mode, but the fallback keeps the
/// component total.
#[component]
pub fn SectionEditor() -> NodeHandle {
    let app = use_store::<AppState>();
    // `section_id` is captured once at mount — within a section-editor
    // session it doesn't change; only `variant` changes via tab clicks,
    // and that's read reactively by VariantTabs / SectionMetaBar
    // internals. Returning to the arrangement re-mounts this component.
    let section_id = match app.editor_mode.get() {
        EditorMode::SectionEditor { section_id, .. } => section_id,
        EditorMode::Arrangement => SectionId::default(),
    };

    let project = app.project.get();
    let overlay = app.overlay.get();
    let section = project.sections.get(&section_id);
    let (section_name, section_color, default_variant) = match section {
        Some(s) => (
            s.name.clone(),
            overlay
                .section_color
                .get(&s.id)
                .cloned()
                .unwrap_or_else(|| theme::ACCENT.to_string()),
            s.default_variant.as_str().to_string(),
        ),
        None => (
            "(unknown section)".to_string(),
            theme::ACCENT.to_string(),
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
                section_id: section_id.get(),
                section_color: section_color.clone(),
                section_name: section_name,
            }
            VariantTabs {
                section_id: section_id.get(),
                section_color: section_color,
                default_variant: default_variant,
            }
            SectionMetaBar {
                section_id: section_id.get(),
            }
            ActivationsHeader { }
            CellList { }
        }
    }
}
