//! One chord-event cell on the timeline. Sized proportionally to
//! the event's duration relative to the loop length; carries the
//! Roman + absolute labels and the focused-state highlight.
//!
//! Focus state lives on [`AppState::focused_chord_event_idx`] so
//! the block reads it reactively and the timeline + inspector share
//! a single source of truth.

use rinch::prelude::*;

use crate::parts::rgba;
use crate::state::AppState;
use crate::theme;

#[component]
pub(super) fn EventBlock(
    idx: usize,
    roman: String,
    absolute: String,
    width_frac: f32,
    color: String,
) -> NodeHandle {
    let color_for_style = color.clone();
    rsx! {
        div {
            style: {|| {
                let focused = use_store::<AppState>().focused_chord_event_idx.get() == Some(idx);
                let pct = (width_frac * 100.0).max(0.5);
                let border_color = rgba(color_for_style.as_str(), 0.45);
                let bg = if focused {
                    rgba(color_for_style.as_str(), 0.20)
                } else {
                    rgba(color_for_style.as_str(), 0.08)
                };
                let stripe = if focused {
                    format!("3px solid {}", color_for_style)
                } else {
                    format!("1px solid {}", border_color)
                };
                format!(
                    "flex: 0 0 {pct}%; min-width: 24px; \
                     box-sizing: border-box; \
                     padding: 4px 6px; margin-right: 2px; \
                     background: {bg}; \
                     border: 1px solid {border_color}; \
                     border-left: {stripe}; border-radius: 3px; \
                     cursor: pointer; \
                     display: flex; flex-direction: column; \
                     justify-content: center; gap: 2px;",
                )
            }},
            onclick: move || {
                use_store::<AppState>().focused_chord_event_idx.set(Some(idx));
            },
            div {
                style: format!(
                    "font-size: 13px; font-weight: 600; \
                     letter-spacing: 0.4px; color: {text}; \
                     font-feature-settings: \"tnum\" 1; line-height: 1;",
                    text = theme::TEXT0,
                ),
                {roman.clone()}
            }
            div {
                style: format!(
                    "font-size: 10px; color: {text2}; \
                     font-feature-settings: \"tnum\" 1; line-height: 1; \
                     overflow: hidden; text-overflow: ellipsis; \
                     white-space: nowrap;",
                    text2 = theme::TEXT2,
                ),
                {absolute.clone()}
            }
        }
    }
}
