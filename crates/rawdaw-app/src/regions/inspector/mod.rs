//! Inspector (right pane). Announces what's selected and offers the
//! most-common edits inline.
//!
//! Translates `docs/design/mockups/round-1/components/inspector.jsx`.
//! Round 1 takes a static `idx: Option<usize>` rather than a Signal —
//! the MainWindow port will switch this to `Signal<Option<usize>>` once
//! click handlers in the arrangement set it.
//!
//! Component props pass *primitive owned* data (`String`, `u32`, `bool`)
//! rather than the full `Section`/`SectionRef` structs. The macro
//! requires every field type to implement `Default`, and threading
//! `Default` through every fixture struct + nested type would balloon.
//! Doing one fixture lookup at the `SelectedInspector` boundary and
//! passing primitives down also matches the way a real `Project` will
//! be navigated — by id rather than by reference.

mod activation_table;

use rinch::prelude::*;

use crate::fixture;
use crate::parts::{rgba, Icon};
use crate::theme;

use activation_table::ActivationTable;

/// Inspector takes an optional selected SectionRef index. None →
/// empty state; Some(idx) → populated with that section's content.
#[component]
pub fn Inspector(idx: Option<usize>) -> NodeHandle {
    let pane_style = format!(
        "width: 320px; flex: 0 0 320px; \
         background: {bg}; border-left: 1px solid {line}; \
         display: flex; flex-direction: column; min-height: 0;",
        bg = theme::BG1,
        line = theme::LINE,
    );

    let idx_value = idx.unwrap_or(usize::MAX);
    let has_selection = idx.is_some();
    rsx! {
        aside { style: {pane_style.clone()},
            // rsx's `if`/`if let` codegen confuses clippy's unused-binding
            // analysis. Routing the option through a `bool` + sentinel
            // keeps both branches obvious *and* clippy happy.
            if has_selection {
                SelectedInspector { idx: idx_value }
            } else {
                InspectorEmpty { }
            }
        }
    }
}

#[component]
fn InspectorEmpty() -> NodeHandle {
    let wrap_style = "flex: 1; display: flex; flex-direction: column; \
         align-items: center; justify-content: center; \
         padding: 24px; gap: 8px; text-align: center;";
    let glyph_style = "width: 28px; height: 28px; opacity: 0.4;";
    let copy_style = "font-size: 12.5px; color: rgba(232,234,238,0.62); \
         line-height: 1.5; max-width: 240px;";
    let stroke = "rgba(232,234,238,0.28)".to_string();
    let sz = 28.0_f32;
    let sw = 1.6_f32;
    rsx! {
        div { style: {wrap_style.to_string()},
            div { style: {glyph_style.to_string()},
                Icon { glyph: "dot", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            }
            div { style: {copy_style.to_string()},
                "Select a section, pattern, or chord to inspect."
            }
        }
    }
}

#[component]
fn SelectedInspector(idx: usize) -> NodeHandle {
    let r = fixture::round1();
    let Some(block) = r.arrangement.get(idx).cloned() else {
        return rsx! { InspectorEmpty { } };
    };
    let Some(section) = fixture::section_by_key(&r, block.section_key).cloned() else {
        return rsx! { InspectorEmpty { } };
    };

    let instance_count = fixture::instance_count(&r, block.section_key);

    // Resolve all the strings/numbers we'll need up front so subcomponents
    // can take primitive props (every `#[component]` field type must impl
    // `Default`; pushing structs through props means deriving `Default`
    // on the whole fixture).
    let section_color = section.color.to_string();
    let section_name = section.name.to_string();
    let variant_id = block.variant.to_string();
    let default_variant = section.default_variant.to_string();
    let base_duration_bars = section.base_duration_bars;
    let start_bar = block.start_bar;
    let bars = block.bars;
    let chord_loop_name = section.chord_loops.first().copied().unwrap_or("").to_string();
    let chord_loop_color = fixture::chord_loop_by_name(&r, chord_loop_name.as_str())
        .map(|c| c.color.to_string())
        .unwrap_or_else(|| theme::PAL_TERRA.to_string());

    rsx! {
        div { style: "display: flex; flex-direction: column; min-height: 0; flex: 1;",
            InspectorHeader {
                section_color: section_color.clone(),
                section_name: section_name,
                variant_id: variant_id.clone(),
                default_variant: default_variant.clone(),
                instance_count: instance_count as u32,
                start_bar: start_bar,
                bars: bars,
            }
            VariantTabs {
                section_color: section_color.clone(),
                section_name_key: section.name.to_string(),
                active_variant: variant_id.clone(),
                default_variant: default_variant.clone(),
            }
            InspectorBody {
                section_name_key: section.name.to_string(),
                variant_id: variant_id,
                base_duration_bars: base_duration_bars,
                chord_loop_name: chord_loop_name,
                chord_loop_color: chord_loop_color,
            }
            InspectorFooter { }
        }
    }
}

// ─── Header ───────────────────────────────────────────────────────────────

#[component]
fn InspectorHeader(
    section_color: String,
    section_name: String,
    variant_id: String,
    default_variant: String,
    instance_count: u32,
    start_bar: u32,
    bars: u32,
) -> NodeHandle {
    let bg = rgba(section_color.as_str(), 0.06);
    let header_style = format!(
        "padding: 12px 14px; border-bottom: 1px solid {line}; \
         display: flex; flex-direction: column; gap: 6px; \
         background: {bg}; border-left: 3px solid {col};",
        line = theme::LINE, col = section_color,
    );
    let title_style = "font-size: 15px; font-weight: 600; \
         color: rgba(232,234,238,0.96); letter-spacing: -0.2px;";
    let meta_style = "font-size: 11px; color: rgba(232,234,238,0.42); \
         display: flex; align-items: center; gap: 10px;";

    let instances_label = if instance_count == 1 {
        format!("{} instance in arrangement", instance_count)
    } else {
        format!("{} instances in arrangement", instance_count)
    };
    let bar_range = format!("bar {}–{}", start_bar + 1, start_bar + bars);
    let show_variant = variant_id != default_variant;

    rsx! {
        div { style: {header_style.clone()},
            div { style: "display: flex; align-items: center; gap: 8px;",
                div { style: {title_style.to_string()}, {section_name.clone()} }
                if show_variant {
                    VariantChip {
                        variant: variant_id.clone(),
                        color: section_color.clone(),
                    }
                }
            }
            div { style: {meta_style.to_string()},
                span { {instances_label.clone()} }
                span { style: "color: rgba(232,234,238,0.28);", "·" }
                span {
                    style: "font-feature-settings: \"tnum\" 1;",
                    {bar_range.clone()}
                }
            }
        }
    }
}

#[component]
fn VariantChip(variant: String, color: String) -> NodeHandle {
    let style = format!(
        "display: inline-flex; align-items: center; \
         height: 17px; padding: 0 6px; border-radius: 3px; \
         font-size: 10.5px; font-weight: 500; letter-spacing: 0.2px; \
         background: {bg}; color: rgba(232,234,238,0.88); \
         border: 1px solid {border};",
        bg = rgba(color.as_str(), 0.16),
        border = rgba(color.as_str(), 0.30),
    );
    let label = format!("variant: {}", variant);
    rsx! {
        span { style: {style.clone()}, {label.clone()} }
    }
}

// ─── Variant tabs ─────────────────────────────────────────────────────────

#[component]
fn VariantTabs(
    section_color: String,
    section_name_key: String,
    active_variant: String,
    default_variant: String,
) -> NodeHandle {
    let strip_style = format!(
        "display: flex; padding: 8px 10px 0; gap: 2px; \
         background: {bg}; border-bottom: 1px solid {line};",
        bg = theme::BG1, line = theme::LINE,
    );

    // The rsx `for` source is wrapped in a `Fn() -> Vec<T> + 'static`
    // closure that may be invoked multiple times. We re-do the lookup
    // inline so the closure body never consumes a captured `Vec` — only
    // borrows the props it captures (`section_name_key`, etc.).
    let key = section_name_key.clone();
    let active = active_variant.clone();
    let default = default_variant.clone();
    let color = section_color.clone();

    rsx! {
        div { style: {strip_style.clone()},
            for v in fixture::round1()
                .sections
                .iter()
                .find(|s| s.name == key.as_str())
                .map(|s| s.variants.to_vec())
                .unwrap_or_default()
            {
                VariantTab {
                    key: v.id,
                    variant_id: v.id.to_string(),
                    variant_name: v.name.to_string(),
                    active: v.id == active.as_str(),
                    is_default: v.id == default.as_str(),
                    section_color: color.clone(),
                }
            }
            div { style: "margin-left: auto;",
                NewVariantBtn { }
            }
        }
    }
}

#[component]
fn VariantTab(
    variant_id: String,
    variant_name: String,
    active: bool,
    is_default: bool,
    section_color: String,
) -> NodeHandle {
    let _ = variant_id; // identity comes from the rsx `key:` prop above
    let border_bottom = if active {
        format!("2px solid {}", section_color)
    } else {
        "2px solid transparent".to_string()
    };
    let bg = if active { theme::BG2 } else { "transparent" };
    let fg = if active {
        "rgba(232,234,238,0.96)"
    } else {
        "rgba(232,234,238,0.62)"
    };
    let weight = if active { 600 } else { 500 };
    let tab_style = format!(
        "padding: 6px 10px 7px; border-radius: 4px 4px 0 0; border: 0; \
         border-bottom: {border_bottom}; background: {bg}; color: {fg}; \
         font-size: 11.5px; font-weight: {weight}; \
         cursor: pointer; position: relative; top: 1px;",
    );

    rsx! {
        button {
            r#type: "button",
            style: {tab_style.clone()},
            {variant_name.clone()}
            if is_default {
                span {
                    style: "margin-left: 6px; font-size: 9.5px; \
                            color: rgba(232,234,238,0.28); letter-spacing: 0.4px; \
                            text-transform: uppercase;",
                    "default"
                }
            }
        }
    }
}

#[component]
fn NewVariantBtn() -> NodeHandle {
    let btn_style = "padding: 6px 8px; border-radius: 3px; border: 0; \
         background: transparent; color: rgba(232,234,238,0.42); \
         cursor: pointer; display: inline-flex; align-items: center; \
         gap: 4px; font-size: 11px;";
    let stroke = "rgba(232,234,238,0.42)".to_string();
    let sz = 11.0_f32;
    let sw = 1.6_f32;
    rsx! {
        button {
            r#type: "button",
            style: {btn_style.to_string()},
            Icon { glyph: "plus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            span { " variant" }
        }
    }
}

// ─── Body (form) ──────────────────────────────────────────────────────────

#[component]
fn InspectorBody(
    section_name_key: String,
    variant_id: String,
    base_duration_bars: u32,
    chord_loop_name: String,
    chord_loop_color: String,
) -> NodeHandle {
    let body_style = "flex: 1; overflow-y: auto; padding: 12px 14px; min-height: 0;";
    let bar_range = format!("0..{}", base_duration_bars);

    rsx! {
        div { style: {body_style.to_string()},
            FieldLabel { label: "Duration" }
            div { style: "margin-bottom: 14px;",
                NumStepper { value: base_duration_bars, unit: "bars" }
            }

            FieldLabel { label: "Scale override" }
            div { style: "margin-bottom: 14px;",
                Select { value: "Inherit project key (C major)" }
            }

            FieldLabel { label: "Chord loops" }
            div { style: "margin-bottom: 14px; display: flex; flex-direction: column; gap: 4px;",
                ChordLoopRow {
                    bar_range: bar_range,
                    loop_name: chord_loop_name,
                    color: chord_loop_color,
                }
                AddRow { label: "add chord loop" }
            }

            FieldLabel { label: "Activations" }
            ActivationTable {
                section_name_key: section_name_key,
                variant_id: variant_id,
            }
        }
    }
}

#[component]
fn FieldLabel(label: String) -> NodeHandle {
    let style = "font-size: 10.5px; letter-spacing: 0.6px; \
         text-transform: uppercase; color: rgba(232,234,238,0.42); \
         font-weight: 600; margin-bottom: 6px; \
         display: flex; align-items: center; gap: 4px;";
    rsx! {
        div { style: {style.to_string()},
            span { {label.clone()} }
        }
    }
}

#[component]
fn NumStepper(value: u32, unit: String) -> NodeHandle {
    let wrap_style = format!(
        "display: inline-flex; align-items: stretch; \
         background: {bg0}; border: 1px solid {line}; border-radius: 4px; \
         width: fit-content;",
        bg0 = theme::BG0, line = theme::LINE,
    );
    let seg_style = "background: transparent; border: 0; padding: 0 8px; \
         color: rgba(232,234,238,0.62); cursor: pointer; \
         display: flex; align-items: center; justify-content: center;";
    let mid_style = format!(
        "padding: 4px 10px; min-width: 30px; text-align: center; \
         font-size: 12.5px; color: rgba(232,234,238,0.96); \
         font-feature-settings: \"tnum\" 1; font-variant-numeric: tabular-nums; \
         border-left: 1px solid {line}; border-right: 1px solid {line};",
        line = theme::LINE,
    );
    let unit_style = format!(
        "padding: 4px 10px; font-size: 11px; \
         color: rgba(232,234,238,0.42); align-self: center; \
         border-left: 1px solid {line};",
        line = theme::LINE,
    );
    let stroke = "rgba(232,234,238,0.62)".to_string();
    let sz = 12.0_f32;
    let sw = 1.6_f32;
    let value_str = value.to_string();

    rsx! {
        div { style: {wrap_style.clone()},
            button { r#type: "button", style: {seg_style.to_string()},
                Icon { glyph: "minus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            }
            div { style: {mid_style.clone()}, {value_str.clone()} }
            button { r#type: "button", style: {seg_style.to_string()},
                Icon { glyph: "plus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            }
            div { style: {unit_style.clone()}, {unit.clone()} }
        }
    }
}

#[component]
fn Select(value: String) -> NodeHandle {
    let wrap_style = format!(
        "display: flex; align-items: center; padding: 5px 10px; \
         border-radius: 4px; background: {bg0}; border: 1px solid {line}; \
         font-size: 12px; color: rgba(232,234,238,0.96);",
        bg0 = theme::BG0, line = theme::LINE,
    );
    let stroke = "rgba(232,234,238,0.42)".to_string();
    let sz = 12.0_f32;
    let sw = 1.6_f32;
    rsx! {
        div { style: {wrap_style.clone()},
            span { style: "flex: 1;", {value.clone()} }
            Icon { glyph: "chevron-d", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
        }
    }
}

#[component]
fn ChordLoopRow(bar_range: String, loop_name: String, color: String) -> NodeHandle {
    let r = fixture::round1();
    let romans = fixture::chord_loop_by_name(&r, loop_name.as_str())
        .map(|c| c.events.iter().map(|e| e.roman).collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    let row_style = format!(
        "display: flex; align-items: center; gap: 8px; \
         padding: 6px 8px; border-radius: 4px; \
         background: {bg0}; border: 1px solid {line};",
        bg0 = theme::BG0, line = theme::LINE,
    );
    let swatch_style = format!(
        "width: 8px; height: 8px; border-radius: 2px; \
         background: {col}; flex: 0 0 auto;",
        col = color,
    );
    let range_style = "font-size: 11px; color: rgba(232,234,238,0.42); \
         font-feature-settings: \"tnum\" 1; min-width: 30px;";
    let name_style = "flex: 1; font-size: 12px; color: rgba(232,234,238,0.96);";
    let roman_style = "font-size: 11px; color: rgba(232,234,238,0.42); \
         letter-spacing: 0.4px; font-feature-settings: \"tnum\" 1;";

    rsx! {
        div { style: {row_style.clone()},
            span { style: {swatch_style.clone()} }
            span { style: {range_style.to_string()}, {bar_range.clone()} }
            span { style: {name_style.to_string()}, {loop_name.clone()} }
            span { style: {roman_style.to_string()}, {romans.clone()} }
        }
    }
}

#[component]
fn AddRow(label: String) -> NodeHandle {
    let btn_style = format!(
        "display: flex; align-items: center; gap: 6px; \
         padding: 4px 8px; border-radius: 3px; \
         background: transparent; border: 1px dashed {line}; \
         color: rgba(232,234,238,0.42); cursor: pointer; font-size: 11.5px;",
        line = theme::LINE,
    );
    let stroke = "rgba(232,234,238,0.42)".to_string();
    let sz = 11.0_f32;
    let sw = 1.6_f32;
    rsx! {
        button {
            r#type: "button",
            style: {btn_style.clone()},
            Icon { glyph: "plus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            span { {label.clone()} }
        }
    }
}


// ─── Footer ───────────────────────────────────────────────────────────────

#[component]
fn InspectorFooter() -> NodeHandle {
    let footer_style = format!(
        "flex: 0 0 auto; padding: 10px 14px; \
         border-top: 1px solid {line}; \
         display: flex; gap: 6px;",
        line = theme::LINE,
    );
    rsx! {
        div { style: {footer_style.clone()},
            FooterBtn { label: "Duplicate placement", primary: false }
            FooterBtn { label: "Open in editor",     primary: true  }
        }
    }
}

#[component]
fn FooterBtn(label: String, primary: bool) -> NodeHandle {
    let bg = if primary { theme::BG3 } else { theme::BG2 };
    let fg = if primary {
        "rgba(232,234,238,0.96)"
    } else {
        "rgba(232,234,238,0.62)"
    };
    let style = format!(
        "flex: 1; padding: 6px 10px; border-radius: 4px; \
         background: {bg}; border: 1px solid {line}; color: {fg}; \
         font-size: 12px; font-weight: 500; cursor: pointer;",
        line = theme::LINE,
    );
    rsx! {
        button {
            r#type: "button",
            style: {style.clone()},
            {label.clone()}
        }
    }
}
