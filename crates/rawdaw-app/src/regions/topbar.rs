//! Top bar — project identity (left) · transport (center) · tempo/zoom (right).
//!
//! Translates `docs/design/mockups/round-1/components/topbar.jsx`.
//! The chrome row sits at `theme::BG1` so it lifts off the arrangement
//! (`theme::BG0`); the inset readouts inside use `theme::BG0` to look
//! recessed against the lighter chrome — same trick as the mockup.

use rinch::prelude::*;

use crate::audio::AudioResources;
use crate::parts::Icon;
use crate::project_display::{current_bpm, scale_label, time_signature_label};
use crate::state::AppState;
use crate::theme;

/// Sentinel value for the "no MIDI device" option in [`MidiPicker`].
/// Picked as an empty string so it's distinguishable from any real
/// device name (midir wraps a non-empty string for every enumerated
/// port — see `MidiInputDevice::name`).
const MIDI_NONE_VALUE: &str = "";

#[component]
pub fn TopBar() -> NodeHandle {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let overlay = app.overlay.get();

    let bar_style = format!(
        "height: {h}px; flex: 0 0 {h}px; \
         background: {bg}; border-bottom: 1px solid {line}; \
         display: grid; grid-template-columns: 1fr auto 1fr; \
         align-items: center; padding: 0 12px; gap: 12px;",
        h = theme::H_TOPBAR,
        bg = theme::BG1,
        line = theme::LINE,
    );

    let project_name = overlay.project_name.clone();
    let project_meta = format!(
        "{} · {}",
        scale_label(&project.default_key),
        time_signature_label(&project.tempo_map),
    );
    let tempo = format!("{:.2}", current_bpm(&project.tempo_map));

    rsx! {
        div { style: {bar_style.clone()},
            // Left: project identity
            div {
                style: "display: flex; flex-direction: column; \
                        justify-content: center; gap: 1px; min-width: 0;",
                div {
                    style: "font-size: 13px; font-weight: 600; \
                            color: rgba(232,234,238,0.96); \
                            letter-spacing: -0.1px; \
                            overflow: hidden; text-overflow: ellipsis; \
                            white-space: nowrap;",
                    {project_name.clone()}
                }
                div {
                    style: "font-size: 11px; color: rgba(232,234,238,0.42); \
                            font-feature-settings: \"tnum\" 1;",
                    {project_meta.clone()}
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
                div {
                    style: format!(
                        "display: flex; align-items: baseline; gap: 6px; \
                         padding: 3px 9px; border-radius: 4px; \
                         background: {bg0}; border: 1px solid {line}; \
                         font-size: 12px; color: rgba(232,234,238,0.96); \
                         font-feature-settings: \"tnum\" 1;",
                        bg0 = theme::BG0, line = theme::LINE,
                    ),
                    span {
                        style: "min-width: 22px; text-align: right; \
                                font-variant-numeric: tabular-nums;",
                        {tempo.clone()}
                    }
                    Tag { text: "BPM" }
                }
                div { style: "display: flex; gap: 1px; margin-left: 4px;",
                    IconBtn { glyph: "zoom-out", title: "Zoom out" }
                    IconBtn { glyph: "zoom-in",  title: "Zoom in"  }
                }
                IconBtn { glyph: "gear", title: "Project settings" }
            }
        }
    }
}

// ─── Helpers (PascalCase components) ──────────────────────────────────────

/// Small uppercase label inside numeric readouts ("Bar", "Beat", "BPM").
#[component]
fn Tag(text: String) -> NodeHandle {
    rsx! {
        span {
            style: "color: rgba(232,234,238,0.42); font-size: 10px; \
                    text-transform: uppercase; letter-spacing: 0.6px;",
            {text.clone()}
        }
    }
}

/// Transport button (28×28). Plays / stops / rewinds / records. Record is
/// rendered as a filled dot, not a stroked path.
///
/// `onclick` defaults to a no-op (the macro's default impl for `Callback`),
/// so the disabled Record button can omit it. Active buttons pass a
/// closure that drives [`crate::audio::AudioResources`].
#[component]
fn TransportBtn(
    glyph: String,
    title: String,
    primary: bool,
    disabled: bool,
    onclick: Callback,
) -> NodeHandle {
    // Color choice: disabled → dim; primary → ok; otherwise → text0.
    let color = if disabled {
        "rgba(232,234,238,0.28)"
    } else if primary {
        theme::OK
    } else {
        "rgba(232,234,238,0.96)"
    };
    let style = format!(
        "height: 28px; width: 28px; border-radius: 4px; \
         background: transparent; border: 1px solid transparent; \
         color: {color}; cursor: {cursor}; padding: 0; \
         display: inline-flex; align-items: center; justify-content: center;",
        color = color,
        cursor = if disabled { "not-allowed" } else { "pointer" },
    );
    let sw = if glyph == "play" || glyph == "pause" { 1.8_f32 } else { 1.6_f32 };
    let sz = if glyph == "play" || glyph == "pause" { 14.0_f32 } else { 13.0_f32 };
    rsx! {
        button {
            r#type: "button",
            title: {title.clone()},
            style: {style.clone()},
            onclick: move || onclick.invoke(),
            Icon {
                glyph: {glyph.clone()},
                size: sz,
                stroke: color.to_string(),
                stroke_width: sw,
            }
        }
    }
}

/// Plain icon button (26×26). Used for zoom + gear.
#[component]
fn IconBtn(glyph: String, title: String) -> NodeHandle {
    let color = "rgba(232,234,238,0.62)".to_string();
    let style = format!(
        "height: 26px; width: 26px; border-radius: 4px; \
         background: transparent; border: 1px solid transparent; \
         color: {color}; cursor: pointer; padding: 0; \
         display: inline-flex; align-items: center; justify-content: center;",
    );
    let sz = 14.0_f32;
    let sw = 1.6_f32;
    let stroke = color.clone();
    rsx! {
        button {
            r#type: "button",
            title: {title.clone()},
            style: {style.clone()},
            Icon {
                glyph: {glyph.clone()},
                size: sz,
                stroke: {stroke.clone()},
                stroke_width: sw,
            }
        }
    }
}

// Icon now lives at crate::parts::Icon (shared by every region that needs it).

/// K2 MIDI input picker.
///
/// Reads the available device list (refreshed at render time via
/// [`AudioResources::available_midi_inputs`]) and binds the dropdown's
/// selected value to [`AudioResources::current_midi_device`] reactively.
/// Picking an entry calls `set_midi_device(Some(name))`; picking the
/// "None" sentinel calls `set_midi_device(None)` to disconnect.
///
/// Visual styling is intentionally placeholder per the
/// `project_ui_redesign_pending` memo. The redesign pass replaces
/// chrome, not API.
#[component]
fn MidiPicker() -> NodeHandle {
    let wrap_style = format!(
        "display: flex; align-items: center; gap: 6px; \
         padding: 0 6px 0 9px; border-radius: 4px; \
         background: {bg0}; border: 1px solid {line}; \
         font-size: 12px; color: rgba(232,234,238,0.96);",
        bg0 = theme::BG0,
        line = theme::LINE,
    );
    rsx! {
        div { style: {wrap_style.clone()},
            Tag { text: "MIDI" }
            Select {
                size: "sm",
                value_fn: {|| use_store::<AudioResources>()
                    .current_midi_device
                    .get()
                    .unwrap_or_else(|| MIDI_NONE_VALUE.to_string())
                },
                data: midi_picker_options(),
                onchange: move |v: String| {
                    let audio = use_store::<AudioResources>();
                    if v == MIDI_NONE_VALUE {
                        audio.set_midi_device(None);
                    } else {
                        audio.set_midi_device(Some(&v));
                    }
                },
            }
        }
    }
}

/// Build the option list for [`MidiPicker`]. The "None" sentinel
/// lives first so the dropdown opens with disconnect as the
/// affordance when no device is selected. midir's enumeration runs
/// every render — cheap; one ALSA query.
fn midi_picker_options() -> Vec<SelectOption> {
    let audio = use_store::<AudioResources>();
    let mut out: Vec<SelectOption> =
        vec![SelectOption::new(MIDI_NONE_VALUE, "None")];
    for name in audio.available_midi_inputs() {
        out.push(SelectOption::new(name.clone(), name));
    }
    out
}
