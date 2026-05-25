//! Master-FX inspector body.
//!
//! X5 placeholder: renders a header + a "coming in X6" copy block
//! so the selection branch has something to mount when the user
//! clicks "Master" in the tracks pane. X6 replaces the body with
//! the real `MasterFxEditor` (per-kind dispatch on the slot's
//! [`crate::audio::MasterFxKind`] →
//! [`SoftClipEditor`]/future EQ/Reverb bodies).
//!
//! The header is already X5-final: chain breadcrumb
//! ("Master · slot N · SoftClip") sourced from the live
//! [`crate::audio::MasterFxEditorHandle`] table so the slot
//! count and per-slot kinds reflect whatever
//! `Project.master_chain.fx` currently carries.

use rinch::prelude::*;

use crate::audio::{AudioResources, MasterFxKind};
use crate::parts::Icon;
use crate::theme;

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
    let body_style = "flex: 1; display: flex; flex-direction: column; \
         align-items: center; justify-content: center; padding: 24px; \
         gap: 10px; text-align: center;";
    let copy_style = "font-size: 12.5px; color: rgba(232,234,238,0.62); \
         line-height: 1.5; max-width: 240px;";
    let stroke = "rgba(232,234,238,0.28)".to_string();

    rsx! {
        div { style: {wrap_style.to_string()},
            div { style: {header_style.clone()},
                div { style: {crumb_style.to_string()},
                    {format!("Master · slot {slot} · {kind_label}")}
                }
                div { style: {title_style.to_string()}, "Master FX" }
            }
            div { style: {body_style.to_string()},
                Icon { glyph: "dot", size: 28.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
                div { style: {copy_style.to_string()},
                    "The soft-clip editor body lands in X6."
                }
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
