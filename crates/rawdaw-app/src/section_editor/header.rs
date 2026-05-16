//! Section editor top header: back chevron · breadcrumb · color stripe ·
//! section name · subtitle · Duplicate / Done buttons.
//!
//! Mirrors `docs/design/mockups/round-2/components/section-editor.jsx` ·
//! `SectionEditorHeader`. The variant tab strip lives in
//! `variant_tabs.rs` rather than here so each piece stays under the
//! 700-line cap and can be navigated independently.

use rinch::prelude::*;

use crate::parts::Icon;
use crate::state::AppState;
use crate::theme;

#[component]
pub fn SectionEditorHeader(section_color: String, section_name: String) -> NodeHandle {
    let app = use_store::<AppState>();

    let bar_style = format!(
        "flex: 0 0 auto; \
         border-bottom: 1px solid {line}; background: {bg1}; \
         padding: 10px 20px; \
         display: flex; align-items: center; gap: 12px;",
        line = theme::LINE,
        bg1 = theme::BG1,
    );

    // Back chevron — currently chevron-r (matches the mockup's visual
    // pointing the breadcrumb forward into the section). Real back
    // navigation is the Done button on the right.
    let back_btn_style = format!(
        "width: 26px; height: 26px; padding: 0; \
         display: inline-flex; align-items: center; justify-content: center; \
         background: transparent; border: 1px solid {line}; border-radius: 4px; \
         color: {text1}; cursor: pointer;",
        line = theme::LINE,
        text1 = theme::TEXT1,
    );

    let breadcrumb_style = format!("font-size: 11px; color: {text3};", text3 = theme::TEXT3);

    let stripe_style = format!(
        "width: 4px; height: 18px; border-radius: 2px; \
         background: {col}; flex: 0 0 auto;",
        col = section_color,
    );
    let title_style = format!(
        "margin: 0; font-size: 16px; font-weight: 700; \
         color: {text0}; letter-spacing: -0.3px;",
        text0 = theme::TEXT0,
    );
    let subtitle_style = format!("font-size: 11px; color: {text2};", text2 = theme::TEXT2);

    let dup_style = format!(
        "padding: 5px 10px; border-radius: 4px; \
         background: {bg0}; border: 1px solid {line}; \
         color: {text1}; cursor: pointer; font-size: 11.5px; \
         font-family: inherit;",
        bg0 = theme::BG0,
        line = theme::LINE,
        text1 = theme::TEXT1,
    );
    let done_style = format!(
        "padding: 5px 10px; border-radius: 4px; \
         background: {bg2}; border: 1px solid {line}; \
         color: {text0}; cursor: pointer; font-size: 11.5px; \
         font-weight: 500; font-family: inherit;",
        bg2 = theme::BG2,
        line = theme::LINE,
        text0 = theme::TEXT0,
    );

    let chevron_stroke = theme::TEXT1.to_string();
    let chevron_size = 13.0_f32;
    let chevron_w = 1.6_f32;

    rsx! {
        div { style: {bar_style.clone()},
            button { r#type: "button", style: {back_btn_style.clone()},
                title: "Back to arrangement",
                onclick: move || app.close_section_editor(),
                Icon { glyph: "chevron-r", size: chevron_size,
                       stroke: {chevron_stroke.clone()}, stroke_width: chevron_w }
            }
            span { style: {breadcrumb_style.clone()}, "arrangement · sections ·" }
            span { style: {stripe_style.clone()} }
            h1 { style: {title_style.clone()}, {section_name.clone()} }
            span { style: {subtitle_style.clone()}, "section editor" }
            span { style: "flex: 1;" }
            button { r#type: "button", style: {dup_style.clone()},
                "Duplicate section"
            }
            button { r#type: "button", style: {done_style.clone()},
                onclick: move || app.close_section_editor(),
                "Done"
            }
        }
    }
}
