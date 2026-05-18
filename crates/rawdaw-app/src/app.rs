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
///
/// Selection is reactive: clicking a SectionBlock writes to
/// `AppState::selected_idx`. The Arrangement reads it inline (per-block
/// reactive style closures); the Inspector re-mounts on selection change
/// via a `for sel in inspector_selection_keys()` keyed singleton — the
/// whole inspector body depends on which section is selected, so a
/// remount is cheaper than wiring fine-grained closures through every
/// header / table cell.
#[component]
fn ArrangementSurface() -> NodeHandle {
    let row_style = "flex: 1; display: flex; min-height: 0;";
    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-height: 0;",
            div { style: {row_style},
                Library { }
                Arrangement { }
                for sel in inspector_selection_keys() {
                    Inspector { key: sel.0, idx: sel.1 }
                }
            }
            BottomStrip { }
        }
    }
}

/// Singleton iterator used by `ArrangementSurface` to force an Inspector
/// re-mount on every selection change. Returns a 1-element vec of
/// `(key, idx)` — `key` is the for-loop identity used by rsx
/// reconciliation (`usize::MAX` stands in for `None`), and `idx` is the
/// actual prop value passed into Inspector. When the user clicks a
/// different SectionBlock, the key changes → rsx unmounts the old
/// Inspector and mounts a new one with fresh content. Pulled out as a
/// free function because the rsx `for` source must be `Fn() -> Vec<T>`
/// callable.
fn inspector_selection_keys() -> Vec<(usize, Option<usize>)> {
    let app = use_store::<AppState>();
    let sel = app.selected_idx.get();
    vec![(sel.unwrap_or(usize::MAX), sel)]
}
