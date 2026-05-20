//! Top bar — project identity (left) · transport (center) · tempo/zoom (right).
//!
//! Translates `docs/design/mockups/round-1/components/topbar.jsx`.
//! The chrome row sits at `theme::BG1` so it lifts off the arrangement
//! (`theme::BG0`); the inset readouts inside use `theme::BG0` to look
//! recessed against the lighter chrome — same trick as the mockup.
//!
//! File layout (post-C4 split — `composition-writability-plan.md`):
//!
//! - `mod.rs` (this file): the `TopBar` component composing the three
//!   columns plus the shared `Tag` micro-helper.
//! - `transport.rs`: `TransportBtn` glyphed button used by the center
//!   column.
//! - `midi_picker.rs`: the K2 `MidiPicker` dropdown used on the right.
//! - `project_meta.rs`: the `Project ▾` menu (C3) plus the C4 BPM /
//!   name / key controls and the debug `BumpBpmDebug` placeholder
//!   (cfg-gated, removed when C4.4 lands real BPM editing).
//!
//! The submodules are intentionally `pub(super)` — TopBar is the only
//! outside consumer.

use rinch::prelude::*;

use crate::audio::AudioResources;
use crate::project_display::time_signature_label;
use crate::state::AppState;
use crate::theme;

mod midi_picker;
mod project_meta;
mod transport;

use midi_picker::MidiPicker;
use project_meta::{BpmControls, IconBtn, KeyDropdown, NameControl, ProjectMenu};
use transport::TransportBtn;

#[component]
pub fn TopBar() -> NodeHandle {
    let bar_style = format!(
        "height: {h}px; flex: 0 0 {h}px; \
         background: {bg}; border-bottom: 1px solid {line}; \
         display: grid; grid-template-columns: 1fr auto 1fr; \
         align-items: center; padding: 0 12px; gap: 12px;",
        h = theme::H_TOPBAR,
        bg = theme::BG1,
        line = theme::LINE,
    );

    rsx! {
        div { style: {bar_style.clone()},
            // Left: Project menu + identity
            div {
                style: "display: flex; align-items: center; gap: 10px; min-width: 0;",
                ProjectMenu { }
                div {
                    style: "display: flex; flex-direction: column; \
                            justify-content: center; gap: 2px; min-width: 0;",
                    NameControl { }
                    div {
                        style: "display: flex; align-items: center; gap: 6px; \
                                padding-left: 4px; \
                                font-size: 11px; color: rgba(232,234,238,0.42); \
                                font-feature-settings: \"tnum\" 1;",
                        KeyDropdown { }
                        span { style: "opacity: 0.5;", "·" }
                        // Time signature is read-only for now —
                        // editing lands when the tempo-map model
                        // grows beyond constant-BPM (currently the
                        // engine only honors the first time-signature
                        // event anyway, so an editor would over-
                        // promise).
                        span {
                            {|| {
                                let project = use_store::<AppState>().project.get();
                                time_signature_label(&project.tempo_map)
                            }}
                        }
                    }
                }
            }

            // Center: transport.
            //
            // Handlers read `AudioResources` from the rinch store (installed
            // by `main_window` at app launch). The `move ||` closures
            // capture the store reference; `use_store::<AudioResources>()`
            // re-resolves the reference on each click, so cloning the
            // store handle into the closure is unnecessary.
            //
            // Play is a toggle: pressing while Playing pauses; otherwise
            // plays. Stop / Rewind both reset to bar 1. Record stays
            // disabled (MIDI v1 has no audio recording surface yet).
            div {
                style: "display: flex; align-items: center; gap: 4px;",
                TransportBtn {
                    glyph: "rewind",
                    title: "Return to zero",
                    onclick: move || {
                        let audio = use_store::<AudioResources>();
                        let _ = audio.stop();
                    },
                }
                // Reactive Play/Pause glyph. rsx `match` is auto-tracked
                // (rinch Rule 14) IF the scrutinee reads a `Signal` —
                // atomic loads aren't subscribed by the reactivity
                // tracker. We read `transport_state: Signal<Transport>`
                // here (the UI-facing mirror), which the play/pause/stop
                // methods keep in lockstep with the audio-thread atomic.
                match use_store::<AudioResources>().transport_state.get() {
                    rawdaw_engine::Transport::Playing => TransportBtn {
                        glyph: "pause",
                        title: "Pause (Space)",
                        primary: true,
                        onclick: move || {
                            let _ = use_store::<AudioResources>().pause();
                        },
                    },
                    _ => TransportBtn {
                        glyph: "play",
                        title: "Play (Space)",
                        primary: true,
                        onclick: move || {
                            let _ = use_store::<AudioResources>().play();
                        },
                    },
                }
                TransportBtn {
                    glyph: "stop",
                    title: "Stop",
                    onclick: move || {
                        let audio = use_store::<AudioResources>();
                        let _ = audio.stop();
                    },
                }
                TransportBtn {
                    glyph: "record",
                    title: "Record (disabled — MIDI v1 has no audio recording)",
                    disabled: true,
                }
                div {
                    style: format!(
                        "width: 1px; height: 18px; background: {line}; margin: 0 6px;",
                        line = theme::LINE,
                    ),
                }
                // Bar / beat readout (inset against bg1)
                div {
                    style: format!(
                        "display: flex; align-items: baseline; gap: 6px; \
                         padding: 3px 9px; border-radius: 4px; \
                         background: {bg0}; border: 1px solid {line}; \
                         font-size: 12px; color: rgba(232,234,238,0.96); \
                         font-feature-settings: \"tnum\" 1; \
                         font-variant-numeric: tabular-nums;",
                        bg0 = theme::BG0, line = theme::LINE,
                    ),
                    Tag { text: "Bar" }
                    // Bar / beat readouts read `AudioResources::playhead_position()`
                    // inside the rsx text expression — the `{|| ...}` closure
                    // wrapper subscribes to the engine sample clock so the
                    // values update whenever the audio thread publishes a new
                    // block boundary.
                    span { style: "min-width: 14px; text-align: right;",
                        {|| use_store::<AudioResources>().playhead_position().bar.to_string()}
                    }
                    span { style: "color: rgba(232,234,238,0.28);", "·" }
                    Tag { text: "Beat" }
                    span { style: "min-width: 8px;",
                        {|| use_store::<AudioResources>().playhead_position().beat.to_string()}
                    }
                }
            }

            // Right: MIDI input · tempo · zoom · gear
            div {
                style: "display: flex; align-items: center; gap: 6px; \
                        justify-content: flex-end;",
                MidiPicker { }
                // C4: editable tempo replaces the C2 read-only span +
                // debug `+1 BPM` button. `BpmControls` carries the
                // number-input + ± nudges and commits through the
                // edit pump.
                BpmControls { }
                div { style: "display: flex; gap: 1px; margin-left: 4px;",
                    IconBtn { glyph: "zoom-out", title: "Zoom out" }
                    IconBtn { glyph: "zoom-in",  title: "Zoom in"  }
                }
                IconBtn { glyph: "gear", title: "Project settings" }
            }
        }
    }
}

// ─── Shared helpers ──────────────────────────────────────────────────────

/// Small uppercase label inside numeric readouts ("Bar", "Beat", "BPM",
/// "MIDI"). Shared across the transport readout, the BPM readout, and
/// the MIDI picker chrome.
#[component]
pub(super) fn Tag(text: String) -> NodeHandle {
    rsx! {
        span {
            style: "color: rgba(232,234,238,0.42); font-size: 10px; \
                    text-transform: uppercase; letter-spacing: 0.6px;",
            {text.clone()}
        }
    }
}
