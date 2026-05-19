//! Synth editor dispatch. Reads the project track's
//! [`SynthAssignment`] and renders the matching per-synth editor
//! placeholder. U5 fills in the wavetable controls; U7 fills in the
//! drum controls.
//!
//! Pulled out of `inspector/mod.rs` because the dispatch (and the
//! per-synth editor sub-components that follow in U5/U7) is its own
//! concern — keeping it here lets the inspector module stay close to
//! the 700-line cap while the synth editors grow.

use rinch::prelude::*;

use rawdaw_model::patch::SynthAssignment;

use crate::audio::AudioResources;
use crate::theme;

/// Dispatches to the per-synth editor for `track_idx` based on the
/// project track's [`SynthAssignment`] variant. Both editors are
/// placeholders for now — U5 lands the [`WavetableEditor`] controls,
/// U7 lands the [`DrumEditor`] controls.
#[component]
pub fn SynthEditor(track_idx: usize) -> NodeHandle {
    let audio = use_store::<AudioResources>();
    let Some(track) = audio.project.tracks.get(track_idx) else {
        // Defensive: the parent gates on selected_track being a valid
        // index, but if a future flow ever picks up a stale index we
        // surface an unobtrusive empty state instead of panicking.
        return rsx! { SynthEditorMissing { } };
    };

    let track_name = track.name.clone();
    let assignment = synth_kind(&track.synth);

    rsx! {
        div { style: {wrap_style()},
            SynthEditorHeader {
                track_name: track_name,
                synth_label: assignment.label.to_string(),
            }
            match assignment.kind {
                SynthEditorKind::Wavetable => WavetableEditor { track_idx: track_idx },
                SynthEditorKind::Drum => DrumEditor { track_idx: track_idx },
            }
        }
    }
}

fn wrap_style() -> String {
    "display: flex; flex-direction: column; min-height: 0; flex: 1;".to_string()
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SynthEditorKind {
    Wavetable,
    Drum,
}

struct AssignmentLabel {
    label: &'static str,
    kind: SynthEditorKind,
}

fn synth_kind(synth: &SynthAssignment) -> AssignmentLabel {
    match synth {
        SynthAssignment::Wavetable(_) => AssignmentLabel {
            label: "Wavetable",
            kind: SynthEditorKind::Wavetable,
        },
        SynthAssignment::Drum(_) => AssignmentLabel {
            label: "Drum",
            kind: SynthEditorKind::Drum,
        },
    }
}

#[component]
fn SynthEditorHeader(track_name: String, synth_label: String) -> NodeHandle {
    let header_style = format!(
        "padding: 12px 14px; border-bottom: 1px solid {line}; \
         display: flex; flex-direction: column; gap: 4px;",
        line = theme::LINE,
    );
    let title_style = "font-size: 15px; font-weight: 600; \
         color: rgba(232,234,238,0.96); letter-spacing: -0.2px;";
    let kind_style = "font-size: 11px; color: rgba(232,234,238,0.42); \
         letter-spacing: 0.4px; text-transform: uppercase;";
    let caption = capitalize(&track_name);
    rsx! {
        div { style: {header_style.clone()},
            div { style: {title_style.to_string()}, {caption.clone()} }
            div { style: {kind_style.to_string()}, {synth_label.clone()} }
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

// `WavetableEditor` is implemented in the sibling
// `wavetable_editor` module and re-exported through the parent
// inspector module — keeps the dispatcher here focused on
// dispatching, and the (eventually large) editor body in its own
// file under the workspace 700-line cap.
pub use super::wavetable_editor::WavetableEditor;

// `DrumEditor` is implemented in the sibling `drum_editor` module
// (U7) and re-exported through the parent inspector module. Same
// split rationale as `WavetableEditor`.
pub use super::drum_editor::DrumEditor;

pub(super) fn placeholder_style() -> String {
    "flex: 1; padding: 24px; \
     display: flex; align-items: center; justify-content: center; \
     font-size: 12.5px; color: rgba(232,234,238,0.42); \
     text-align: center;"
        .to_string()
}

#[component]
fn SynthEditorMissing() -> NodeHandle {
    rsx! {
        div { style: {placeholder_style()},
            "Track no longer exists"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rawdaw_model::fixtures::build_round1_project;
    use rawdaw_model::patch::drum::DrumPatchData;
    use rawdaw_model::patch::wavetable::WavetablePatchData;

    #[test]
    fn synth_kind_dispatches_wavetable_to_wavetable_editor() {
        let synth = SynthAssignment::Wavetable(WavetablePatchData::default());
        let assignment = synth_kind(&synth);
        assert_eq!(assignment.kind, SynthEditorKind::Wavetable);
        assert_eq!(assignment.label, "Wavetable");
    }

    #[test]
    fn synth_kind_dispatches_drum_to_drum_editor() {
        let synth = SynthAssignment::Drum(DrumPatchData::default());
        let assignment = synth_kind(&synth);
        assert_eq!(assignment.kind, SynthEditorKind::Drum);
        assert_eq!(assignment.label, "Drum");
    }

    #[test]
    fn round_1_pitched_tracks_dispatch_to_wavetable() {
        // bass / lead / pad in the round-1 fixture are Pitched tracks
        // with the default Wavetable synth assignment; only drums is
        // Drum. The dispatcher contract for U4 is that every Pitched
        // track in the fixture lands on Wavetable and every Drum
        // track lands on Drum — this pins the per-track mapping so
        // U5/U7 can rely on it.
        let (project, _) = build_round1_project();
        let kinds: Vec<SynthEditorKind> = project
            .tracks
            .iter()
            .map(|t| synth_kind(&t.synth).kind)
            .collect();
        // round-1 track order is [bass, lead, drums, pad].
        assert_eq!(
            kinds,
            vec![
                SynthEditorKind::Wavetable,
                SynthEditorKind::Wavetable,
                SynthEditorKind::Drum,
                SynthEditorKind::Wavetable,
            ],
        );
    }
}
