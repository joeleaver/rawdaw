//! Dashed-border placeholder for tracks with no entry in either base
//! or the active variant override list. Per round-2 README decision
//! 14, these still render so the user sees every project track.

use rinch::prelude::*;

use rawdaw_model::id::{SectionId, TrackId};

use crate::overlay::TrackKindTag as TrackKind;
use crate::parts::Icon;
use crate::theme;

use super::pattern_select::PatternSelect;

#[component]
pub fn CellInherit(
    section_id: SectionId,
    track_id: TrackId,
    track_name: String,
    track_kind: TrackKind,
    track_role: String,
    reason: String,
) -> NodeHandle {
    let outer_style = format!(
        "background: {bg1}; border: 1px dashed {line}; \
         border-radius: 6px; padding: 14px 18px; \
         display: flex; align-items: center; gap: 12px; \
         color: {text2}; font-size: 12.5px;",
        bg1 = theme::BG1,
        line = theme::LINE,
        text2 = theme::TEXT2,
    );
    let kind_label = match track_kind {
        TrackKind::Pitched => "PITCHED",
        TrackKind::Drum => "DRUM",
    };
    let kind_style = format!(
        "font-size: 10px; color: {text3}; letter-spacing: 0.4px;",
        text3 = theme::TEXT3,
    );
    let name_style = format!(
        "font-weight: 600; color: {text1};",
        text1 = theme::TEXT1,
    );
    let dot_stroke = theme::TEXT3.to_string();
    let show_role = !track_role.is_empty() && track_role != "—";
    let role_label = format!("role: {track_role}");
    let kind_label_owned = kind_label.to_string();
    let name_owned = track_name.clone();
    let reason_owned = reason.clone();
    let reason_style = format!("color: {text3};", text3 = theme::TEXT3);

    rsx! {
        div { style: {outer_style.clone()},
            Icon { glyph: "dot", size: 8.0, stroke: {dot_stroke.clone()}, stroke_width: 1.6 }
            strong { style: {name_style.clone()}, {name_owned.clone()} }
            span { style: {kind_style.clone()}, {kind_label_owned.clone()} }
            if show_role {
                RolePill { label: role_label.clone() }
            }
            span { style: "flex: 1;" }
            span { style: {reason_style.clone()}, {reason_owned.clone()} }
            // P4: picking a pattern here creates the activation
            // entry via `set_activation_pattern`. The "+ Add
            // activation" affordance from the round-2 mockup
            // collapses into the picker itself — one click, one
            // resulting entry.
            PatternSelect {
                section_id: section_id,
                track_id: track_id,
                current_pattern_value: super::pattern_select::NO_PATTERN_SENTINEL,
            }
        }
    }
}

#[component]
fn RolePill(label: String) -> NodeHandle {
    let style = format!(
        "display: inline-flex; align-items: center; height: 16px; \
         padding: 0 5px; border-radius: 3px; \
         font-size: 10px; color: {text2}; \
         background: {bg0}; border: 1px solid {line};",
        text2 = theme::TEXT2,
        bg0 = theme::BG0,
        line = theme::LINE,
    );
    rsx! { span { style: {style.clone()}, {label.clone()} } }
}
