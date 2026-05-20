//! Top bar — project identity (left) · transport (center) · tempo/zoom (right).
//!
//! Translates `docs/design/mockups/round-1/components/topbar.jsx`.
//! The chrome row sits at `theme::BG1` so it lifts off the arrangement
//! (`theme::BG0`); the inset readouts inside use `theme::BG0` to look
//! recessed against the lighter chrome — same trick as the mockup.

use rinch::prelude::*;
use rinch::core::reactive::Effect;

use crate::audio::AudioResources;
use crate::parts::Icon;
use crate::project_display::{current_bpm, scale_label, time_signature_label};
use crate::project_io::{
    apply_loaded_bundle,
    dialog::{pick_open_path, pick_save_path, DialogOutcome},
    load_from_path, save_to_path, SavedBundle,
};
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

    // Project name + scale/time-signature meta: read once at component
    // build. These are stable across the C2 debug edit (which only
    // touches tempo); the C4 controls will make them reactive when
    // they grow real edit affordances.
    let project_name = overlay.project_name.clone();
    let project_meta = format!(
        "{} · {}",
        scale_label(&project.default_key),
        time_signature_label(&project.tempo_map),
    );

    rsx! {
        div { style: {bar_style.clone()},
            // Left: Project menu + identity
            div {
                style: "display: flex; align-items: center; gap: 10px; min-width: 0;",
                ProjectMenu { }
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
                        // Reactive: subscribes to the AppState project
                        // signal so C2's debug +1 BPM button (and the
                        // real C4 tempo control later) updates the
                        // readout in lockstep with the audio engine.
                        {|| {
                            let project = use_store::<AppState>().project.get();
                            format!("{:.2}", current_bpm(&project.tempo_map))
                        }}
                    }
                    Tag { text: "BPM" }
                }
                // C2 debug-only: bump the project tempo by 1 BPM and
                // re-arm the audio engine end-to-end via the edit
                // pump. Proof-of-life for the
                // edit → re-realize → audio re-arm path. The
                // component body is cfg-gated so release builds
                // render an empty marker; the unconditional call
                // site keeps rsx happy (it can't take a `#[cfg]`
                // attribute on a child). Replaced by the real
                // editable BPM control in C4 (see
                // `docs/composition-writability-plan.md`).
                BumpBpmDebug { }
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

/// Debug-only "+1 BPM" button. Bumps the project tempo by one BPM
/// and pumps the change through [`AppState::apply_project_edit`]
/// — the C2 proof-of-life that the edit pipeline reaches the audio
/// engine end-to-end. Replaced in C4 by the real editable tempo
/// control (`docs/composition-writability-plan.md`).
///
/// Body cfg-gated to `debug_assertions` — release builds render
/// nothing visible (an empty span). The unconditional call site in
/// the rsx tree keeps the macro happy (rsx children can't carry a
/// `#[cfg]` attribute) and a release build's dead-code pass elides
/// the empty span. Errors print to stderr — the engine isn't
/// expected to fail re-arm in normal use, and the button is a
/// developer tool.
#[component]
fn BumpBpmDebug() -> NodeHandle {
    #[cfg(debug_assertions)]
    {
        let style = format!(
            "height: 22px; min-width: 30px; padding: 0 8px; \
             border-radius: 4px; background: {bg0}; border: 1px solid {line}; \
             color: rgba(232,234,238,0.72); font-size: 11px; \
             font-feature-settings: \"tnum\" 1; cursor: pointer;",
            bg0 = theme::BG0,
            line = theme::LINE,
        );
        return rsx! {
            button {
                r#type: "button",
                title: "Debug: bump project tempo by 1 BPM (composition-writability C2)",
                style: {style.clone()},
                onclick: move || {
                    let app = use_store::<AppState>();
                    if let Err(e) = app.apply_project_edit(bump_bpm_by_one) {
                        eprintln!("topbar: +1 BPM edit failed: {e}");
                    }
                },
                "+1"
            }
        };
    }
    #[cfg(not(debug_assertions))]
    rsx! { span { style: "display: none;" } }
}

/// Mutation closure used by [`BumpBpmDebug`]. Replaces the project's
/// tempo map with a constant-BPM map at `current + 1`, preserving
/// the existing time signature. Free-standing so any future test
/// that exercises the same edit pump path can call this directly.
#[cfg(debug_assertions)]
fn bump_bpm_by_one(project: &mut rawdaw_model::project::Project) {
    use rawdaw_model::tempo::{BeatUnit, TempoMap};
    let current_bpm = project
        .tempo_map
        .bpm_events
        .first()
        .map(|e| e.bpm)
        .unwrap_or(120.0);
    let ts = project
        .tempo_map
        .time_signature_events
        .first()
        .copied();
    let beats_per_bar = ts.map(|e| e.beats_per_bar).unwrap_or(4);
    let beat_unit = ts.map(|e| e.beat_unit).unwrap_or(BeatUnit::Quarter);
    project.tempo_map = TempoMap::constant(current_bpm + 1.0, beats_per_bar, beat_unit);
}

// ─── C3 Project ▾ menu ────────────────────────────────────────────────────

/// `Project ▾` dropdown — `New`, `Open…`, `Save`, `Save As…`.
///
/// Composition-writability C3. Each item drives one project-IO
/// path; the dialog flows run on background `std::thread`s (rfd
/// blocks) and feed back to the UI via cross-thread `Signal::send`.
/// Two Effects inside the component body observe the
/// `pending_open` / `pending_save` signals and dispatch the
/// matching load / save once the dialog closes.
#[component]
fn ProjectMenu() -> NodeHandle {
    // Local-to-component reactive state — the dropdown's open flag
    // and the dialog outcome channels. `Signal::new` returns Copy,
    // so the closures below freely close over each handle.
    let opened = Signal::new(false);
    let pending_open: Signal<Option<DialogOutcome>> = Signal::new(None);
    let pending_save: Signal<Option<DialogOutcome>> = Signal::new(None);

    // When `pending_open` becomes Some(Picked(path)), load the
    // file and apply it through the C2 edit pump. Cancellation
    // resets the signal silently. Errors go to stderr — when the
    // round-1 toast primitive lands we'll wire a user-facing alert.
    let _ = Effect::new(move || {
        let outcome = pending_open.get();
        if let Some(outcome) = outcome {
            pending_open.set(None);
            match outcome {
                DialogOutcome::Picked(path) => {
                    let app = use_store::<AppState>();
                    let audio = use_store::<AudioResources>();
                    match load_from_path(&path) {
                        Ok(bundle) => {
                            if let Err(e) = apply_loaded_bundle(&app, &audio, bundle) {
                                eprintln!("project: load apply failed: {e}");
                            } else {
                                app.current_path.set(Some(path));
                            }
                        }
                        Err(e) => eprintln!("project: load failed: {e}"),
                    }
                }
                DialogOutcome::Cancelled => {}
            }
        }
    });

    // Symmetric save-as outcome handler: write the bundle to the
    // picked path and remember it as the new current path so the
    // next plain "Save" goes back to the same file.
    let _ = Effect::new(move || {
        let outcome = pending_save.get();
        if let Some(outcome) = outcome {
            pending_save.set(None);
            match outcome {
                DialogOutcome::Picked(path) => {
                    let app = use_store::<AppState>();
                    let project = (*app.project.get()).clone();
                    let overlay = (*app.overlay.get()).clone();
                    let bundle = SavedBundle::new(project, overlay);
                    match save_to_path(&bundle, &path) {
                        Ok(()) => app.current_path.set(Some(path)),
                        Err(e) => eprintln!("project: save failed: {e}"),
                    }
                }
                DialogOutcome::Cancelled => {}
            }
        }
    });

    let target_btn_style = format!(
        "height: 24px; padding: 0 8px; \
         border-radius: 4px; background: {bg0}; border: 1px solid {line}; \
         color: rgba(232,234,238,0.96); font-size: 12px; cursor: pointer; \
         display: inline-flex; align-items: center; gap: 4px;",
        bg0 = theme::BG0,
        line = theme::LINE,
    );

    rsx! {
        DropdownMenu { opened_fn: move || opened.get(),
            DropdownMenuTarget {
                button {
                    r#type: "button",
                    title: "Project menu",
                    style: {target_btn_style.clone()},
                    onclick: move || opened.update(|v| *v = !*v),
                    "Project ▾"
                }
            }
            DropdownMenuDropdown {
                DropdownMenuItem {
                    onclick: move || {
                        new_project_action();
                        opened.set(false);
                    },
                    "New"
                }
                DropdownMenuItem {
                    onclick: move || {
                        pick_open_path(pending_open);
                        opened.set(false);
                    },
                    "Open…"
                }
                DropdownMenuDivider {}
                DropdownMenuItem {
                    onclick: move || {
                        save_action(pending_save);
                        opened.set(false);
                    },
                    "Save"
                }
                DropdownMenuItem {
                    onclick: move || {
                        save_as_action(pending_save);
                        opened.set(false);
                    },
                    "Save As…"
                }
            }
        }
    }
}

/// "New" — replace the live project with a minimum-sensible empty
/// project. C3 ships the placeholder (a blank C-major Project);
/// C4's plan defines the real minimum sensible empty project
/// (one Pitched track, one Untitled section, one arrangement
/// block). Routes through the edit pump so the engine drains its
/// song queue and re-arms against the empty event stream.
///
/// Errors go to stderr; the C3 plan doesn't promise an alert UI.
fn new_project_action() {
    use rawdaw_model::pitch::PitchClass;
    use rawdaw_model::project::Project;
    use rawdaw_model::scale::Scale;
    let app = use_store::<AppState>();
    if let Err(e) =
        app.apply_project_edit(|p| *p = Project::new(Scale::major(PitchClass::C)))
    {
        eprintln!("project: New failed: {e}");
        return;
    }
    // Empty overlay + clear the path so the next Save prompts.
    app.overlay
        .set(std::rc::Rc::new(crate::overlay::ProjectOverlay::default()));
    app.current_path.set(None);
}

/// "Save" — write the live bundle to the current path. Falls
/// through to [`save_as_action`] when no path is known yet
/// (fresh boot or post-New). The fall-through is the plan's
/// `Save with no open path falls through to Save As` contract.
fn save_action(pending_save: Signal<Option<DialogOutcome>>) {
    let app = use_store::<AppState>();
    let Some(path) = app.current_path.get() else {
        save_as_action(pending_save);
        return;
    };
    let project = (*app.project.get()).clone();
    let overlay = (*app.overlay.get()).clone();
    let bundle = SavedBundle::new(project, overlay);
    if let Err(e) = save_to_path(&bundle, &path) {
        eprintln!("project: save failed: {e}");
    }
}

/// "Save As…" — prompt for a path. Default filename seeds from the
/// overlay's `project_name` so a fresh project saves as
/// `<project name>.rawd`.
fn save_as_action(pending_save: Signal<Option<DialogOutcome>>) {
    let app = use_store::<AppState>();
    let overlay = app.overlay.get();
    let name = if overlay.project_name.is_empty() {
        "Untitled".to_string()
    } else {
        format!("{}.rawd", sanitize_filename(&overlay.project_name))
    };
    pick_save_path(Some(&name), pending_save);
}

/// Strip filesystem-hostile characters from a project name so it
/// can seed the Save As default filename. Conservative — replaces
/// `/`, `\`, and the platform path separators with `_`; leaves
/// spaces, dashes, and unicode letters alone since modern OSes
/// handle those fine.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}
