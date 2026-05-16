//! Section meta bar: Duration · Scale override · Chord loops.
//!
//! Mirrors round-2's `SectionMetaBar`. Three fields in a 200/240/1fr
//! grid; each shows a `↳ base` inheritance pill when the current
//! variant doesn't override that field.
//!
//! ## Inheritance computation (round-2 README port-time note)
//!
//! The mockup's `↳ base` tag fires when `current_variant !=
//! default_variant && variant_override_for_that_field.is_none()`. Our
//! current `Section.variant_overrides` shape only carries activation
//! overrides — duration / scale / chord-loops are never overridden in
//! the round-2 fixture. So the practical rule is:
//!
//! - Non-base variant → all three fields show `↳ base`.
//! - Base variant → no inheritance tags.
//!
//! Field-level variant overrides on the section meta are a v2
//! extension to the data model; flagged here so the rule above stays
//! correct as the model grows.

use rinch::prelude::*;

use crate::fixture;
use crate::parts::Icon;
use crate::state::{AppState, EditorMode};
use crate::theme;

#[component]
pub fn SectionMetaBar(section_name_key: String) -> NodeHandle {
    let r = fixture::round1();
    let section = fixture::section_by_key(&r, section_name_key.as_str())
        .expect("section editor target must exist in the fixture");

    // Static-for-this-section values — `default_variant` is on the
    // section template, not on the per-variant view. The `inherited`
    // flag at each field is computed reactively against the active
    // variant inside FieldLabelRow.
    let default_variant = section.default_variant.to_string();
    let duration = section.base_duration_bars;
    let loop_name = section.chord_loops.first().copied().unwrap_or("").to_string();
    let loop_color = fixture::chord_loop_by_name(&r, loop_name.as_str())
        .map(|c| c.color.to_string())
        .unwrap_or_else(|| theme::PAL_TERRA.to_string());

    let bar_style = format!(
        "flex: 0 0 auto; padding: 12px 20px; \
         border-bottom: 1px solid {line}; \
         display: grid; grid-template-columns: 200px 240px 1fr; \
         gap: 18px; align-items: stretch;",
        line = theme::LINE,
    );

    rsx! {
        div { style: {bar_style.clone()},
            DurationField  { default_variant: default_variant.clone(), duration: duration }
            ScaleField     { default_variant: default_variant.clone() }
            ChordLoopsField { default_variant: default_variant.clone(),
                              loop_name: loop_name,
                              loop_color: loop_color }
        }
    }
}

// ─── Per-field components ─────────────────────────────────────────────────
//
// Each field component owns its own label row (with the optional `↳
// base` pill and `+ action` button) plus the field's content. Inlining
// the label-row logic per field avoids needing a `children`-style prop
// on a shared shell — String captures inside rsx's `if` blocks fight
// the macro's `move` Fn boundary, and inlining sidesteps that without
// adding an Rc layer.

#[component]
fn DurationField(default_variant: String, duration: u32) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 5px;",
            FieldLabelRow { label: "Duration",
                            default_variant: default_variant,
                            action_label: "" }
            NumStepperMini { value: duration, unit: "bars" }
        }
    }
}

#[component]
fn ScaleField(default_variant: String) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 5px;",
            FieldLabelRow { label: "Scale override",
                            default_variant: default_variant,
                            action_label: "" }
            PseudoSelect { value: "Inherit project key (C major)" }
        }
    }
}

#[component]
fn ChordLoopsField(
    default_variant: String,
    loop_name: String,
    loop_color: String,
) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 5px;",
            FieldLabelRow { label: "Chord loops",
                            default_variant: default_variant,
                            action_label: "Loop range" }
            ChordLoopPlaceholder { loop_name: loop_name, color: loop_color }
        }
    }
}

#[component]
fn FieldLabelRow(label: String, default_variant: String, action_label: String) -> NodeHandle {
    let app = use_store::<AppState>();
    let label_row_style = format!(
        "font-size: 10px; letter-spacing: 0.6px; \
         text-transform: uppercase; color: {text2}; font-weight: 600; \
         display: flex; align-items: center; gap: 5px;",
        text2 = theme::TEXT2,
    );
    let has_action = !action_label.is_empty();

    // The `inherited` predicate reads the signal each render. The rsx
    // `if` auto-tracks signal reads inside its scrutinee (Rule 14), so
    // the pill appears/disappears as variant tabs are clicked without
    // re-mounting the component.
    let default_for_check = default_variant.clone();

    rsx! {
        div { style: {label_row_style.clone()},
            span { {label.clone()} }
            if matches!(
                app.editor_mode.get(),
                EditorMode::SectionEditor { ref variant, .. }
                    if variant != &default_for_check
            ) {
                // Inline style literal — String captures inside rsx `if`
                // blocks hit the macro's `move` Fn boundary. theme::TEXT3
                // / LINE_SOFT are inlined here.
                span { style: "color: rgba(232,234,238,0.28); font-size: 10px; \
                               cursor: help; padding: 0 4px; \
                               border: 1px solid #1E222A; border-radius: 2px; \
                               letter-spacing: 0.2px; text-transform: none;",
                    title: "inherited from base",
                    "↳ base"
                }
            }
            span { style: "flex: 1;" }
            if has_action {
                ActionPill { label: action_label.clone() }
            }
        }
    }
}

#[component]
fn ActionPill(label: String) -> NodeHandle {
    let style = format!(
        "padding: 1px 6px; border-radius: 2px; \
         background: transparent; border: 1px solid {line_soft}; \
         color: {text2}; cursor: pointer; \
         display: inline-flex; align-items: center; gap: 3px; \
         font-size: 10.5px; letter-spacing: 0.2px; text-transform: none; \
         height: 16px; font-family: inherit;",
        line_soft = theme::LINE_SOFT,
        text2 = theme::TEXT2,
    );
    let stroke = theme::TEXT2.to_string();
    let sz = 10.0_f32;
    let sw = 1.6_f32;
    rsx! {
        button { r#type: "button", style: {style.clone()},
            title: "Add another (BarRange, ChordLoopRef) to this section",
            Icon { glyph: "plus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            span { {label.clone()} }
        }
    }
}

#[component]
fn NumStepperMini(value: u32, unit: String) -> NodeHandle {
    let wrap_style = format!(
        "display: inline-flex; align-items: stretch; \
         background: {bg0}; border: 1px solid {line}; border-radius: 4px; \
         width: fit-content;",
        bg0 = theme::BG0,
        line = theme::LINE,
    );
    // Per round-1's NumStepper pattern: bind the segment-button style as
    // a `&str` so `.to_string()` at the call sites doesn't move it.
    let seg_btn_style: &str = "width: 24px; height: 24px; padding: 0; \
         background: transparent; border: 0; color: rgba(232,234,238,0.42); \
         cursor: pointer; \
         display: inline-flex; align-items: center; justify-content: center; \
         font-family: inherit;";
    let mid_style = format!(
        "padding: 3px 12px; min-width: 28px; text-align: center; \
         font-size: 12.5px; color: {text0}; \
         font-feature-settings: \"tnum\" 1; font-variant-numeric: tabular-nums; \
         border-left: 1px solid {line}; border-right: 1px solid {line};",
        text0 = theme::TEXT0,
        line = theme::LINE,
    );
    let unit_style = format!(
        "padding: 3px 10px; font-size: 11px; color: {text2}; align-self: center; \
         border-left: 1px solid {line};",
        text2 = theme::TEXT2,
        line = theme::LINE,
    );

    let stroke = theme::TEXT1.to_string();
    let sz = 12.0_f32;
    let sw = 1.6_f32;
    let value_str = value.to_string();

    rsx! {
        div { style: {wrap_style.clone()},
            button { r#type: "button", style: {seg_btn_style.to_string()},
                Icon { glyph: "minus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            }
            div { style: {mid_style.clone()}, {value_str.clone()} }
            button { r#type: "button", style: {seg_btn_style.to_string()},
                Icon { glyph: "plus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            }
            div { style: {unit_style.clone()}, {unit.clone()} }
        }
    }
}

#[component]
fn PseudoSelect(value: String) -> NodeHandle {
    let wrap_style = format!(
        "display: flex; align-items: center; \
         padding: 4px 10px; border-radius: 4px; \
         background: {bg0}; border: 1px solid {line}; \
         font-size: 12px; color: {text0};",
        bg0 = theme::BG0,
        line = theme::LINE,
        text0 = theme::TEXT0,
    );
    let stroke = theme::TEXT2.to_string();
    let sz = 12.0_f32;
    let sw = 1.6_f32;
    rsx! {
        div { style: {wrap_style.clone()},
            span { style: "flex: 1;", {value.clone()} }
            Icon { glyph: "chevron-d", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
        }
    }
}

/// Placeholder rendering for the chord-loop strip. Phase 3 of the
/// port plan replaces this with the real bar-tiled `ChordLoopBar`
/// (custom SVG). For now we render a single muted row that names the
/// loop, so the meta bar's grid is the right size.
#[component]
fn ChordLoopPlaceholder(loop_name: String, color: String) -> NodeHandle {
    let row_style = format!(
        "display: flex; align-items: center; gap: 8px; \
         padding: 8px 10px; border-radius: 4px; height: 38px; box-sizing: border-box; \
         background: {bg0}; border: 1px solid {line};",
        bg0 = theme::BG0,
        line = theme::LINE,
    );
    let swatch_style = format!(
        "width: 8px; height: 8px; border-radius: 2px; \
         background: {col}; flex: 0 0 auto;",
        col = color,
    );
    let name_style = format!("font-size: 12px; color: {text0};", text0 = theme::TEXT0);
    let hint_style = format!(
        "margin-left: auto; font-size: 10.5px; color: {text3}; font-style: italic;",
        text3 = theme::TEXT3,
    );
    rsx! {
        div { style: {row_style.clone()},
            span { style: {swatch_style.clone()} }
            span { style: {name_style.clone()}, {loop_name.clone()} }
            span { style: {hint_style.clone()}, "(strip in phase 3)" }
        }
    }
}
