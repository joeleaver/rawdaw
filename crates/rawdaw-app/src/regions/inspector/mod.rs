//! Inspector (right pane). Announces what's selected and offers the
//! most-common edits inline.
//!
//! Translates `docs/design/mockups/round-1/components/inspector.jsx`.
//! Component props pass *primitive owned* data (`String`, `u32`,
//! `bool`) rather than the full `Section`/`SectionRef` structs. The
//! macro requires every field type to implement `Default`, and
//! threading `Default` through every fixture struct + nested type
//! would balloon. Doing one fixture lookup at the `SelectedInspector`
//! boundary and passing primitives down also matches the way a real
//! `Project` will be navigated — by id rather than by reference.
//!
//! ## Three selection states
//!
//! Inspector reads both selection axes off [`AppState`] and branches:
//!
//! 1. `selected_idx = Some(_)` — render the section/block detail (the
//!    round-1 behavior; section-block selection takes precedence over
//!    track selection so existing UX is preserved).
//! 2. `selected_idx = None`, `selected_track = Some(_)` — render the
//!    [`SynthEditor`] for the selected track (U4+).
//! 3. Both `None` — render the empty state.
//!
//! The pane width grows from 320 px to 600 px when the synth editor
//! is active (rule 14 of the rinch skill — the `style:` closure is
//! itself a Fn effect, so it re-runs on selection-signal changes).
//! The parent (`ArrangementSurface`) re-mounts the whole Inspector
//! component on every selection change via its keyed for-loop, so
//! the inner subtree always starts fresh — the reactive width still
//! handles the per-mount initial paint.

mod activation_table;
mod drum_editor;
mod master_fx_editor;
mod matrix_editor;
mod preset_dropdown;
mod soft_clip_editor;
mod synth_editor;
mod wavetable_editor;

use rinch::prelude::*;

use rawdaw_model::chord::ChordSpec;
use rawdaw_model::id::{SectionId, VariantId};
use rawdaw_model::time::PPQ;

use crate::chord_display::roman_label;
use crate::parts::{Icon, StripePaper};
use crate::state::AppState;
use crate::theme;

use activation_table::ActivationTable;
use master_fx_editor::MasterFxEditor;
use synth_editor::SynthEditor;

/// Beats per bar baked into the round-1 fixture — mirrors the
/// arrangement module's constant. Switches to the model
/// `tempo_map.beats_per_bar_at(...)` lookup when the arrangement
/// honors mid-arrangement time-signature changes.
const BEATS_PER_BAR: u32 = 4;

/// Inspector — branches on (`selected_idx`, `selected_track`) and
/// renders one of three subtrees. The pane width is reactive: 600 px
/// while the synth editor is active, 320 px otherwise.
///
/// Parent (`ArrangementSurface`) re-mounts this component on every
/// selection change via a keyed for-loop, so the inner match arm is
/// evaluated once per mount. The reactive style closure on `aside`
/// still handles per-mount initial paint via `pane_style`.
///
/// Uses an `if`/`else if` chain over the three states rather than an
/// rsx `match` — the macro's match-arm codegen treats each pattern
/// binding as if it were behind a `Fn` closure and refuses to forward
/// non-`Copy` bindings to component props. Routing each state through
/// a `bool` + sentinel mirrors the pattern that
/// [`InspectorEmpty`]/[`SelectedInspector`] already uses.
#[component]
pub fn Inspector() -> NodeHandle {
    let app = use_store::<AppState>();
    let section_sel = app.selected_idx.get();
    let track_sel = app.selected_track.get();
    let master_fx_sel = app.selected_master_fx.get();

    let is_section = section_sel.is_some();
    let is_track = !is_section && track_sel.is_some();
    let is_master_fx = !is_section && !is_track && master_fx_sel.is_some();
    let section_idx = section_sel.unwrap_or(usize::MAX);
    let track_idx = track_sel.unwrap_or(usize::MAX);
    let master_fx_slot = master_fx_sel.unwrap_or(usize::MAX);

    rsx! {
        aside {
            style: {|| pane_style(synth_mode_active())},
            if is_section {
                SelectedInspector { idx: section_idx }
            } else if is_track {
                SynthEditor { track_idx: track_idx }
            } else if is_master_fx {
                MasterFxEditor { slot: master_fx_slot }
            } else {
                InspectorEmpty { }
            }
        }
    }
}

fn synth_mode_active() -> bool {
    // The synth editor and the master-FX editor both want the wider
    // 600px pane (more knobs, more screen real estate); the section
    // / empty states use the narrower default. Mirrors U4's
    // pane-widening contract.
    let app = use_store::<AppState>();
    let no_section = app.selected_idx.get().is_none();
    no_section
        && (app.selected_track.get().is_some() || app.selected_master_fx.get().is_some())
}

fn pane_style(wide: bool) -> String {
    let width = if wide { 600 } else { 320 };
    format!(
        "width: {width}px; flex: 0 0 {width}px; \
         background: {bg}; border-left: 1px solid {line}; \
         display: flex; flex-direction: column; min-height: 0;",
        bg = theme::BG1,
        line = theme::LINE,
    )
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
    rsx! {
        div { style: {wrap_style.to_string()},
            div { style: {glyph_style.to_string()},
                Icon { glyph: "dot", size: 28.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
            }
            div { style: {copy_style.to_string()},
                "Select a section, pattern, or chord to inspect."
            }
        }
    }
}

#[component]
fn SelectedInspector(idx: usize) -> NodeHandle {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let overlay = app.overlay.get();

    let Some(section_ref) = project.arrangement.sections.get(idx).cloned() else {
        return rsx! { InspectorEmpty { } };
    };
    let Some(section) = project.sections.get(&section_ref.section).cloned() else {
        return rsx! { InspectorEmpty { } };
    };

    // Pre-resolve everything subcomponents will need into primitive
    // props (every `#[component]` field type must impl `Default`).
    let instance_count = project
        .arrangement
        .sections
        .iter()
        .filter(|sr| sr.section == section_ref.section)
        .count();
    let section_color = overlay
        .section_color
        .get(&section_ref.section)
        .cloned()
        .unwrap_or_else(|| theme::TEXT2.to_string());
    let section_name = section.name.clone();
    let variant_id = section_ref.variant.as_str().to_string();
    let default_variant = section.default_variant.as_str().to_string();
    let bars = section
        .variants
        .get(&section_ref.variant)
        .and_then(|v| v.duration_bars)
        .unwrap_or(section.base.duration_bars);
    let base_duration_bars = section.base.duration_bars;
    let start_bar = (section_ref.start.as_ticks() / (PPQ * BEATS_PER_BAR as i64)).max(0) as u32;
    let (chord_loop_name, chord_loop_color) = section
        .base
        .chord_loops
        .first()
        .and_then(|(_, clid)| project.chord_loops.get(clid).map(|cl| (cl.name.clone(), *clid)))
        .map(|(name, clid)| {
            let color = overlay
                .chord_loop_color
                .get(&clid)
                .cloned()
                .unwrap_or_else(|| theme::PAL_TERRA.to_string());
            (name, color)
        })
        .unwrap_or_else(|| (String::new(), theme::PAL_TERRA.to_string()));

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
                variant_id: variant_id.clone(),
                base_duration_bars: base_duration_bars,
                chord_loop_name: chord_loop_name,
                chord_loop_color: chord_loop_color,
            }
            InspectorFooter {
                section_id: section_ref.section.get(),
                variant_id: variant_id,
            }
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
    // The header uses `StripePaper` for its left-stripe + tinted-body
    // chrome. The bottom border (which the primitive doesn't carry,
    // since it's not a structural concern of the stripe pattern)
    // continues to come from the parent's container — see the
    // `border-bottom` on the inspector frame itself. Phase 4 of the
    // round-2 port introduced this primitive.
    let bg = with_alpha(section_color.as_str(), 0.06);
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

    // Wrap StripePaper in an outer div that owns the bottom border —
    // StripePaper is a self-contained card primitive; the divider
    // between header and the rest of the inspector is a parent
    // concern.
    let wrapper_style = format!("border-bottom: 1px solid {line};", line = theme::LINE);

    rsx! {
        div { style: {wrapper_style.clone()},
            StripePaper {
                stripe_color: section_color.clone(),
                background: bg,
                padding: "12px 14px".to_string(),
                radius: 0.0,
                div { style: "display: flex; flex-direction: column; gap: 6px;",
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
        bg = with_alpha(color.as_str(), 0.16),
        border = with_alpha(color.as_str(), 0.30),
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
            for v in variant_options_for_section(key.clone()) {
                VariantTab {
                    key: v.id.clone(),
                    variant_id: v.id.clone(),
                    variant_name: v.name,
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

/// One variant tab payload — owned strings so the rsx `for` source
/// closure can be `Fn` (returns a fresh `Vec` per call).
#[derive(Clone, PartialEq)]
pub(crate) struct VariantOption {
    pub id: String,
    pub name: String,
}

/// Variants surface as: the base "main"/"base" tab plus every named
/// variant override on the section. Caller passes the section's `name`
/// string (back-compat with the section_key plumbing in
/// `EditorMode::SectionEditor`); migration to `SectionId` lookups is
/// the next C1c slice.
pub(crate) fn variant_options_for_section(name: String) -> Vec<VariantOption> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(section) = project.sections.values().find(|s| s.name == name) else {
        return Vec::new();
    };
    let default_id = section.default_variant.as_str().to_string();
    let mut out = vec![VariantOption {
        id: default_id.clone(),
        name: default_id,
    }];
    for variant_id in section.variants.keys() {
        let id = variant_id.as_str().to_string();
        out.push(VariantOption {
            id: id.clone(),
            name: id,
        });
    }
    out
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
    rsx! {
        button {
            r#type: "button",
            style: {btn_style.to_string()},
            Icon { glyph: "plus", size: 11.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
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
    let value_str = value.to_string();

    rsx! {
        div { style: {wrap_style.clone()},
            button { r#type: "button", style: {seg_style.to_string()},
                Icon { glyph: "minus", size: 12.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
            }
            div { style: {mid_style.clone()}, {value_str.clone()} }
            button { r#type: "button", style: {seg_style.to_string()},
                Icon { glyph: "plus", size: 12.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
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
    rsx! {
        div { style: {wrap_style.clone()},
            span { style: "flex: 1;", {value.clone()} }
            Icon { glyph: "chevron-d", size: 12.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
        }
    }
}

#[component]
fn ChordLoopRow(bar_range: String, loop_name: String, color: String) -> NodeHandle {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let romans = project
        .chord_loops
        .values()
        .find(|cl| cl.name == loop_name)
        .map(|cl| {
            cl.events
                .iter()
                .filter_map(|e| match &e.chord {
                    ChordSpec::Functional { roman, suffix, .. } => {
                        Some(roman_label(*roman, &suffix.quality))
                    }
                    ChordSpec::Absolute { .. } => None,
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
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
    rsx! {
        button {
            r#type: "button",
            style: {btn_style.clone()},
            Icon { glyph: "plus", size: 11.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
            span { {label.clone()} }
        }
    }
}


// ─── Footer ───────────────────────────────────────────────────────────────

#[component]
fn InspectorFooter(section_id: u64, variant_id: String) -> NodeHandle {
    let app = use_store::<AppState>();
    let footer_style = format!(
        "flex: 0 0 auto; padding: 10px 14px; \
         border-top: 1px solid {line}; \
         display: flex; gap: 6px;",
        line = theme::LINE,
    );

    // Move id + variant into the open-editor closure as Copy / String
    // clones so the closure can be `Fn` (invoked on every click,
    // never consume the captures). `section_id` is passed as `u64`
    // (rather than `SectionId` directly) because rinch's `#[component]`
    // requires every prop type to impl Default — `u64` does;
    // wrapping happens inside the closure via `SectionId::new`. The
    // variant is also a String at the prop boundary for the same
    // reason; `VariantId::new` re-types it inside.
    let sid = SectionId::new(section_id);
    let open_variant_str = variant_id.clone();

    rsx! {
        div { style: {footer_style.clone()},
            FooterBtn { label: "Duplicate placement", primary: false, onclick: move || {} }
            FooterBtn {
                label: "Open in editor",
                primary: true,
                onclick: move || {
                    app.open_section_editor(sid, VariantId::new(open_variant_str.clone()))
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_style_widens_for_synth_mode() {
        let wide = pane_style(true);
        let narrow = pane_style(false);
        assert!(wide.contains("width: 600px"), "wide: {wide}");
        assert!(wide.contains("flex: 0 0 600px"), "wide: {wide}");
        assert!(narrow.contains("width: 320px"), "narrow: {narrow}");
        assert!(narrow.contains("flex: 0 0 320px"), "narrow: {narrow}");
    }
}

#[component]
fn FooterBtn(label: String, primary: bool, onclick: Callback) -> NodeHandle {
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
            onclick: move || onclick.invoke(),
            {label.clone()}
        }
    }
}
