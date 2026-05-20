//! Activations region of the section editor. Phase 2 ships only the
//! header (`Activations · N project tracks` + `New track to project`);
//! Phase 4 lands the activation cells themselves.

use rinch::prelude::*;

use crate::parts::Icon;
use crate::state::AppState;
use crate::theme;

#[component]
pub fn ActivationsHeader() -> NodeHandle {
    let app = use_store::<AppState>();
    let n = app.project.get().tracks.len();

    let row_style = format!(
        "display: flex; align-items: center; gap: 8px; \
         padding: 14px 20px 8px; color: {text0};",
        text0 = theme::TEXT0,
    );
    let label_style = format!(
        "font-size: 10.5px; letter-spacing: 0.6px; text-transform: uppercase; \
         color: {text2}; font-weight: 600;",
        text2 = theme::TEXT2,
    );
    let count_style = format!("font-size: 11px; color: {text3};", text3 = theme::TEXT3);

    let new_track_style = format!(
        "display: inline-flex; align-items: center; gap: 4px; \
         padding: 4px 8px; border-radius: 3px; \
         background: transparent; border: 1px solid {line}; \
         color: {text1}; cursor: pointer; font-size: 11px; \
         font-family: inherit;",
        line = theme::LINE,
        text1 = theme::TEXT1,
    );
    let stroke = theme::TEXT1.to_string();
    let sz = 11.0_f32;
    let sw = 1.6_f32;

    let count_label = format!("· {n} project tracks");

    rsx! {
        div { style: {row_style.clone()},
            span { style: {label_style.clone()}, "Activations" }
            span { style: {count_style.clone()}, {count_label.clone()} }
            span { style: "flex: 1;" }
            button { r#type: "button", style: {new_track_style.clone()},
                title: "Adds a track to the project (visible in every section)",
                Icon { glyph: "plus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
                span { "New track to project" }
            }
        }
    }
}
