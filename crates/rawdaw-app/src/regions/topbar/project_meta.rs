//! Project-meta controls: the `Project ▾` menu, project name +
//! key/time-sig readouts, BPM controls, and the zoom / gear icon
//! buttons.
//!
//! Composition-writability:
//! - C3 lands the `ProjectMenu` (New / Open / Save / Save As).
//! - C4 promotes name / tempo / key into editable controls and
//!   removes the debug `+1 BPM` placeholder that C2 introduced.

use rinch::prelude::*;
use rinch::core::reactive::{untracked, Effect};

use rawdaw_model::pitch::PitchClass;
use rawdaw_model::project::Project;
use rawdaw_model::scale::{Mode, Scale};
use rawdaw_model::tempo::{BeatUnit, TempoMap};

use crate::audio::AudioResources;
use crate::chord_display::pitch_class_name;
use crate::parts::Icon;
use crate::project_display::current_bpm;
use crate::project_io::{
    apply_loaded_bundle,
    dialog::{pick_open_path, pick_save_path, DialogOutcome},
    load_from_path, save_to_path, SavedBundle,
};
use crate::state::AppState;
use crate::theme;

use super::Tag;

/// Plain icon button (26×26). Used for zoom + gear.
#[component]
pub(super) fn IconBtn(glyph: String, title: String) -> NodeHandle {
    let color = "rgba(232,234,238,0.62)".to_string();
    let style = format!(
        "height: 26px; width: 26px; border-radius: 4px; \
         background: transparent; border: 1px solid transparent; \
         color: {color}; cursor: pointer; padding: 0; \
         display: inline-flex; align-items: center; justify-content: center;",
    );
    let stroke = color.clone();
    rsx! {
        button {
            r#type: "button",
            title: {title.clone()},
            style: {style.clone()},
            Icon {
                glyph: {glyph.clone()},
                size: 14.0,
                stroke: {stroke.clone()},
                stroke_width: 1.6,
            }
        }
    }
}

/// Project default-key picker. 12 pitch classes × {major, minor} for
/// 24 options. Committing re-realizes every Roman-numeral chord
/// under the new key, so round-1's chord ribbon visibly + audibly
/// re-pitches on selection.
///
/// **Scope:** the model's `Scale` supports many modes (dorian,
/// pentatonic, custom, …); this v1 dropdown surfaces only Ionian and
/// Aeolian — major / minor are the two the round-1 demo project and
/// chord-loop editor work in. A project whose default key is some
/// other mode still renders + plays correctly; opening the dropdown
/// just defaults the visible selection to the equivalent major
/// (canonical fallback in [`scale_dropdown_value`]). Picking a value
/// commits over whatever was there before.
#[component]
pub(super) fn KeyDropdown() -> NodeHandle {
    rsx! {
        Select {
            size: "xs",
            value_fn: {|| {
                let project = use_store::<AppState>().project.get();
                scale_dropdown_value(&project.default_key)
            }},
            data: key_dropdown_options(),
            onchange: move |v: String| {
                if let Some(new_scale) = parse_scale_dropdown_value(&v) {
                    let app = use_store::<AppState>();
                    if let Err(e) =
                        app.apply_project_edit(move |p| p.default_key = new_scale.clone())
                    {
                        eprintln!("topbar: key edit failed: {e}");
                    }
                }
            },
        }
    }
}

/// Encode a [`Scale`] as the structured `"<semitones>/<mode>"` value
/// the [`KeyDropdown`] uses. `<mode>` is `"major"` or `"minor"`;
/// modes outside that pair fall back to `"major"` so the dropdown
/// always shows *some* selection rather than rendering a blank
/// state — the user's next pick still commits cleanly.
fn scale_dropdown_value(scale: &Scale) -> String {
    let tonic = scale.tonic.semitones_from_c();
    let mode = match scale.mode {
        Mode::Aeolian => "minor",
        // Every non-Aeolian mode (Ionian + Dorian + …) maps to
        // "major" here. The dropdown is a v1 affordance; future
        // expansion adds the other modes as their own options.
        _ => "major",
    };
    format!("{tonic}/{mode}")
}

/// Inverse of [`scale_dropdown_value`]. Returns `None` on malformed
/// inputs (out-of-range tonic, unknown mode token); callers treat
/// `None` as "no-op" so a malformed dropdown event doesn't replace
/// the project's key with garbage.
fn parse_scale_dropdown_value(s: &str) -> Option<Scale> {
    let (tonic_str, mode_str) = s.split_once('/')?;
    let tonic_idx: i32 = tonic_str.parse().ok()?;
    if !(0..12).contains(&tonic_idx) {
        return None;
    }
    let tonic = PitchClass::from_semitones_mod12(tonic_idx);
    let mode = match mode_str {
        "major" => Mode::Ionian,
        "minor" => Mode::Aeolian,
        _ => return None,
    };
    Some(Scale::new(tonic, mode))
}

/// Build the static option list — 12 pitch classes × {major, minor}
/// in semitone order (C, C#, D, …, B). Each tonic's major / minor
/// pair sits adjacent so the dropdown reads as paired rows.
fn key_dropdown_options() -> Vec<SelectOption> {
    let mut out = Vec::with_capacity(24);
    for tonic_idx in 0..12 {
        let pc = PitchClass::from_semitones_mod12(tonic_idx);
        let pc_name = pitch_class_name(pc);
        out.push(SelectOption::new(
            format!("{tonic_idx}/major"),
            format!("{pc_name} major"),
        ));
        out.push(SelectOption::new(
            format!("{tonic_idx}/minor"),
            format!("{pc_name} minor"),
        ));
    }
    out
}

/// Inline editable project-name field. Reads + writes
/// [`rawdaw_model::project::Project::name`] through the C2 edit pump.
/// Empty / whitespace-only input on Enter reverts to the canonical
/// name rather than committing a blank — the TopBar always has
/// something to render and `Project::new`'s "Untitled" default
/// becomes the floor.
///
/// Style is intentionally placeholder (subtle border so the user
/// notices it's editable) per the `project_ui_redesign_pending`
/// memo. The redesign pass replaces chrome, not the contract.
#[component]
pub(super) fn NameControl() -> NodeHandle {
    let app = use_store::<AppState>();
    let initial = app.project.get().name.clone();
    let name_input = Signal::new(initial);

    let _ = Effect::new(move || {
        let project = use_store::<AppState>().project.get();
        let canonical = project.name.clone();
        let current_display = untracked(|| name_input.get());
        if current_display == canonical {
            return;
        }
        name_input.set(canonical);
    });

    let input_style = format!(
        "min-width: 80px; max-width: 240px; height: 22px; \
         padding: 0 6px; box-sizing: border-box; \
         border-radius: 4px; background: transparent; \
         border: 1px solid {line}; \
         color: rgba(232,234,238,0.96); \
         font-size: 13px; font-weight: 600; letter-spacing: -0.1px;",
        line = theme::LINE,
    );

    rsx! {
        input {
            r#type: "text",
            title: "Project name (Enter to commit)",
            style: {input_style.clone()},
            value: {|| name_input.get()},
            oninput: move |v: String| name_input.set(v),
            onsubmit: move || {
                let typed = name_input.get();
                let trimmed = typed.trim();
                if trimmed.is_empty() {
                    // Reject empty names — revert the input to the
                    // canonical project name so the field always
                    // renders something the user can act on.
                    let canonical =
                        use_store::<AppState>().project.get().name.clone();
                    name_input.set(canonical);
                    return;
                }
                let new_name = trimmed.to_string();
                let app = use_store::<AppState>();
                if let Err(e) = app.apply_project_edit(move |p| p.name = new_name.clone()) {
                    eprintln!("topbar: name edit failed: {e}");
                }
            },
        }
    }
}

/// Lower bound on user-editable BPM. The audio engine doesn't ship a
/// hard rate floor, but clamping here keeps stray inputs (typos,
/// nudge-below-1) from producing nonsense tempo maps and zero-rate
/// playback. The upper bound is intentionally unenforced for now —
/// extreme tempos are valid for some composition styles, and a real
/// ceiling will land alongside the design pass that tightens this
/// surface visually.
const MIN_BPM: f64 = 1.0;

/// Editable tempo control: text input bound to the project BPM plus
/// `−` / `+` nudge buttons. Replaces the C2 `BumpBpmDebug`
/// placeholder. Edits commit through [`AppState::apply_project_edit`]
/// so the engine drains + re-arms in lockstep — same pump path the
/// debug button used, now driven by a real surface.
///
/// The display syncs from the project signal via an [`Effect`] so
/// that external mutations (load, future automation) refresh the
/// input. The Effect reads the input's own `Signal<String>` through
/// [`untracked`] so the user's mid-edit typing isn't a feedback loop
/// — only project-side changes trigger the re-format.
#[component]
pub(super) fn BpmControls() -> NodeHandle {
    let app = use_store::<AppState>();
    let initial = current_bpm(&app.project.get().tempo_map);
    let bpm_input = Signal::new(format_bpm(initial));

    let _ = Effect::new(move || {
        let project = use_store::<AppState>().project.get();
        let canonical = current_bpm(&project.tempo_map);
        let current_display = untracked(|| bpm_input.get());
        // If the user's typing already parses to the canonical
        // numeric value, leave their literal text alone — they may
        // be mid-edit ("120" vs "120.00" are the same value but the
        // user wants to keep typing). Otherwise snap to canonical
        // formatting so loads / nudges show up immediately.
        if let Ok(typed) = current_display.parse::<f64>()
            && (typed - canonical).abs() < f64::EPSILON
        {
            return;
        }
        bpm_input.set(format_bpm(canonical));
    });

    let input_style = format!(
        "width: 64px; height: 22px; padding: 0 6px; box-sizing: border-box; \
         border-radius: 4px; background: {bg0}; border: 1px solid {line}; \
         color: rgba(232,234,238,0.96); font-size: 12px; \
         font-variant-numeric: tabular-nums; \
         font-feature-settings: \"tnum\" 1; text-align: right;",
        bg0 = theme::BG0,
        line = theme::LINE,
    );
    let nudge_minus_style = format!(
        "height: 22px; width: 22px; padding: 0; \
         border-radius: 4px; background: {bg0}; border: 1px solid {line}; \
         color: rgba(232,234,238,0.72); font-size: 13px; cursor: pointer; \
         display: inline-flex; align-items: center; justify-content: center;",
        bg0 = theme::BG0,
        line = theme::LINE,
    );
    let nudge_plus_style = nudge_minus_style.clone();

    rsx! {
        div { style: "display: flex; align-items: center; gap: 4px;",
            button {
                r#type: "button",
                title: "Decrease tempo by 1 BPM",
                style: {nudge_minus_style.clone()},
                onclick: move || nudge_bpm(-1.0),
                "−"
            }
            input {
                r#type: "text",
                style: {input_style.clone()},
                value: {|| bpm_input.get()},
                oninput: move |v: String| bpm_input.set(v),
                onsubmit: move || {
                    let raw = bpm_input.get();
                    if let Some(bpm) = parse_bpm(&raw) {
                        commit_bpm(bpm);
                        bpm_input.set(format_bpm(bpm));
                    }
                },
            }
            Tag { text: "BPM" }
            button {
                r#type: "button",
                title: "Increase tempo by 1 BPM",
                style: {nudge_plus_style.clone()},
                onclick: move || nudge_bpm(1.0),
                "+"
            }
        }
    }
}

/// Canonical numeric formatting for the BPM input. Two decimal places
/// so a fractional value entered via future automation surfaces still
/// renders precisely; integer entries display as "120.00" after
/// commit, which is the convention the C2 read-only readout used.
fn format_bpm(bpm: f64) -> String {
    format!("{bpm:.2}")
}

/// Parse + clamp a BPM string. Returns `None` for empty / unparseable
/// input so the caller can leave the project untouched. Clamps below
/// to [`MIN_BPM`]; no upper clamp yet ([see `MIN_BPM` doc-comment]).
fn parse_bpm(s: &str) -> Option<f64> {
    let parsed: f64 = s.trim().parse().ok()?;
    if !parsed.is_finite() {
        return None;
    }
    Some(parsed.max(MIN_BPM))
}

/// Apply `delta` to the project's current BPM and commit. Floors at
/// [`MIN_BPM`] so the `−` button can't drag the tempo into nonsense
/// territory.
fn nudge_bpm(delta: f64) {
    let app = use_store::<AppState>();
    let current = current_bpm(&app.project.get().tempo_map);
    let new_bpm = (current + delta).max(MIN_BPM);
    commit_bpm(new_bpm);
}

/// Replace the project's tempo map with a constant-BPM map at the
/// requested rate, preserving the existing time signature. Routes
/// through [`AppState::apply_project_edit`] so the engine drains its
/// song queue and re-arms in lockstep with the host signal swap.
fn commit_bpm(bpm: f64) {
    let app = use_store::<AppState>();
    let edit = move |project: &mut Project| {
        let ts = project.tempo_map.time_signature_events.first().copied();
        let beats_per_bar = ts.map(|e| e.beats_per_bar).unwrap_or(4);
        let beat_unit = ts.map(|e| e.beat_unit).unwrap_or(BeatUnit::Quarter);
        project.tempo_map = TempoMap::constant(bpm, beats_per_bar, beat_unit);
    };
    if let Err(e) = app.apply_project_edit(edit) {
        eprintln!("topbar: BPM edit failed: {e}");
    }
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
pub(super) fn ProjectMenu() -> NodeHandle {
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
        DropdownMenu {
            opened_fn: move || opened.get(),
            on_close: move || opened.set(false),
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
/// project's `name` field so a fresh project saves as
/// `<project name>.rawd`.
fn save_as_action(pending_save: Signal<Option<DialogOutcome>>) {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let name = if project.name.is_empty() {
        "Untitled".to_string()
    } else {
        format!("{}.rawd", sanitize_filename(&project.name))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bpm_accepts_floats_and_integers() {
        assert_eq!(parse_bpm("120"), Some(120.0));
        assert_eq!(parse_bpm("120.5"), Some(120.5));
        // Leading/trailing whitespace is forgiving — the input widget
        // sometimes accumulates space from copy/paste.
        assert_eq!(parse_bpm("  140  "), Some(140.0));
    }

    #[test]
    fn parse_bpm_clamps_below_to_min() {
        // Sub-MIN_BPM values clamp up rather than committing nonsense.
        // Negative values too — they parse but get clamped.
        assert_eq!(parse_bpm("0.5"), Some(MIN_BPM));
        assert_eq!(parse_bpm("-10"), Some(MIN_BPM));
    }

    #[test]
    fn parse_bpm_rejects_garbage_and_non_finite() {
        assert_eq!(parse_bpm(""), None);
        assert_eq!(parse_bpm("abc"), None);
        assert_eq!(parse_bpm("inf"), None);
        assert_eq!(parse_bpm("NaN"), None);
    }

    #[test]
    fn format_bpm_uses_two_decimal_places() {
        // Pins the canonical formatting the readout snaps to after a
        // commit, so the Effect's "no clobber if equal" check uses
        // the same string the user will see post-Enter.
        assert_eq!(format_bpm(120.0), "120.00");
        assert_eq!(format_bpm(120.5), "120.50");
    }

    #[test]
    fn sanitize_filename_replaces_unsafe_characters() {
        assert_eq!(sanitize_filename("foo/bar"), "foo_bar");
        assert_eq!(sanitize_filename("a:b*c?"), "a_b_c_");
        assert_eq!(sanitize_filename("clean name 1"), "clean name 1");
    }

    #[test]
    fn scale_dropdown_value_encodes_major_and_minor() {
        // Ionian + Aeolian (the two modes the dropdown surfaces) map
        // to "major" + "minor" tokens. Tonic encodes as semitones
        // from C so the value is stable across human-readable naming
        // changes.
        assert_eq!(
            scale_dropdown_value(&Scale::major(PitchClass::C)),
            "0/major",
        );
        assert_eq!(
            scale_dropdown_value(&Scale::natural_minor(PitchClass::A)),
            "9/minor",
        );
    }

    #[test]
    fn scale_dropdown_value_falls_back_to_major_for_unsupported_modes() {
        // Dorian / pentatonic / custom — every non-Aeolian mode
        // surfaces as "major" so the dropdown always paints SOMETHING
        // until the user picks. Picking commits over whatever was
        // there before.
        assert_eq!(
            scale_dropdown_value(&Scale::new(PitchClass::D, Mode::Dorian)),
            "2/major",
        );
        assert_eq!(
            scale_dropdown_value(&Scale::new(PitchClass::G, Mode::MajorPentatonic)),
            "7/major",
        );
    }

    #[test]
    fn parse_scale_dropdown_value_inverts_scale_dropdown_value() {
        // Round-trip every option in the dropdown so no garbage
        // value can sneak past the parser.
        for tonic_idx in 0..12_i32 {
            for mode in [Mode::Ionian, Mode::Aeolian] {
                let pc = PitchClass::from_semitones_mod12(tonic_idx);
                let scale = Scale::new(pc, mode.clone());
                let v = scale_dropdown_value(&scale);
                assert_eq!(
                    parse_scale_dropdown_value(&v),
                    Some(scale.clone()),
                    "round-trip mismatch for {v}",
                );
            }
        }
    }

    #[test]
    fn parse_scale_dropdown_value_rejects_malformed_inputs() {
        // Out-of-range tonic, unknown mode, missing separator — all
        // surface as None so an external dropdown event can't wipe
        // the project's key with garbage.
        assert_eq!(parse_scale_dropdown_value(""), None);
        assert_eq!(parse_scale_dropdown_value("0"), None);
        assert_eq!(parse_scale_dropdown_value("12/major"), None);
        assert_eq!(parse_scale_dropdown_value("-1/minor"), None);
        assert_eq!(parse_scale_dropdown_value("0/dorian"), None);
    }

    #[test]
    fn key_dropdown_options_lists_all_24_combinations() {
        // Pins the order: paired (major, minor) per tonic, ascending
        // by semitone. Anything that resorts these has to update the
        // dropdown's affordance too.
        let opts = key_dropdown_options();
        assert_eq!(opts.len(), 24);
        // C major is first; C minor is second.
        assert_eq!(opts[0].value, "0/major");
        assert_eq!(opts[1].value, "0/minor");
        // B minor is last.
        assert_eq!(opts[23].value, "11/minor");
    }
}
