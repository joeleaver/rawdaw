//! Variant tab strip: `base default variant · stripped · + New variant`.
//!
//! Mirrors round-2's `section-editor.jsx` variant-tabs strip. Clicking
//! a tab mutates `AppState.editor_mode.variant` via `set_variant`; the
//! active-tab styling is driven directly off the same signal via
//! `{|| ...}` closures so the visual updates without the component
//! re-mounting (rinch components render once).
//!
//! The `+ New variant` affordance is visual-only for now; round-3
//! covers variant creation flows.
//!
//! Decision 19 / 21 in the round-2 README: the "default variant"
//! label is **italic light-grey** text next to the tab name, NOT an
//! uppercase chip. This is the section's own default — distinct from
//! the cell-schedule "no schedule" and pattern-default treatments.

use rinch::prelude::*;

use crate::parts::Icon;
use crate::regions::inspector::variant_options_for_section;
use crate::state::{AppState, EditorMode};
use crate::theme;

#[component]
pub fn VariantTabs(
    section_name_key: String,
    section_color: String,
    default_variant: String,
) -> NodeHandle {
    let strip_style = format!(
        "flex: 0 0 auto; display: flex; gap: 2px; \
         padding: 0 20px; align-items: flex-end; \
         background: {bg1}; border-bottom: 1px solid {line};",
        bg1 = theme::BG1,
        line = theme::LINE,
    );

    let key = section_name_key.clone();
    let default = default_variant.clone();
    let color = section_color.clone();

    rsx! {
        div { style: {strip_style.clone()},
            for v in variant_options_for_section(key.clone()) {
                VariantTab {
                    key: v.id.clone(),
                    variant_id: v.id.clone(),
                    variant_name: v.name,
                    is_default: v.id == default.as_str(),
                    section_color: color.clone(),
                }
            }
            NewVariantBtn { }
        }
    }
}

#[component]
fn VariantTab(
    variant_id: String,
    variant_name: String,
    is_default: bool,
    section_color: String,
) -> NodeHandle {
    let app = use_store::<AppState>();
    let target = variant_id.clone();

    // Pre-built style strings for the two states. The {|| ...} closure
    // below reads the signal on each style update and swaps which one
    // applies.
    let active_style = format!(
        "padding: 7px 14px 8px; border-radius: 4px 4px 0 0; border: 0; \
         border-bottom: 2px solid {section_color}; background: {bg0}; \
         color: {text0}; font-size: 12.5px; font-weight: 600; \
         cursor: pointer; position: relative; top: 1px; \
         letter-spacing: -0.05px; font-family: inherit; \
         display: inline-flex; align-items: center; gap: 8px;",
        bg0 = theme::BG0,
        text0 = theme::TEXT0,
    );
    let inactive_style = format!(
        "padding: 7px 14px 8px; border-radius: 4px 4px 0 0; border: 0; \
         border-bottom: 2px solid transparent; background: transparent; \
         color: {text1}; font-size: 12.5px; font-weight: 500; \
         cursor: pointer; position: relative; top: 1px; \
         letter-spacing: -0.05px; font-family: inherit; \
         display: inline-flex; align-items: center; gap: 8px;",
        text1 = theme::TEXT1,
    );

    // Two captures of `target` — one for the reactive style expression,
    // one for the click handler. Strings need a fresh owner per move
    // closure boundary.
    let target_for_style = target.clone();
    let target_for_click = target.clone();
    let _ = section_color; // captured by the format!s above

    // The rsx macro wraps non-literal `style:` expressions in an effect
    // closure that tracks signal reads inside. Writing `move || ...`
    // around the expression here would double-wrap and force String
    // captures to be consumed by an inner closure that can only be
    // called once. The bare expression below lets the macro's own
    // effect do the right thing: borrow active_style / inactive_style
    // each invocation.
    rsx! {
        button {
            r#type: "button",
            style: {
                if matches!(
                    app.editor_mode.get(),
                    EditorMode::SectionEditor { ref variant, .. } if variant == &target_for_style
                ) {
                    active_style.clone()
                } else {
                    inactive_style.clone()
                }
            },
            onclick: move || app.set_variant(target_for_click.clone()),
            {variant_name.clone()}
            if is_default {
                // Inline style literal — String captures inside rsx `if`
                // blocks run afoul of the macro's `move` Fn boundary;
                // theme::TEXT3 is hardcoded here.
                span { style: "font-size: 9.5px; color: rgba(232,234,238,0.28); \
                               letter-spacing: 0.4px; font-style: italic;",
                    "default variant"
                }
            }
        }
    }
}

#[component]
fn NewVariantBtn() -> NodeHandle {
    let btn_style = format!(
        "padding: 6px 8px; border-radius: 3px; border: 0; \
         background: transparent; color: {text2}; cursor: pointer; \
         display: inline-flex; align-items: center; gap: 4px; \
         margin-left: 4px; font-size: 11.5px; \
         font-family: inherit; align-self: center;",
        text2 = theme::TEXT2,
    );
    let stroke = theme::TEXT2.to_string();
    let sz = 11.0_f32;
    let sw = 1.6_f32;
    rsx! {
        button {
            r#type: "button",
            style: {btn_style.clone()},
            title: "Create a new sparse-override variant of this section",
            Icon { glyph: "plus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            span { "New variant" }
        }
    }
}
