//! The main window composition.
//!
//! Owns the shared `AppState` store (installed via `create_store`) and
//! switches the content area below the top bar between the round-1
//! arrangement view and the round-2 section editor based on
//! `state::EditorMode`. The top bar persists across both modes.

use rinch::core::events::set_keyboard_interceptor;
use rinch::prelude::*;
use rawdaw_engine::Transport;

use crate::audio::AudioResources;
use crate::regions::{Arrangement, BottomStrip, Inspector, Library, TopBar, TracksPane};
use crate::section_editor::SectionEditor;
use crate::state::{AppState, EditorMode};
use crate::theme;

#[component]
pub fn main_window() -> NodeHandle {
    // Install the shared stores once at the top of the tree. Every
    // nested component reaches AppState via `use_store::<AppState>()`;
    // audio resources are installed alongside so handlers (E6
    // transport, future MIDI) can consume them from any closure.
    let app = create_store(AppState::new());
    let _audio = create_store(AudioResources::build());

    // Global Space → Play/Pause shortcut. Rinch's keyboard interceptor
    // is a global singleton — only one can be active at a time — so
    // we install it once here at app startup. Returning `false` for
    // every key except Space lets rinch's normal handling continue
    // (text input focus, contenteditable, etc.); a future text-input
    // field would need to coordinate with this interceptor if it
    // wants to consume Space.
    set_keyboard_interceptor(|data| {
        if data.key == "Space" && !data.ctrl && !data.alt && !data.meta {
            let audio = use_store::<AudioResources>();
            // Read the audio-thread atomic here, not the UI signal —
            // we want a no-rinch-effect-tracking peek (the interceptor
            // closure isn't inside any reactive context).
            let _ = if audio.transport.get() == Transport::Playing {
                audio.pause()
            } else {
                audio.play()
            };
            return true;
        }
        false
    });

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
                TracksPane { }
                Arrangement { }
                // The for-loop drives an Inspector re-mount whenever
                // `inspector_selection_keys()` returns a different
                // string. The macro's key-fallback path Debug-formats
                // the iteration variable for `for_each_dom_typed`'s
                // identity, so the loop body itself never has to
                // reference `sel` directly — the `_` prefix mirrors
                // that intent and silences rustc's unused warning.
                for _sel in inspector_selection_keys() {
                    Inspector { }
                }
            }
            BottomStrip { }
        }
    }
}

/// Singleton iterator used by `ArrangementSurface` to force an
/// Inspector re-mount on every selection change. Returns a 1-element
/// vec of a string key derived from both selection axes — section-
/// block (`selected_idx`) and project-track (`selected_track`). When
/// either changes, the key changes and rsx unmounts the old Inspector
/// and mounts a new one with fresh content. Pulled out as a free
/// function because the rsx `for` source must be a `Fn() -> Vec<T>`
/// callable.
fn inspector_selection_keys() -> Vec<String> {
    let app = use_store::<AppState>();
    let key = match (app.selected_idx.get(), app.selected_track.get()) {
        (Some(idx), _) => format!("s{idx}"),
        (None, Some(t)) => format!("t{t}"),
        (None, None) => "none".to_string(),
    };
    vec![key]
}
