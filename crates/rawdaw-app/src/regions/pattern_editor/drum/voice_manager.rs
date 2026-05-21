//! Voice-list management header for the drum editor.
//!
//! Renders the pattern's current voices as short-label chips with a
//! `×` button each (calls `remove_drum_voice` — refused for the last
//! voice), plus an "Add voice" `Select` dropdown that lists the fixed
//! voices not yet present. `Extra(_)` voice creation is deferred to a
//! follow-up bite (would need a text-input affordance).

use rinch::prelude::*;

use rawdaw_model::id::PatternId;
use rawdaw_model::pattern::{DrumVoice, PatternBody};

use crate::pattern_actions::{add_drum_voice, remove_drum_voice, VoiceEditError};
use crate::state::AppState;
use crate::theme;

use super::helpers::{voice_full_name, voice_short_label};

const ADD_VOICE_PLACEHOLDER: &str = "__add_voice__";

#[component]
pub(super) fn VoiceManager(id: PatternId) -> NodeHandle {
    let bar_style = format!(
        "display: flex; align-items: center; gap: 6px; flex-wrap: wrap; \
         padding: 4px 8px; border: 1px solid {line}; border-radius: 4px; \
         background: {bg1};",
        line = theme::LINE,
        bg1 = theme::BG1,
    );
    rsx! {
        div { style: {bar_style.clone()},
            span {
                style: "font-size: 10px; letter-spacing: 0.6px; \
                        text-transform: uppercase; \
                        color: rgba(232,234,238,0.42); font-weight: 600;",
                "Voices"
            }
            for chip in build_voice_chips(id) {
                VoiceChip {
                    key: chip.value_key.clone(),
                    pattern_id: id,
                    voice: chip.voice.clone(),
                    short: chip.short.clone(),
                    full: chip.full.clone(),
                }
            }
            for opts in add_voice_options(id) {
                Select {
                    key: opts.key.clone(),
                    size: "sm",
                    value: ADD_VOICE_PLACEHOLDER.to_string(),
                    data: opts.options.clone(),
                    onchange: move |v: String| handle_add_voice(id, v),
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Default)]
struct VoiceChipModel {
    voice: DrumVoice,
    short: String,
    full: String,
    value_key: String,
}

fn build_voice_chips(id: PatternId) -> Vec<VoiceChipModel> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(pattern) = project.patterns.get(&id) else {
        return Vec::new();
    };
    let body = match &pattern.body {
        PatternBody::Drum(b) => b,
        PatternBody::Pitched(_) => return Vec::new(),
    };
    body.metadata
        .voices
        .iter()
        .map(|v| VoiceChipModel {
            voice: v.clone(),
            short: voice_short_label(v),
            full: voice_full_name(v),
            value_key: format!("chip-{}", voice_short_label(v)),
        })
        .collect()
}

#[component]
fn VoiceChip(
    pattern_id: PatternId,
    voice: DrumVoice,
    short: String,
    full: String,
) -> NodeHandle {
    let chip_style = format!(
        "display: inline-flex; align-items: center; gap: 4px; \
         padding: 2px 4px 2px 8px; \
         border: 1px solid {line}; border-radius: 3px; \
         background: {bg0}; height: 22px;",
        line = theme::LINE,
        bg0 = theme::BG0,
    );
    let label_style = "font-size: 11px; font-weight: 600; \
                       color: rgba(232,234,238,0.85);";
    let remove_btn_style = "height: 18px; width: 18px; padding: 0; \
         border-radius: 3px; background: transparent; \
         border: 1px solid transparent; color: rgba(232,234,238,0.55); \
         font-size: 10px; cursor: pointer; \
         display: inline-flex; align-items: center; justify-content: center;"
        .to_string();
    let voice_for_click = voice.clone();
    rsx! {
        div { style: {chip_style.clone()}, title: {full.clone()},
            span { style: label_style, {short.clone()} }
            button {
                r#type: "button",
                title: "Remove voice (purges its events)",
                style: {remove_btn_style.clone()},
                onclick: move || handle_remove_voice(pattern_id, voice_for_click.clone()),
                "×"
            }
        }
    }
}

#[derive(Clone, PartialEq, Default)]
struct AddVoiceOptions {
    key: String,
    options: Vec<SelectOption>,
}

fn add_voice_options(id: PatternId) -> Vec<AddVoiceOptions> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(pattern) = project.patterns.get(&id) else {
        return Vec::new();
    };
    let body = match &pattern.body {
        PatternBody::Drum(b) => b,
        PatternBody::Pitched(_) => return Vec::new(),
    };
    let present: std::collections::BTreeSet<String> = body
        .metadata
        .voices
        .iter()
        .map(voice_value_key)
        .collect();
    let mut options = vec![SelectOption::new(
        ADD_VOICE_PLACEHOLDER.to_string(),
        "+ Add voice".to_string(),
    )];
    for v in candidate_voices() {
        let key = voice_value_key(&v);
        if present.contains(&key) {
            continue;
        }
        options.push(SelectOption::new(key, voice_full_name(&v)));
    }
    // Key encodes the option set so add/remove/rename remounts the
    // dropdown with the fresh list.
    let voices_key: String = body
        .metadata
        .voices
        .iter()
        .map(voice_short_label)
        .collect::<Vec<_>>()
        .join(":");
    vec![AddVoiceOptions {
        key: format!("add-{voices_key}"),
        options,
    }]
}

/// The 14 fixed voices presented in the add-voice dropdown. `Extra(_)`
/// creation requires a text input (deferred to a polish bite).
fn candidate_voices() -> Vec<DrumVoice> {
    vec![
        DrumVoice::Kick,
        DrumVoice::Snare,
        DrumVoice::SnareRim,
        DrumVoice::ClosedHat,
        DrumVoice::OpenHat,
        DrumVoice::PedalHat,
        DrumVoice::TomLow,
        DrumVoice::TomMid,
        DrumVoice::TomHigh,
        DrumVoice::Crash,
        DrumVoice::Ride,
        DrumVoice::RideBell,
        DrumVoice::Clap,
        DrumVoice::Cowbell,
    ]
}

fn voice_value_key(voice: &DrumVoice) -> String {
    match voice {
        DrumVoice::Kick => "Kick".into(),
        DrumVoice::Snare => "Snare".into(),
        DrumVoice::SnareRim => "SnareRim".into(),
        DrumVoice::ClosedHat => "ClosedHat".into(),
        DrumVoice::OpenHat => "OpenHat".into(),
        DrumVoice::PedalHat => "PedalHat".into(),
        DrumVoice::TomLow => "TomLow".into(),
        DrumVoice::TomMid => "TomMid".into(),
        DrumVoice::TomHigh => "TomHigh".into(),
        DrumVoice::Crash => "Crash".into(),
        DrumVoice::Ride => "Ride".into(),
        DrumVoice::RideBell => "RideBell".into(),
        DrumVoice::Clap => "Clap".into(),
        DrumVoice::Cowbell => "Cowbell".into(),
        DrumVoice::Extra(name) => format!("x:{name}"),
    }
}

fn voice_from_value_key(s: &str) -> Option<DrumVoice> {
    Some(match s {
        "Kick" => DrumVoice::Kick,
        "Snare" => DrumVoice::Snare,
        "SnareRim" => DrumVoice::SnareRim,
        "ClosedHat" => DrumVoice::ClosedHat,
        "OpenHat" => DrumVoice::OpenHat,
        "PedalHat" => DrumVoice::PedalHat,
        "TomLow" => DrumVoice::TomLow,
        "TomMid" => DrumVoice::TomMid,
        "TomHigh" => DrumVoice::TomHigh,
        "Crash" => DrumVoice::Crash,
        "Ride" => DrumVoice::Ride,
        "RideBell" => DrumVoice::RideBell,
        "Clap" => DrumVoice::Clap,
        "Cowbell" => DrumVoice::Cowbell,
        s if s.starts_with("x:") => DrumVoice::Extra(s[2..].to_string()),
        _ => return None,
    })
}

fn handle_add_voice(id: PatternId, value_key: String) {
    if value_key == ADD_VOICE_PLACEHOLDER {
        return;
    }
    let Some(voice) = voice_from_value_key(&value_key) else { return };
    let app = use_store::<AppState>();
    if let Err(e) = app.apply_project_edit(move |p| {
        add_drum_voice(p, id, voice.clone());
    }) {
        eprintln!("drum_editor: add voice failed: {e}");
    }
}

fn handle_remove_voice(id: PatternId, voice: DrumVoice) {
    let app = use_store::<AppState>();
    let outcome: std::rc::Rc<std::cell::RefCell<Result<(), VoiceEditError>>> =
        std::rc::Rc::new(std::cell::RefCell::new(Ok(())));
    let outcome_capture = std::rc::Rc::clone(&outcome);
    let voice_for_edit = voice.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        let res = remove_drum_voice(p, id, &voice_for_edit);
        *outcome_capture.borrow_mut() = res;
    }) {
        eprintln!("drum_editor: remove voice edit failed: {e}");
        return;
    }
    let taken =
        std::mem::replace(&mut *outcome.borrow_mut(), Ok::<_, VoiceEditError>(()));
    if let Err(err) = taken {
        eprintln!("drum_editor: remove voice refused: {err:?}");
    }
}
