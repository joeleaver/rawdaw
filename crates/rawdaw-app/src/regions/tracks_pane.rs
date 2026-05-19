//! TracksPane — left-of-arrangement strip listing each project track.
//!
//! Lands as part of Phase U4 of the synth-UI integration plan. Each row
//! is a click target that selects a project track via
//! [`AppState::select_track`], which clears the section-block
//! selection. Selection drives the inspector's synth-editor branch.
//!
//! The pane lives between the [`Library`](super::library::Library) and
//! [`Arrangement`](super::arrangement::Arrangement) regions inside
//! `ArrangementSurface`. It reads the track list off
//! [`AudioResources::project`] (the model snapshot owned by the
//! engine-build) rather than the app-side fixture — the synth dispatch
//! in U4 needs `Track::synth`, which only lives on the model type.
//!
//! A "Pitched" / "Drum" tag plus the role pill mirrors the section
//! editor's existing `TrackRow` styling so the two views agree
//! visually.

use rinch::prelude::*;

use rawdaw_model::patch::SynthAssignment;
use rawdaw_model::track::{Role, TrackKind};

use crate::audio::AudioResources;
use crate::parts::{rgba, Icon};
use crate::state::AppState;
use crate::theme;

#[component]
pub fn TracksPane() -> NodeHandle {
    let pane_style = format!(
        "width: 200px; flex: 0 0 200px; \
         background: {bg}; border-right: 1px solid {line}; \
         display: flex; flex-direction: column; min-height: 0;",
        bg = theme::BG1,
        line = theme::LINE,
    );

    rsx! {
        aside { style: {pane_style.clone()},
            TracksPaneHeader { }
            div {
                style: "flex: 1; overflow-y: auto; min-height: 0; padding: 4px 0;",
                for row in build_track_rows() {
                    TrackRow {
                        key: row.idx,
                        idx: row.idx,
                        name: row.name,
                        kind_label: row.kind_label,
                        role_label: row.role_label,
                        accent: row.accent,
                    }
                }
                NewTrackBtn { }
            }
        }
    }
}

#[component]
fn TracksPaneHeader() -> NodeHandle {
    let header_style = format!(
        "display: flex; align-items: center; gap: 6px; \
         padding: 8px 10px; border-bottom: 1px solid {line};",
        line = theme::LINE,
    );
    let title_style = "font-size: 11px; letter-spacing: 0.6px; \
         text-transform: uppercase; font-weight: 600; \
         color: rgba(232,234,238,0.62); flex: 1;";
    rsx! {
        div { style: {header_style.clone()},
            span { style: {title_style.to_string()}, "Tracks" }
        }
    }
}

/// One row payload — owned primitives so it satisfies the rsx `for`
/// `Clone + PartialEq + 'static` bound.
#[derive(Clone, PartialEq)]
struct TrackRowData {
    idx: usize,
    name: String,
    kind_label: String,
    role_label: String,
    accent: String,
}

fn build_track_rows() -> Vec<TrackRowData> {
    let audio = use_store::<AudioResources>();
    audio
        .project
        .tracks
        .iter()
        .enumerate()
        .map(|(idx, t)| TrackRowData {
            idx,
            name: capitalize(&t.name),
            kind_label: kind_label_for(&t.synth),
            role_label: role_label_for(&t.kind),
            accent: accent_for(&t.kind),
        })
        .collect()
}

fn kind_label_for(synth: &SynthAssignment) -> String {
    match synth {
        SynthAssignment::Wavetable(_) => "Wavetable".to_string(),
        SynthAssignment::Drum(_) => "Drum".to_string(),
    }
}

fn role_label_for(kind: &TrackKind) -> String {
    match kind {
        TrackKind::Drum { .. } => "kit".to_string(),
        TrackKind::Pitched { role } => match role {
            Role::Bass => "bass",
            Role::Voicing => "voicing",
            Role::Arp => "arp",
            Role::Melodic => "melodic",
            Role::Pad => "pad",
            Role::Countermelody => "counter",
            Role::Other => "other",
        }
        .to_string(),
    }
}

fn accent_for(kind: &TrackKind) -> String {
    // Borrow the section-editor's existing convention: drum tracks
    // are dimmer; pitched tracks use the standard text color. The
    // accent shows in the left stripe — it's a quick scan cue, not a
    // per-role palette.
    match kind {
        TrackKind::Drum { .. } => "rgba(232,234,238,0.42)".to_string(),
        TrackKind::Pitched { .. } => "rgba(232,234,238,0.72)".to_string(),
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[component]
fn TrackRow(
    idx: usize,
    name: String,
    kind_label: String,
    role_label: String,
    accent: String,
) -> NodeHandle {
    let app = use_store::<AppState>();

    // Base style is static; the reactive `style:` closure below
    // composes per-state overrides on top. Done this way so the row
    // never re-mounts on selection changes — only the bg/border
    // strings flip.
    let row_static = "display: flex; align-items: center; gap: 8px; \
         padding: 6px 10px; cursor: pointer; min-height: 30px; \
         border-left: 2px solid transparent;";
    let stripe_color = accent.clone();
    let name_style = "font-size: 12.5px; color: rgba(232,234,238,0.96); \
         font-weight: 500; flex: 1; overflow: hidden; \
         text-overflow: ellipsis; white-space: nowrap;";
    let kind_style = "font-size: 10px; color: rgba(232,234,238,0.42); \
         letter-spacing: 0.4px;";

    // Composite kind/role caption — `WAVETABLE · bass` / `DRUM · kit`.
    // Drum rows currently render their kit as "kit" (the kit id isn't
    // surfaced yet); U7 surfaces kit details.
    let kind_caption = format!(
        "{} · {}",
        kind_label.to_uppercase(),
        role_label,
    );

    rsx! {
        div {
            style: {
                let selected = app.selected_track.get() == Some(idx);
                let bg = if selected {
                    rgba(stripe_color.as_str(), 0.16)
                } else {
                    "transparent".to_string()
                };
                let border_left = if selected {
                    format!("2px solid {}", stripe_color)
                } else {
                    "2px solid transparent".to_string()
                };
                format!(
                    "{row_static} background: {bg}; border-left: {border_left};",
                )
            },
            onclick: move || app.select_track(Some(idx)),
            div { style: "display: flex; flex-direction: column; min-width: 0; flex: 1; gap: 1px;",
                span { style: {name_style.to_string()}, {name.clone()} }
                span { style: {kind_style.to_string()}, {kind_caption.clone()} }
            }
        }
    }
}

#[component]
fn NewTrackBtn() -> NodeHandle {
    let btn_style = "display: flex; align-items: center; gap: 6px; \
         margin: 6px 10px; padding: 4px 6px; \
         background: transparent; border: 0; \
         color: rgba(232,234,238,0.42); cursor: pointer; \
         border-radius: 3px; font-size: 11px; font-family: inherit;";
    let stroke = "rgba(232,234,238,0.42)".to_string();
    let sz = 12.0_f32;
    let sw = 1.6_f32;
    rsx! {
        button {
            r#type: "button",
            style: {btn_style.to_string()},
            Icon { glyph: "plus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            span { "new track" }
        }
    }
}
