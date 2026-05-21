//! Pattern editor — P2 of `docs/pattern-editor-plan.md`.
//!
//! Center-stage editor that mounts inside `ArrangementSurface` when
//! `AppState::selected_pattern` is `Some`. Dispatches on
//! `PatternBody::Pitched` vs `Drum`:
//!
//! - `Pitched` → [`pitched::PitchedPatternEditor`] — piano-roll
//!   surface + per-note inspector + realized strip.
//! - `Drum` → P3 deferral banner; the step-grid surface lands when
//!   `docs/pattern-editor-plan.md` § P3 lands.
//!
//! File layout per P2 design decision 10 — split eagerly:
//! - `mod.rs` (this file) — top-level surface + editor header.
//! - `pitched/` — pitched-pattern editor + helpers + inspector.
//! - `drum/` — step-grid editor (P3).

pub mod pitched;

use rinch::core::reactive::{untracked, Effect};
use rinch::prelude::*;

use rawdaw_model::id::{PatternId, VariantId};
use rawdaw_model::pattern::{Pattern, PatternBody};
use rawdaw_model::time::{Duration, MusicalTime};

use crate::pattern_actions::{
    create_variant, delete_variant, duplicate_variant, rename_pattern,
    set_pitched_pattern_length, VariantEditError,
};
use crate::state::AppState;
use crate::theme;

pub use pitched::PitchedPatternEditor;

/// Center-stage pattern editor. Mounts inside the arrangement surface
/// when `AppState::selected_pattern` is `Some`. The caller (the
/// `ArrangementSurface` component in `app.rs`) does the mount/unmount;
/// this component renders the surface for the selected id.
#[component]
pub fn PatternEditor(id: PatternId) -> NodeHandle {
    let surface_style = format!(
        "flex: 1; min-width: 0; display: flex; flex-direction: column; \
         background: {bg}; min-height: 0;",
        bg = theme::BG0,
    );

    rsx! {
        section { style: {surface_style.clone()},
            EditorHeader { id: id }
            VariantTabs { id: id }
            // Body dispatch — the `for` source over a single-element
            // keyed vec forces a remount on body-kind change, and
            // its body closure receives `body_kind` by value (no
            // lifetime issues that a bare `match` on a Signal read
            // would hit when capturing into multiple component
            // closures).
            for body_code in pattern_body_kind_codes(id) {
                BodyDispatch { kind_code: body_code, pattern_id: id }
            }
        }
    }
}

#[component]
fn BodyDispatch(pattern_id: PatternId, kind_code: u8) -> NodeHandle {
    rsx! {
        match kind_code {
            0 => PitchedPatternEditor { id: pattern_id },
            1 => DrumDeferralBanner { },
            _ => EmptyPatternBanner { },
        }
    }
}

/// Single-element key vector used to keep the body dispatch reactive
/// to the pattern's body kind. Returns `0` for Pitched, `1` for
/// Drum, `2` for missing pattern. `u8` keeps the match scrutinee
/// `Copy` so the rsx-wrapped Effect closure doesn't fight String
/// captures.
fn pattern_body_kind_codes(id: PatternId) -> Vec<u8> {
    let project = use_store::<AppState>().project.get();
    let code = match project.patterns.get(&id) {
        Some(p) => match &p.body {
            PatternBody::Pitched(_) => 0u8,
            PatternBody::Drum(_) => 1u8,
        },
        None => 2u8,
    };
    vec![code]
}

/// Header bar — pattern name (inline rename), length editor, close.
/// Variant tabs live in the row beneath in [`VariantTabs`].
#[component]
fn EditorHeader(id: PatternId) -> NodeHandle {
    let name_buffer = Signal::new(String::new());
    let renaming = Signal::new(false);

    // Sync the name buffer from the live project. Same untracked-
    // Effect peek pattern as the chord-loop editor's EditorHeader.
    let _ = Effect::new(move || {
        let project = use_store::<AppState>().project.get();
        let canonical = project
            .patterns
            .get(&id)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let display = untracked(|| name_buffer.get());
        if display == canonical {
            return;
        }
        name_buffer.set(canonical);
    });

    let header_style = format!(
        "display: flex; align-items: center; gap: 12px; \
         padding: 8px 14px; border-bottom: 1px solid {line}; \
         background: {bg1}; min-height: 36px;",
        line = theme::LINE,
        bg1 = theme::BG1,
    );

    rsx! {
        header { style: {header_style.clone()},
            span {
                style: "font-size: 10px; letter-spacing: 0.6px; \
                        text-transform: uppercase; \
                        color: rgba(232,234,238,0.42); font-weight: 600;",
                "Pattern"
            }
            if renaming.get() {
                TextInput {
                    size: "sm",
                    value_fn: move || name_buffer.get(),
                    oninput: move |v: String| name_buffer.set(v),
                    onsubmit: move || commit_name_edit(id, name_buffer, renaming),
                }
            } else {
                button {
                    r#type: "button",
                    title: "Click to rename",
                    style: {header_name_btn_style()},
                    onclick: move || renaming.set(true),
                    {|| name_buffer.get()}
                }
            }
            PatternLengthControls { id: id }
            div { style: "flex: 1;" }
            button {
                r#type: "button",
                title: "Close (×)",
                style: {close_btn_style()},
                onclick: move || use_store::<AppState>().select_pattern(None),
                "×"
            }
        }
    }
}

#[component]
fn DrumDeferralBanner() -> NodeHandle {
    let style = format!(
        "flex: 1; display: flex; flex-direction: column; \
         align-items: center; justify-content: center; gap: 10px; \
         padding: 24px; background: {bg0}; color: {text1};",
        bg0 = theme::BG0,
        text1 = theme::TEXT1,
    );
    rsx! {
        div { style: {style.clone()},
            div {
                style: "font-size: 14px; font-weight: 600; letter-spacing: 0.2px;",
                "Drum pattern editor"
            }
            div {
                style: "font-size: 12px; color: rgba(232,234,238,0.55); max-width: 320px; \
                        text-align: center; line-height: 1.5;",
                "The drum step-grid editor (P3) is not yet implemented. \
                 The pattern's events still realize through the engine \
                 when bound to a track via an activation."
            }
        }
    }
}

#[component]
fn EmptyPatternBanner() -> NodeHandle {
    let style = format!(
        "flex: 1; display: flex; align-items: center; justify-content: center; \
         padding: 24px; background: {bg0}; color: rgba(232,234,238,0.55);",
        bg0 = theme::BG0,
    );
    rsx! {
        div { style: {style.clone()},
            div {
                style: "font-size: 12px;",
                "Pattern not found."
            }
        }
    }
}

// ─── Variant tabs ─────────────────────────────────────────────────────────

/// Variant tab strip. One tab per variant in the pattern. Click to
/// switch; the rightmost `+` creates a new variant; per-tab buttons
/// rename / duplicate / delete. Default variant is bolded.
#[component]
fn VariantTabs(id: PatternId) -> NodeHandle {
    let bar_style = format!(
        "display: flex; align-items: center; gap: 6px; flex-wrap: wrap; \
         padding: 6px 14px; border-bottom: 1px solid {line}; \
         background: {bg1};",
        line = theme::LINE,
        bg1 = theme::BG1,
    );

    rsx! {
        div { style: {bar_style.clone()},
            for tab in build_variant_tabs(id) {
                VariantTab {
                    key: tab.variant_label.clone(),
                    pattern_id: id,
                    variant_label: tab.variant_label.clone(),
                    is_default: tab.is_default,
                }
            }
            button {
                r#type: "button",
                title: "Add a new variant",
                style: {add_variant_btn_style()},
                onclick: move || add_variant_action(id),
                "+ Variant"
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct VariantTabModel {
    variant_label: String,
    is_default: bool,
}

fn build_variant_tabs(id: PatternId) -> Vec<VariantTabModel> {
    let project = use_store::<AppState>().project.get();
    let Some(pattern) = project.patterns.get(&id) else {
        return Vec::new();
    };
    let default = pattern.default_variant.clone();
    let variants: Vec<VariantId> = match &pattern.body {
        PatternBody::Pitched(body) => body.variants.keys().cloned().collect(),
        PatternBody::Drum(body) => body.variants.keys().cloned().collect(),
    };
    variants
        .into_iter()
        .map(|v| VariantTabModel {
            is_default: v == default,
            variant_label: v.0,
        })
        .collect()
}

#[component]
fn VariantTab(pattern_id: PatternId, variant_label: String, is_default: bool) -> NodeHandle {
    // Each closure below moves its own `Rc<String>` clone, so the
    // String storage is shared cheaply while the closures stay
    // `FnMut`-compatible (rinch's `create_effect` requires it). The
    // closure body's `(*label).clone()` is a `&String -> String`
    // call that doesn't consume the captured Rc.
    let label = std::rc::Rc::new(variant_label.clone());
    let label_for_style = std::rc::Rc::clone(&label);
    let label_for_click = std::rc::Rc::clone(&label);
    let label_for_dup = std::rc::Rc::clone(&label);
    let label_for_del = std::rc::Rc::clone(&label);
    rsx! {
        div {
            style: "display: inline-flex; align-items: center; gap: 4px;",
            button {
                r#type: "button",
                title: "Switch to this variant",
                style: {|| {
                    let active = use_store::<AppState>().focused_variant.get()
                        == Some(VariantId::new((*label_for_style).clone()));
                    variant_tab_btn_style(active, is_default)
                }},
                onclick: move || {
                    use_store::<AppState>()
                        .focused_variant
                        .set(Some(VariantId::new((*label_for_click).clone())));
                    use_store::<AppState>().focused_pattern_note.set(None);
                },
                {variant_label.clone()}
            }
            button {
                r#type: "button",
                title: "Duplicate variant",
                style: {variant_sub_btn_style()},
                onclick: move || {
                    duplicate_variant_action(
                        pattern_id,
                        VariantId::new((*label_for_dup).clone()),
                    )
                },
                "⎘"
            }
            button {
                r#type: "button",
                title: "Delete variant",
                style: {variant_sub_btn_style()},
                onclick: move || {
                    delete_variant_action(
                        pattern_id,
                        VariantId::new((*label_for_del).clone()),
                    )
                },
                "×"
            }
        }
    }
}

fn variant_tab_btn_style(active: bool, is_default: bool) -> String {
    let bg = if active { theme::BG0 } else { theme::BG1 };
    let border = if active { theme::TEXT2 } else { theme::LINE };
    let weight = if is_default { 600 } else { 400 };
    format!(
        "height: 22px; padding: 0 8px; \
         border-radius: 3px; background: {bg}; \
         border: 1px solid {border}; color: rgba(232,234,238,0.92); \
         font-size: 11px; font-weight: {weight}; cursor: pointer;"
    )
}

fn variant_sub_btn_style() -> String {
    format!(
        "height: 22px; width: 22px; padding: 0; \
         border-radius: 3px; background: transparent; \
         border: 1px solid {line}; color: rgba(232,234,238,0.62); \
         font-size: 11px; cursor: pointer; \
         display: inline-flex; align-items: center; justify-content: center;",
        line = theme::LINE,
    )
}

fn add_variant_btn_style() -> String {
    format!(
        "height: 22px; padding: 0 8px; \
         border-radius: 3px; background: {bg0}; \
         border: 1px dashed {line}; color: rgba(232,234,238,0.72); \
         font-size: 11px; cursor: pointer; margin-left: auto;",
        bg0 = theme::BG0,
        line = theme::LINE,
    )
}

fn add_variant_action(id: PatternId) {
    let app = use_store::<AppState>();
    let chosen = std::rc::Rc::new(std::cell::Cell::new(None::<VariantId>));
    let chosen_capture = std::rc::Rc::clone(&chosen);
    if let Err(e) = app.apply_project_edit(move |p| {
        if let Some(v) = create_variant(p, id, "untitled") {
            chosen_capture.set(Some(v));
        }
    }) {
        eprintln!("pattern_editor: add variant failed: {e}");
        return;
    }
    if let Some(v) = chosen.take() {
        app.focused_variant.set(Some(v));
        app.focused_pattern_note.set(None);
    }
}

fn duplicate_variant_action(id: PatternId, source: VariantId) {
    let app = use_store::<AppState>();
    let chosen = std::rc::Rc::new(std::cell::Cell::new(None::<VariantId>));
    let chosen_capture = std::rc::Rc::clone(&chosen);
    let source_for_edit = source.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        match duplicate_variant(p, id, &source_for_edit) {
            Ok(v) => chosen_capture.set(Some(v)),
            Err(err) => eprintln!("pattern_editor: duplicate variant failed: {err:?}"),
        }
    }) {
        eprintln!("pattern_editor: duplicate variant edit failed: {e}");
        return;
    }
    if let Some(v) = chosen.take() {
        app.focused_variant.set(Some(v));
        app.focused_pattern_note.set(None);
    }
}

fn delete_variant_action(id: PatternId, variant: VariantId) {
    let app = use_store::<AppState>();
    // Cell::take requires the inner type to implement Default, which
    // Result<(), _> does not. Use RefCell instead so we can replace
    // the contents wholesale on read.
    let outcome: std::rc::Rc<std::cell::RefCell<Result<(), VariantEditError>>> =
        std::rc::Rc::new(std::cell::RefCell::new(Ok(())));
    let outcome_capture = std::rc::Rc::clone(&outcome);
    let variant_for_edit = variant.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        let res = delete_variant(p, id, &variant_for_edit);
        *outcome_capture.borrow_mut() = res;
    }) {
        eprintln!("pattern_editor: delete variant edit failed: {e}");
        return;
    }
    let taken =
        std::mem::replace(&mut *outcome.borrow_mut(), Ok::<_, VariantEditError>(()));
    if let Err(err) = taken {
        eprintln!("pattern_editor: delete variant refused: {err:?}");
        return;
    }
    // Focused variant may have been deleted; re-derive from the
    // pattern's (possibly updated) default_variant.
    let project = app.project.get();
    let new_focus = project
        .patterns
        .get(&id)
        .map(|p| p.default_variant.clone());
    app.focused_variant.set(new_focus);
    app.focused_pattern_note.set(None);
}

// ─── Length controls ──────────────────────────────────────────────────────

/// Bar-count nudge + numeric input for the pattern's length. Pitched
/// patterns only — drum-length editing is deferred to P3.
#[component]
fn PatternLengthControls(id: PatternId) -> NodeHandle {
    let bars_input = Signal::new(String::new());

    let _ = Effect::new(move || {
        let project = use_store::<AppState>().project.get();
        let canonical = project
            .patterns
            .get(&id)
            .map(|p| ticks_to_bars(p.length().as_ticks(), beats_per_bar(&project)))
            .unwrap_or(0);
        let typed = untracked(|| bars_input.get());
        if typed.parse::<u32>().ok() == Some(canonical) {
            return;
        }
        bars_input.set(canonical.to_string());
    });

    rsx! {
        div { style: "display: flex; align-items: center; gap: 4px;",
            span {
                style: "font-size: 10px; color: rgba(232,234,238,0.42); \
                        text-transform: uppercase; letter-spacing: 0.6px;",
                "Bars"
            }
            button {
                r#type: "button",
                title: "Shorten by 1 bar",
                style: {nudge_btn_style()},
                onclick: move || nudge_bars(id, -1),
                "−"
            }
            TextInput {
                size: "sm",
                value_fn: move || bars_input.get(),
                oninput: move |v: String| bars_input.set(v),
                onsubmit: move || {
                    if let Ok(bars) = bars_input.get().trim().parse::<u32>() {
                        let bars = bars.max(1);
                        set_bars(id, bars);
                        bars_input.set(bars.to_string());
                    }
                },
            }
            button {
                r#type: "button",
                title: "Lengthen by 1 bar",
                style: {nudge_btn_style()},
                onclick: move || nudge_bars(id, 1),
                "+"
            }
        }
    }
}

fn beats_per_bar(project: &rawdaw_model::project::Project) -> u32 {
    project.tempo_map.beats_per_bar_at(MusicalTime::ZERO)
}

fn ticks_to_bars(ticks: i64, beats_per_bar: u32) -> u32 {
    let ticks_per_bar = rawdaw_model::time::PPQ * beats_per_bar.max(1) as i64;
    (ticks / ticks_per_bar).max(0) as u32
}

fn nudge_bars(id: PatternId, delta: i32) {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let bpb = beats_per_bar(&project);
    let current = project
        .patterns
        .get(&id)
        .map(|p| ticks_to_bars(p.length().as_ticks(), bpb))
        .unwrap_or(0);
    let next = (current as i32 + delta).max(1) as u32;
    set_bars(id, next);
}

fn set_bars(id: PatternId, bars: u32) {
    let app = use_store::<AppState>();
    if let Err(e) = app.apply_project_edit(move |p| {
        let bpb = beats_per_bar(p);
        // Only pitched patterns mutate via the pitched-length helper;
        // drum-length nudging is deferred to P3 — silently ignored
        // here. UI v1 doesn't render the controls for drum patterns
        // through the same widget (a later polish pass could
        // disable / hide the input).
        set_pitched_pattern_length(p, id, Duration::bars(bars as i64, bpb));
    }) {
        eprintln!("pattern_editor: set pattern length failed: {e}");
    }
}

// ─── Inline editors / styles ──────────────────────────────────────────────

fn header_name_btn_style() -> String {
    format!(
        "height: 24px; padding: 0 8px; \
         border-radius: 4px; background: {bg0}; border: 1px solid {line}; \
         color: rgba(232,234,238,0.96); font-size: 14px; font-weight: 600; \
         letter-spacing: -0.1px; cursor: pointer;",
        bg0 = theme::BG0,
        line = theme::LINE,
    )
}

fn close_btn_style() -> String {
    format!(
        "height: 24px; width: 24px; padding: 0; \
         border-radius: 4px; background: transparent; \
         border: 1px solid {line}; color: rgba(232,234,238,0.62); \
         font-size: 16px; line-height: 1; cursor: pointer;",
        line = theme::LINE,
    )
}

fn nudge_btn_style() -> String {
    format!(
        "height: 22px; width: 22px; padding: 0; \
         border-radius: 4px; background: {bg0}; border: 1px solid {line}; \
         color: rgba(232,234,238,0.72); font-size: 13px; cursor: pointer; \
         display: inline-flex; align-items: center; justify-content: center;",
        bg0 = theme::BG0,
        line = theme::LINE,
    )
}

fn commit_name_edit(id: PatternId, name_buffer: Signal<String>, renaming: Signal<bool>) {
    let typed = name_buffer.get();
    let trimmed = typed.trim();
    if trimmed.is_empty() {
        let app = use_store::<AppState>();
        let canonical = app
            .project
            .get()
            .patterns
            .get(&id)
            .map(|p: &Pattern| p.name.clone())
            .unwrap_or_default();
        name_buffer.set(canonical);
        renaming.set(false);
        return;
    }
    let new_name = trimmed.to_string();
    let app = use_store::<AppState>();
    if let Err(e) = app.apply_project_edit(move |p| rename_pattern(p, id, new_name.clone())) {
        eprintln!("pattern_editor: rename failed: {e}");
    }
    renaming.set(false);
}
