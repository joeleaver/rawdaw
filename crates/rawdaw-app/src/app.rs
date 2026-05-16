//! The main window composition.
//!
//! Owns the shared `AppState` store (installed via `create_store`) and
//! switches the content area below the top bar between the round-1
//! arrangement view and the round-2 section editor based on
//! `state::EditorMode`. The top bar persists across both modes.

use rinch::prelude::*;

use crate::regions::{Arrangement, BottomStrip, Inspector, Library, TopBar};
use crate::section_editor::SectionEditor;
use crate::state::{AppState, EditorMode};
use crate::theme;

#[component]
pub fn main_window() -> NodeHandle {
    // Install the shared store once at the top of the tree. Every nested
    // component reaches this via `use_store::<AppState>()`.
    let app = create_store(AppState::new());

    let style = format!(
        "width: 100%; height: 100%; \
         background: {bg}; color: rgba(232,234,238,0.96); \
         font-family: {font}; font-size: 13px; line-height: 1.4; \
         display: flex; flex-direction: column; overflow: hidden;",
        bg = theme::BG0,
        font = theme::FONT_SANS,
    );
    rsx! {
        div { style: {style.clone()},
            TopBar { }
            // Native `match` inside rsx is auto-tracked by the reactive
            // system (per rinch Rule 14), so switching `editor_mode`
            // surgically swaps the subtree without rebuilding TopBar.
            match app.editor_mode.get() {
                EditorMode::Arrangement => ArrangementSurface { },
                EditorMode::SectionEditor { .. } => SectionEditor { },
            }
        }
    }
}

/// Round-1 main-window middle + bottom: library / arrangement / inspector
/// row + bottom strip. Pulled out so the `match` above can keep both arms
/// terse and so the section-editor mode doesn't pay for any of these
/// regions' renders.
#[component]
fn ArrangementSurface() -> NodeHandle {
    let row_style = "flex: 1; display: flex; min-height: 0;";
    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",
            div { style: {row_style},
                Library { }
                // Round 1 selection is hardcoded to "verse base at bar 5–8"
                // (idx=1) so the inspector renders its populated state and
                // the arrangement shows linked-highlight on the matching
                // section blocks. Real click-driven selection is queued
                // for after the round-2 port (engine wiring milestone).
                Arrangement { selected_idx: Some(1usize) }
                Inspector { idx: Some(1usize) }
            }
            BottomStrip { }
        }
    }
}
