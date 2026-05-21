//! Voice selector for the focused drum event.
//!
//! The dropdown options are the pattern's voice list (read reactively
//! from the project). Swapping an event's voice reassigns its row on
//! the step grid — the model just rewrites the field, the grid
//! re-derives the row mapping on its next read.

use rinch::prelude::*;

use rawdaw_model::id::{NoteId, PatternId};
use rawdaw_model::pattern::{DrumVoice, PatternBody};

use crate::pattern_actions::update_drum_event;
use crate::regions::pattern_editor::drum::helpers::{voice_full_name, voice_short_label};
use crate::state::AppState;

use super::{current_variant, fetch_event, field_group_style, field_label_style};

#[component]
pub(super) fn VoiceSelectorGroup(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    let note_id = NoteId::new(note_id_value);
    rsx! {
        div { style: {field_group_style()},
            span { style: {field_label_style()}, "Voice" }
            for opts in voice_options(pattern_id, note_id) {
                Select {
                    key: opts.key.clone(),
                    size: "sm",
                    value: {opts.current_value.clone()},
                    data: opts.options.clone(),
                    onchange: move |v: String| commit_voice(pattern_id, note_id, v),
                }
            }
        }
    }
}

/// Per-render options bag. Wrapped in a `Vec<>` so the parent rsx
/// for-loop can key on `key` — when the pattern's voice list changes,
/// the Select remounts with the new option list rather than holding
/// stale entries.
#[derive(Clone, PartialEq, Default)]
struct VoiceOptions {
    key: String,
    current_value: String,
    options: Vec<SelectOption>,
}

fn voice_options(pattern_id: PatternId, note_id: NoteId) -> Vec<VoiceOptions> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(pattern) = project.patterns.get(&pattern_id) else {
        return Vec::new();
    };
    let body = match &pattern.body {
        PatternBody::Drum(b) => b,
        PatternBody::Pitched(_) => return Vec::new(),
    };
    let current = fetch_event(pattern_id, note_id).map(|e| e.voice);
    let options: Vec<SelectOption> = body
        .metadata
        .voices
        .iter()
        .map(|v| SelectOption::new(voice_value_key(v), voice_full_name(v)))
        .collect();
    let current_value = current
        .as_ref()
        .map(voice_value_key)
        .unwrap_or_default();
    // Key includes the voice-list snapshot so changes to the list (add /
    // remove / rename) remount the Select with fresh option data.
    let voices_key: String = body
        .metadata
        .voices
        .iter()
        .map(voice_short_label)
        .collect::<Vec<_>>()
        .join(":");
    let key = format!("v{}-{}-{voices_key}", note_id.get(), current_value);
    vec![VoiceOptions {
        key,
        current_value,
        options,
    }]
}

/// Stringly-keyed identity for a `DrumVoice`. The fixed voices use
/// their `Debug` repr (`"Kick"`, `"Snare"`, …); extras use `"x:<name>"`
/// so the key namespace stays disjoint from the fixed names.
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

fn commit_voice(pattern_id: PatternId, note_id: NoteId, value: String) {
    let Some(voice) = voice_from_value_key(&value) else { return };
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_drum_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            ev.voice = voice.clone();
        });
    }) {
        eprintln!("drum_editor: set voice failed: {e}");
    }
}
