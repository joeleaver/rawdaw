//! Master-FX inspector body.
//!
//! Per-kind dispatcher (mirror of U5's `SynthEditor`): the chain
//! breadcrumb sits at the top, and the editor body below
//! dispatches on the slot's [`crate::audio::MasterFxKind`] to the
//! kind-specific editor (X6 ships [`super::soft_clip_editor::
//! SoftClipEditor`]; future EQ / Reverb editors plug in here).
//!
//! Sourced from the live [`crate::audio::MasterFxEditorHandle`]
//! table so the slot count and per-slot kinds reflect whatever
//! `Project.master_chain.fx` currently carries.

use rinch::prelude::*;

use crate::audio::{AudioResources, MasterFxKind};
use crate::parts::Icon;
use crate::theme;

use super::soft_clip_editor::SoftClipEditor;

#[component]
pub fn MasterFxEditor(slot: usize) -> NodeHandle {
    let audio = use_store::<AudioResources>();
    let handles = audio.master_fx_handles.clone();
    let Some(handle) = handles.get(slot).cloned() else {
        // Selected slot is past the chain length — possible if the
        // user is mid-edit and the chain was just reconfigured.
        // Render an empty state rather than panicking.
        return rsx! { OutOfRange { slot: slot } };
    };
    let kind_label = match handle.kind {
        MasterFxKind::SoftClip => "SoftClip".to_string(),
    };
    let is_soft_clip = matches!(handle.kind, MasterFxKind::SoftClip);

    let wrap_style = "display: flex; flex-direction: column; \
         min-height: 0; flex: 1;";
    let header_style = format!(
        "padding: 14px 16px 10px; border-bottom: 1px solid {line};",
        line = theme::LINE,
    );
    let crumb_style = "font-size: 11px; letter-spacing: 0.5px; \
         text-transform: uppercase; color: rgba(232,234,238,0.42); \
         font-weight: 600;";
    let title_style = "font-size: 16px; font-weight: 600; \
         color: rgba(232,234,238,0.96); margin-top: 4px;";

    rsx! {
        div { style: {wrap_style.to_string()},
            div { style: {header_style.clone()},
                div { style: {crumb_style.to_string()},
                    {format!("Master · slot {slot} · {kind_label}")}
                }
                div { style: {title_style.to_string()}, "Master FX" }
            }
            // Per-kind editor dispatch. v1 only has SoftClip; the
            // `if` arms grow as new FX kinds land — `else if` for
            // each variant of `MasterFxKind`, falling through to
            // an explicit "unhandled" state so we notice when a
            // new kind ships without an editor.
            if is_soft_clip {
                SoftClipEditor { slot: slot }
            } else {
                UnhandledKind { }
            }
        }
    }
}

/// Fallback for a `MasterFxKind` variant without an editor body
/// yet. With v1's single `SoftClip` variant this never renders;
/// future EQ / Reverb additions will trip it during a partial
/// adoption window if their editor lands behind the kind.
#[component]
fn UnhandledKind() -> NodeHandle {
    let wrap_style = "flex: 1; display: flex; flex-direction: column; \
         align-items: center; justify-content: center; padding: 24px; \
         gap: 8px; text-align: center;";
    let copy_style = "font-size: 12.5px; color: rgba(232,234,238,0.62); \
         line-height: 1.5; max-width: 240px;";
    let stroke = "rgba(232,234,238,0.28)".to_string();
    rsx! {
        div { style: {wrap_style.to_string()},
            Icon { glyph: "dot", size: 28.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
            div { style: {copy_style.to_string()},
                "No editor available for this FX kind yet."
            }
        }
    }
}

#[component]
fn OutOfRange(slot: usize) -> NodeHandle {
    let wrap_style = "flex: 1; display: flex; flex-direction: column; \
         align-items: center; justify-content: center; padding: 24px; \
         gap: 8px; text-align: center;";
    let copy_style = "font-size: 12.5px; color: rgba(232,234,238,0.62); \
         line-height: 1.5; max-width: 240px;";
    let stroke = "rgba(232,234,238,0.28)".to_string();
    rsx! {
        div { style: {wrap_style.to_string()},
            Icon { glyph: "dot", size: 28.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
            div { style: {copy_style.to_string()},
                {format!("Master-FX slot {slot} is past the chain length.")}
            }
        }
    }
}
