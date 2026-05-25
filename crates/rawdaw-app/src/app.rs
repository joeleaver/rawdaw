//! The main window composition.
//!
//! Owns the shared `AppState` store (installed via `create_store`) and
//! switches the content area below the top bar between the round-1
//! arrangement view and the round-2 section editor based on
//! `state::EditorMode`. The top bar persists across both modes.

use rinch::core::events::set_keyboard_interceptor;
use rinch::core::reactive::Effect;
use rinch::prelude::*;
use rawdaw_engine::Transport;

use std::rc::Rc;

use crate::audio::AudioResources;
use crate::initial_project;
use crate::regions::{
    Arrangement, BottomStrip, ChordLoopEditor, Inspector, Library, PatternEditor, TopBar,
    TracksPane,
};
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
    let audio = create_store(AudioResources::build());

    // C1b: seed AppState's project + overlay signals from the shared
    // `initial_project::build_initial()` factory. `AudioResources::build`
    // above also invokes the factory internally for its own audio-graph
    // seed (separate `Rc` allocation today; C2's edit pump unifies them).
    // The seed happens before the rsx tree below renders so no region
    // ever observes the empty `AppState::new()` defaults.
    let (initial, initial_overlay) = initial_project::build_initial();
    app.project.set(Rc::new(initial));
    app.overlay.set(Rc::new(initial_overlay));

    // K3: seed the MIDI routing target from the first Pitched track
    // (matches the boot-time `midi_target` atomic on AudioResources)
    // and wire an Effect that propagates AppState selection changes
    // back to the audio side. The Effect runs once on creation with
    // the seeded value, then on every `midi_target_track.get()`
    // change. AudioResources::set_midi_target_track stores into the
    // `Arc<AtomicU32>` shared with midir's callback thread; the next
    // incoming MIDI message gets routed to the new NodeId.
    app.midi_target_track.set(audio.first_pitched_track_index());
    let _ = Effect::new(move || {
        let target = app.midi_target_track.get();
        audio.set_midi_target_track(target);
    });

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
                // CL2: when a chord loop is selected, the editor
                // takes over the center stage and the regular
                // TracksPane + Arrangement + Inspector triplet
                // disappears (per CL0 design decision 1 of the
                // chord-loop-editing plan). The match scrutinee
                // reads a Signal so rsx tracks selection changes
                // and surgically swaps the subtree.
                // `_chord_loop_id` underscore prefix silences the
                // unused-variable lint — the rsx macro hides the
                // inner prop use from the lint pass, but
                // `_`-prefixed names are still usable in Rust.
                if let Some(_chord_loop_id) =
                    use_store::<AppState>().selected_chord_loop.get()
                {
                    ChordLoopEditor { id: _chord_loop_id }
                } else if use_store::<AppState>().selected_pattern.get().is_some() {
                    // P2: pattern editor takes center stage when a
                    // pattern is selected. The keyed for-loop forces
                    // PatternEditor (and every sub-component that
                    // captured `id` at construction) to remount when
                    // the user clicks a different pattern in the
                    // Library — a bare `if let { PatternEditor { id }
                    // }` would re-evaluate the if-let condition but
                    // keep the same component instance with stale
                    // props.
                    for pid_value in pattern_editor_mount_keys() {
                        PatternEditor {
                            key: pid_value.to_string(),
                            id: rawdaw_model::id::PatternId::new(pid_value),
                        }
                    }
                } else {
                    StandardArrangementRow { }
                }
            }
            BottomStrip { }
        }
    }
}

/// The default arrangement row: TracksPane + Arrangement +
/// Inspector. Pulled out so the chord-loop-editor swap above
/// reads cleanly and the for-loop wrapping the Inspector keeps
/// its existing selection-key re-mount behavior.
#[component]
fn StandardArrangementRow() -> NodeHandle {
    rsx! {
        // rsx fragment-like wrapper — the parent's flex row owns
        // layout, so emit the three siblings inside a transparent
        // wrapper.
        div { style: "flex: 1; display: flex; min-width: 0;",
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
/// Single-element key vector for the pattern editor mount. The
/// element is the selected pattern id's raw `u64`, so a click on a
/// different pattern row produces a different key and the for-loop
/// remounts the editor subtree. Returns an empty vec when no
/// pattern is selected (the if-let branch above prevents us from
/// reaching this in that state, but the contract stays clean).
fn pattern_editor_mount_keys() -> Vec<u64> {
    use_store::<AppState>()
        .selected_pattern
        .get()
        .map(|id| vec![id.get()])
        .unwrap_or_default()
}

fn inspector_selection_keys() -> Vec<String> {
    let app = use_store::<AppState>();
    let key = match (
        app.selected_idx.get(),
        app.selected_track.get(),
        app.selected_master_fx.get(),
    ) {
        (Some(idx), _, _) => format!("s{idx}"),
        (None, Some(t), _) => format!("t{t}"),
        (None, None, Some(slot)) => format!("m{slot}"),
        (None, None, None) => "none".to_string(),
    };
    vec![key]
}
