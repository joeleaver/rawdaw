//! Octave-spec sub-row shared by the Scale and Chord sub-editors.
//!
//! Mounts a `Select` for the `OctaveSpec` discriminator
//! (Nearest / Anchored / UpFromPrev / DownFromPrev / RelativeToRole)
//! plus a conditional anchor-octave picker that only renders when
//! the current spec is `Anchored(_)`. The picker is wrapped in a
//! keyed for-loop so toggling between Anchored and the other shapes
//! remounts cleanly.

use rinch::prelude::*;

use rawdaw_model::id::{NoteId, PatternId};
use rawdaw_model::pattern::{OctaveSpec, PitchSpec};
use rawdaw_model::pitch::Octave;

use crate::pattern_actions::update_pitched_event;
use crate::regions::pattern_editor::pitched::helpers::DEFAULT_ANCHOR_OCTAVE;
use crate::state::AppState;

use super::{current_variant, fetch_event};

#[component]
pub(super) fn OctaveRow(pattern_id: PatternId, note_id_value: u64) -> NodeHandle {
    let note_id = NoteId::new(note_id_value);
    let options = octave_spec_options();
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 6px;",
            Select {
                size: "sm",
                value_fn: move || match fetch_event(pattern_id, note_id) {
                    Some(ev) => octave_spec_kind_str(&ev.spec).to_string(),
                    None => "nearest".to_string(),
                },
                data: options,
                onchange: move |v: String| commit_octave_kind(pattern_id, note_id, v),
            }
            for show in octave_anchored_show_keys(pattern_id, note_id) {
                AnchoredOctavePickerWrapper {
                    key: show.clone(),
                    pattern_id: pattern_id,
                    note_id_value: note_id_value,
                    show: show.clone(),
                }
            }
        }
    }
}

fn octave_anchored_show_keys(pattern_id: PatternId, note_id: NoteId) -> Vec<String> {
    let event = fetch_event(pattern_id, note_id);
    let show = matches!(
        event.as_ref().map(|e| &e.spec),
        Some(PitchSpec::Scale {
            octave: OctaveSpec::Anchored(_),
            ..
        }) | Some(PitchSpec::Chord {
            octave: OctaveSpec::Anchored(_),
            ..
        }),
    );
    vec![if show { "y" } else { "n" }.to_string()]
}

#[component]
fn AnchoredOctavePickerWrapper(
    pattern_id: PatternId,
    note_id_value: u64,
    show: String,
) -> NodeHandle {
    if show != "y" {
        return rsx! { div { } };
    }
    let note_id = NoteId::new(note_id_value);
    let options: Vec<SelectOption> = (0..=8i8)
        .map(|o| SelectOption::new(o.to_string(), format!("{o}")))
        .collect();
    rsx! {
        Select {
            size: "sm",
            value_fn: move || {
                match fetch_event(pattern_id, note_id) {
                    Some(ev) => anchored_octave_for_spec(&ev.spec)
                        .unwrap_or(DEFAULT_ANCHOR_OCTAVE)
                        .0
                        .to_string(),
                    None => DEFAULT_ANCHOR_OCTAVE.0.to_string(),
                }
            },
            data: options,
            onchange: move |v: String| commit_anchored_octave(pattern_id, note_id, v),
        }
    }
}

fn anchored_octave_for_spec(spec: &PitchSpec) -> Option<Octave> {
    match spec {
        PitchSpec::Scale {
            octave: OctaveSpec::Anchored(o),
            ..
        }
        | PitchSpec::Chord {
            octave: OctaveSpec::Anchored(o),
            ..
        } => Some(*o),
        _ => None,
    }
}

fn commit_octave_kind(pattern_id: PatternId, note_id: NoteId, value: String) {
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            let next_octave = match value.as_str() {
                "nearest" => OctaveSpec::Nearest,
                "anchored" => OctaveSpec::Anchored(
                    anchored_octave_for_spec(&ev.spec).unwrap_or(DEFAULT_ANCHOR_OCTAVE),
                ),
                "up" => OctaveSpec::UpFromPrev,
                "down" => OctaveSpec::DownFromPrev,
                "role" => OctaveSpec::RelativeToRole,
                _ => return,
            };
            match &mut ev.spec {
                PitchSpec::Scale { octave, .. } | PitchSpec::Chord { octave, .. } => {
                    *octave = next_octave;
                }
                _ => {}
            }
        });
    }) {
        eprintln!("pattern_editor: set octave kind failed: {e}");
    }
}

fn commit_anchored_octave(pattern_id: PatternId, note_id: NoteId, value: String) {
    let Ok(o) = value.trim().parse::<i8>() else { return };
    let app = use_store::<AppState>();
    let variant = current_variant(pattern_id);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        update_pitched_event(p, pattern_id, &variant_for_edit, note_id, |ev| {
            match &mut ev.spec {
                PitchSpec::Scale {
                    octave: OctaveSpec::Anchored(existing),
                    ..
                }
                | PitchSpec::Chord {
                    octave: OctaveSpec::Anchored(existing),
                    ..
                } => {
                    *existing = Octave(o);
                }
                _ => {}
            }
        });
    }) {
        eprintln!("pattern_editor: set anchored octave failed: {e}");
    }
}

fn octave_spec_kind_str(spec: &PitchSpec) -> &'static str {
    match spec {
        PitchSpec::Scale { octave, .. } | PitchSpec::Chord { octave, .. } => match octave {
            OctaveSpec::Nearest => "nearest",
            OctaveSpec::Anchored(_) => "anchored",
            OctaveSpec::UpFromPrev => "up",
            OctaveSpec::DownFromPrev => "down",
            OctaveSpec::RelativeToRole => "role",
        },
        _ => "nearest",
    }
}

fn octave_spec_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("nearest", "Nearest"),
        SelectOption::new("anchored", "Anchored"),
        SelectOption::new("up", "Up from previous"),
        SelectOption::new("down", "Down from previous"),
        SelectOption::new("role", "Track role default"),
    ]
}
