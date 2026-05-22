//! Section meta bar: Duration · Scale override · Chord loops.
//!
//! Mirrors round-2's `SectionMetaBar`. Three fields in a 200/240/1fr
//! grid; each shows a `↳ base` inheritance pill when the current
//! variant doesn't override that field.
//!
//! S3 (2026-05-22) wires Duration + Scale to the section_actions edit
//! helpers. Duration: numeric nudger (− / +) writing through
//! `set_section_duration_bars(id, current_variant, new_bars)` — base
//! tab targets `section.base.duration_bars` directly, non-default
//! variants auto-promote a `SectionVariantOverride` per S0 decision
//! 3. Scale: three-way `Select` ("inherit base" / "no override" /
//! "use scale …"). The chord-loops surface stays as today (CL4 +
//! CL4.x cover that flow).

use rinch::prelude::*;

use rawdaw_model::id::SectionId;
use rawdaw_model::pitch::PitchClass;
use rawdaw_model::scale::{Mode, Scale};

use crate::parts::Icon;
use crate::section_actions::{
    clear_variant_scale_override, set_section_duration_bars, set_section_scale_override,
};
use crate::section_editor::chord_loop_bar::ChordLoopBar;
use crate::state::{AppState, EditorMode};
use crate::theme;

#[component]
pub fn SectionMetaBar(section_id: u64) -> NodeHandle {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let sid = SectionId::new(section_id);
    let section = project
        .sections
        .get(&sid)
        .expect("section editor target must exist in the project");

    let default_variant = section.default_variant.as_str().to_string();
    let section_name_key = section.name.clone();
    let duration = section.base.duration_bars;

    let bar_style = format!(
        "flex: 0 0 auto; padding: 12px 20px; \
         border-bottom: 1px solid {line}; \
         display: grid; grid-template-columns: 200px 240px 1fr; \
         gap: 18px; align-items: stretch;",
        line = theme::LINE,
    );

    rsx! {
        div { style: {bar_style.clone()},
            DurationField  { section_id: section_id, default_variant: default_variant.clone() }
            ScaleField     { section_id: section_id, default_variant: default_variant.clone() }
            ChordLoopsField { default_variant: default_variant.clone(),
                              section_name_key: section_name_key,
                              duration_bars: duration }
        }
    }
}

// ─── Per-field components ─────────────────────────────────────────────────

#[component]
fn DurationField(section_id: u64, default_variant: String) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 5px;",
            FieldLabelRow { label: "Duration",
                            default_variant: default_variant,
                            action_label: "" }
            NumStepperMini {
                section_id: section_id,
                unit: "bars".to_string(),
            }
        }
    }
}

#[component]
fn ScaleField(section_id: u64, default_variant: String) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 5px;",
            FieldLabelRow { label: "Scale override",
                            default_variant: default_variant,
                            action_label: "" }
            ScalePicker { section_id: section_id }
        }
    }
}

#[component]
fn ChordLoopsField(
    default_variant: String,
    section_name_key: String,
    duration_bars: u32,
) -> NodeHandle {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 5px;",
            FieldLabelRow { label: "Chord loops",
                            default_variant: default_variant,
                            action_label: "Loop range" }
            ChordLoopBar { section_name_key: section_name_key, duration_bars: duration_bars }
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
    let default_for_check = default_variant.clone();

    rsx! {
        div { style: {label_row_style.clone()},
            span { {label.clone()} }
            if matches!(
                app.editor_mode.get(),
                EditorMode::SectionEditor { ref variant, .. }
                    if variant.as_str() != default_for_check.as_str()
            ) {
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

/// Numeric nudger wired to `set_section_duration_bars`. Reads the
/// effective duration each render from the store: on the base tab,
/// `section.base.duration_bars`; on a non-default variant, the
/// override's `duration_bars` if `Some`, else inherits the base.
///
/// −/+ buttons commit through `apply_project_edit` so the engine
/// re-arms in lockstep. Clamps the floor at 1 bar.
#[component]
fn NumStepperMini(section_id: u64, unit: String) -> NodeHandle {
    let sid = SectionId::new(section_id);

    let wrap_style = format!(
        "display: inline-flex; align-items: stretch; \
         background: {bg0}; border: 1px solid {line}; border-radius: 4px; \
         width: fit-content;",
        bg0 = theme::BG0,
        line = theme::LINE,
    );
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

    rsx! {
        div { style: {wrap_style.clone()},
            button {
                r#type: "button",
                style: {seg_btn_style.to_string()},
                title: "Decrease duration",
                onclick: move || nudge_duration(sid, -1),
                Icon { glyph: "minus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            }
            div { style: {mid_style.clone()},
                {|| effective_duration(sid).to_string()}
            }
            button {
                r#type: "button",
                style: {seg_btn_style.to_string()},
                title: "Increase duration",
                onclick: move || nudge_duration(sid, 1),
                Icon { glyph: "plus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            }
            div { style: {unit_style.clone()}, {unit.clone()} }
        }
    }
}

/// Read the effective duration for this section under the active
/// variant tab. Mirrors `section_actions::duration` semantics. Looks
/// up the section's `default_variant` from the live project so the
/// reactive closure that drives the display reads only `sid: Copy`
/// from outer scope.
fn effective_duration(sid: SectionId) -> u32 {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(section) = project.sections.get(&sid) else {
        return 0;
    };
    let active = match app.editor_mode.get() {
        EditorMode::SectionEditor { variant, .. } => variant,
        EditorMode::Arrangement => section.default_variant.clone(),
    };
    if active == section.default_variant {
        return section.base.duration_bars;
    }
    section
        .variants
        .get(&active)
        .and_then(|o| o.duration_bars)
        .unwrap_or(section.base.duration_bars)
}

/// Apply a ± nudge to the section's duration under the active variant
/// tab. Clamps the result at 1 bar.
fn nudge_duration(sid: SectionId, delta: i32) {
    let app = use_store::<AppState>();
    let active = match app.editor_mode.get() {
        EditorMode::SectionEditor { variant, .. } => variant,
        EditorMode::Arrangement => return,
    };
    let current = effective_duration(sid);
    let next = (current as i32 + delta).max(1) as u32;
    if next == current {
        return;
    }
    let active_for_edit = active.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        set_section_duration_bars(p, sid, &active_for_edit, next);
    }) {
        eprintln!("section editor: set duration failed: {e}");
    }
}

// ─── Scale picker ──────────────────────────────────────────────────────────

/// Three-way scale picker:
/// - "Inherit base" — only available on non-default variants;
///   writes `clear_variant_scale_override` to drop the override field
///   to `None`.
/// - "No scale override" — writes `Some(None)` on a non-default variant
///   or `None` on the base tab.
/// - "Use scale …" — concrete scale; writes `Some(Some(scale))` on
///   non-default or `Some(scale)` on base.
#[component]
fn ScalePicker(section_id: u64) -> NodeHandle {
    let sid = SectionId::new(section_id);
    // Encode the picker's current value into the Select's `key` so a
    // model edit elsewhere remounts the Select with the fresh choice.
    // (Pattern from `pattern_select.rs` PatternOptions::key.) The
    // section's `default_variant` is looked up inside the helpers so
    // the closures only capture `sid: Copy`.
    rsx! {
        for opts in scale_picker_options(sid) {
            Select {
                key: opts.key.clone(),
                size: "sm",
                value: {opts.current_value.clone()},
                data: opts.options.clone(),
                onchange: move |v: String| {
                    commit_scale_pick(sid, v);
                },
            }
        }
    }
}

#[derive(Clone, PartialEq, Default)]
struct ScalePickerOptions {
    key: String,
    current_value: String,
    options: Vec<SelectOption>,
}

fn scale_picker_options(sid: SectionId) -> Vec<ScalePickerOptions> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(section) = project.sections.get(&sid) else {
        return vec![ScalePickerOptions::default()];
    };
    let on_default = match app.editor_mode.get() {
        EditorMode::SectionEditor { ref variant, .. } => {
            variant == &section.default_variant
        }
        EditorMode::Arrangement => true,
    };

    // Base tab: "no override" + concrete scales. Variant tab: also
    // "inherit base" at the top.
    let mut options: Vec<SelectOption> = Vec::new();
    if !on_default {
        options.push(SelectOption::new("inherit", "Inherit base"));
    }
    options.push(SelectOption::new("none", "No scale override"));
    let roots: [(PitchClass, &str); 12] = [
        (PitchClass::C, "C"),
        (PitchClass::CSharp, "C#"),
        (PitchClass::D, "D"),
        (PitchClass::DSharp, "D#"),
        (PitchClass::E, "E"),
        (PitchClass::F, "F"),
        (PitchClass::FSharp, "F#"),
        (PitchClass::G, "G"),
        (PitchClass::GSharp, "G#"),
        (PitchClass::A, "A"),
        (PitchClass::ASharp, "A#"),
        (PitchClass::B, "B"),
    ];
    for (_, name) in roots {
        options.push(SelectOption::new(
            format!("{name}:major"),
            format!("{name} major"),
        ));
    }
    for (_, name) in roots {
        options.push(SelectOption::new(
            format!("{name}:minor"),
            format!("{name} minor"),
        ));
    }
    let current_value = scale_picker_current_value(sid);
    let key = format!("scale-{}-{current_value}", sid.get());
    vec![ScalePickerOptions {
        key,
        current_value,
        options,
    }]
}

fn parse_pitch_class(name: &str) -> Option<PitchClass> {
    match name {
        "C" => Some(PitchClass::C),
        "C#" => Some(PitchClass::CSharp),
        "D" => Some(PitchClass::D),
        "D#" => Some(PitchClass::DSharp),
        "E" => Some(PitchClass::E),
        "F" => Some(PitchClass::F),
        "F#" => Some(PitchClass::FSharp),
        "G" => Some(PitchClass::G),
        "G#" => Some(PitchClass::GSharp),
        "A" => Some(PitchClass::A),
        "A#" => Some(PitchClass::ASharp),
        "B" => Some(PitchClass::B),
        _ => None,
    }
}

fn pitch_class_name(pc: PitchClass) -> &'static str {
    match pc {
        PitchClass::C => "C",
        PitchClass::CSharp => "C#",
        PitchClass::D => "D",
        PitchClass::DSharp => "D#",
        PitchClass::E => "E",
        PitchClass::F => "F",
        PitchClass::FSharp => "F#",
        PitchClass::G => "G",
        PitchClass::GSharp => "G#",
        PitchClass::A => "A",
        PitchClass::ASharp => "A#",
        PitchClass::B => "B",
    }
}

fn scale_to_value(scale: &Scale) -> String {
    let kind = match scale.mode {
        Mode::Ionian => "major",
        Mode::Aeolian => "minor",
        _ => "major", // collapse other modes to major for the v1 picker
    };
    format!("{}:{kind}", pitch_class_name(scale.tonic))
}

fn scale_picker_current_value(sid: SectionId) -> String {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(section) = project.sections.get(&sid) else {
        return "inherit".into();
    };
    let active = match app.editor_mode.get() {
        EditorMode::SectionEditor { variant, .. } => variant,
        EditorMode::Arrangement => section.default_variant.clone(),
    };
    if active == section.default_variant {
        // Base path: Some(scale) → that scale, None → "none"
        return match &section.base.scale_override {
            Some(s) => scale_to_value(s),
            None => "none".into(),
        };
    }
    // Variant path: None → inherit, Some(None) → none, Some(Some(s)) → scale
    let over = section.variants.get(&active);
    match over.and_then(|o| o.scale_override.as_ref()) {
        None => "inherit".into(),
        Some(None) => "none".into(),
        Some(Some(s)) => scale_to_value(s),
    }
}

fn commit_scale_pick(sid: SectionId, value: String) {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(section) = project.sections.get(&sid) else { return };
    let active = match app.editor_mode.get() {
        EditorMode::SectionEditor { variant, .. } => variant,
        EditorMode::Arrangement => return,
    };
    let is_default = active == section.default_variant;
    drop(project);
    if value == "inherit" {
        // Only meaningful on non-default; clear the override field to None.
        if !is_default {
            let v = active.clone();
            if let Err(e) = app.apply_project_edit(move |p| {
                clear_variant_scale_override(p, sid, &v);
            }) {
                eprintln!("section editor: clear scale override failed: {e}");
            }
        }
        return;
    }
    if value == "none" {
        let v = active.clone();
        if let Err(e) = app.apply_project_edit(move |p| {
            set_section_scale_override(p, sid, &v, None);
        }) {
            eprintln!("section editor: set scale override (none) failed: {e}");
        }
        return;
    }
    // "<root>:<kind>"
    let (root_name, kind) = match value.split_once(':') {
        Some(parts) => parts,
        None => return,
    };
    let Some(root) = parse_pitch_class(root_name) else {
        return;
    };
    let scale = match kind {
        "major" => Scale::major(root),
        "minor" => Scale::natural_minor(root),
        _ => return,
    };
    let v = active.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        set_section_scale_override(p, sid, &v, Some(scale.clone()));
    }) {
        eprintln!("section editor: set scale override failed: {e}");
    }
}
