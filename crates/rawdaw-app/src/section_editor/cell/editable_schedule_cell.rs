//! Per-bar editable cell in the activation cell's variant-schedule
//! row. Mirrors `chord_loop_bar/mod.rs`'s `EditableChordCell`:
//!
//! - A `Select` shows the variant covering this bar and lets the
//!   user pick a different variant, the default, or "(silent)".
//! - A `ContextMenu` on the cell offers merge-left / merge-right /
//!   clear-range — same shape as CL4.x's chord-loop bar.
//!
//! When the parent has no pattern bound, the cell renders disabled
//! (no variants to schedule). Clicking the picker is a no-op in that
//! state — the user has to bind a pattern first via the identity
//! column's PatternSelect.

use rinch::prelude::*;

use rawdaw_model::id::{PatternId, SectionId, TrackId, VariantId};
use rawdaw_model::pattern::PatternBody;


use crate::pattern_actions::{
    clear_activation_variant_range, merge_activation_variant_left,
    merge_activation_variant_right, set_activation_variant_for_bar,
};
use crate::state::{AppState, EditorMode};
use crate::theme;

use super::schedule_column::BarCellKind;

/// Sentinel option values for the variant Select dropdown.
const DEFAULT_OPTION: &str = "__default__";
const SILENT_OPTION: &str = "__silent__";

#[component]
pub(super) fn EditableScheduleCell(
    section_id: SectionId,
    track_id: TrackId,
    bound_pattern_value: u64,
    /// Pattern's default variant label — used for the Default option
    /// label and for filtering "this variant IS the default" out of
    /// the per-bar commits (we just clear the range instead).
    default_variant: String,
    bar: u32,
    variant_label: String,
    kind: BarCellKind,
    pattern_color: String,
) -> NodeHandle {
    let has_pattern = bound_pattern_value != super::pattern_select::NO_PATTERN_SENTINEL;
    let (bg, border) = match kind {
        BarCellKind::Default => (
            with_alpha(pattern_color.as_str(), 0.06),
            with_alpha(pattern_color.as_str(), 0.20),
        ),
        BarCellKind::Named => (
            with_alpha(pattern_color.as_str(), 0.18),
            with_alpha(pattern_color.as_str(), 0.50),
        ),
        BarCellKind::Silent => (
            "transparent".to_string(),
            format!("1px dashed {col}", col = theme::TEXT3),
        ),
    };
    let cell_style = format!(
        "flex: 1; min-width: 0; \
         background: {bg}; \
         border: 1px solid {border}; \
         border-radius: 2px; padding: 2px; \
         display: flex; flex-direction: column; gap: 2px; \
         box-sizing: border-box;",
    );
    let label_style = format!(
        "font-size: 9.5px; color: {text2}; text-align: center; \
         font-style: {style}; line-height: 1;",
        text2 = theme::TEXT2,
        style = if matches!(kind, BarCellKind::Default) { "italic" } else { "normal" },
    );
    let display_label = match kind {
        BarCellKind::Default => format!("↳ {variant_label}"),
        BarCellKind::Named => variant_label.clone(),
        BarCellKind::Silent => "silent".to_string(),
    };

    // Per rinch issue #26 fix (1fa57c5), the rsx for-iter expression
    // now auto-shadows captured function-param identifiers, so the
    // earlier `Rc<String>` wrapping + has_pattern-folded-into-source
    // workaround is no longer needed. Inline if-then-for is the
    // idiomatic shape.
    rsx! {
        ContextMenu {
            ContextMenuTarget {
                div { style: {cell_style.clone()},
                    if has_pattern {
                        for opts in variant_options(
                            bound_pattern_value,
                            default_variant.clone(),
                            kind,
                            variant_label.clone(),
                        ) {
                            Select {
                                key: opts.key.clone(),
                                size: "sm",
                                value: {opts.current_value.clone()},
                                data: opts.options.clone(),
                                onchange: move |v: String| {
                                    commit_variant_for_bar(
                                        section_id, track_id, bar,
                                        bound_pattern_value, v,
                                    );
                                },
                            }
                        }
                    }
                    span { style: {label_style.clone()}, {display_label.clone()} }
                }
            }
            ContextMenuDropdown {
                DropdownMenuItem {
                    onclick: move || commit_merge_left(section_id, track_id, bar),
                    "Merge with left"
                }
                DropdownMenuItem {
                    onclick: move || commit_merge_right(section_id, track_id, bar),
                    "Merge with right"
                }
                DropdownMenuDivider {}
                DropdownMenuItem {
                    onclick: move || commit_clear_range(section_id, track_id, bar),
                    "Clear this range"
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Default)]
struct VariantOptions {
    key: String,
    current_value: String,
    options: Vec<SelectOption>,
}

fn variant_options(
    bound_pattern_value: u64,
    default_variant: String,
    kind: BarCellKind,
    current_variant_label: String,
) -> Vec<VariantOptions> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let pid = PatternId::new(bound_pattern_value);
    let Some(pattern) = project.patterns.get(&pid) else {
        return Vec::new();
    };
    let variants: Vec<VariantId> = match &pattern.body {
        PatternBody::Pitched(b) => b.variants.keys().cloned().collect(),
        PatternBody::Drum(b) => b.variants.keys().cloned().collect(),
    };
    let default_str = default_variant.as_str();
    let mut options = vec![SelectOption::new(
        DEFAULT_OPTION.to_string(),
        format!("↳ {default_str}"),
    )];
    for v in &variants {
        if v.as_str() == default_str {
            continue;
        }
        options.push(SelectOption::new(v.0.clone(), v.0.clone()));
    }
    options.push(SelectOption::new(
        SILENT_OPTION.to_string(),
        "(silent)".to_string(),
    ));
    let current_value = match kind {
        BarCellKind::Default => DEFAULT_OPTION.to_string(),
        BarCellKind::Silent => SILENT_OPTION.to_string(),
        BarCellKind::Named => current_variant_label,
    };
    let variants_key: String = variants
        .iter()
        .map(|v| v.as_str().to_string())
        .collect::<Vec<_>>()
        .join(":");
    let key = format!("vs{bound_pattern_value}-{current_value}-{variants_key}");
    vec![VariantOptions {
        key,
        current_value,
        options,
    }]
}

/// Sentinel VariantId for "silent" segment — matches `cell/mod.rs`'s
/// `SILENT_VARIANT_SENTINEL`. Kept as a free helper here so the
/// commit-time interpretation doesn't depend on parent module
/// internals.
fn silent_variant_id() -> VariantId {
    VariantId::new("__silent__")
}

/// Read the active variant from `EditorMode`. Returns `None` when the
/// section editor isn't mounted (shouldn't fire — these handlers only
/// exist inside the section editor — but the closure paths can't
/// `unreachable!` safely without surfacing in panic logs).
fn active_variant_id(app: &AppState) -> Option<VariantId> {
    match app.editor_mode.get() {
        EditorMode::SectionEditor { variant, .. } => Some(variant),
        EditorMode::Arrangement => None,
    }
}

fn commit_variant_for_bar(
    section_id: SectionId,
    track_id: TrackId,
    bar: u32,
    bound_pattern_value: u64,
    value: String,
) {
    let app = use_store::<AppState>();
    let Some(variant_id) = active_variant_id(&app) else { return };
    if let Err(e) = app.apply_project_edit(move |p| {
        // Re-derive the pattern's default-variant string inside the
        // edit closure so the rsx onchange handler doesn't have to
        // thread it through. `apply_project_edit` already gives us
        // the live project, so the read is free here.
        let default_variant: String = p
            .patterns
            .get(&PatternId::new(bound_pattern_value))
            .map(|pat| pat.default_variant.as_str().to_string())
            .unwrap_or_default();
        let new_variant: Option<VariantId> = match value.as_str() {
            // Selecting the default option clears the pin for this
            // bar — the resolver then falls back to the pattern's
            // default at realization time.
            DEFAULT_OPTION => None,
            SILENT_OPTION => Some(silent_variant_id()),
            other if other == default_variant => None,
            other => Some(VariantId::new(other.to_string())),
        };
        set_activation_variant_for_bar(p, section_id, track_id, &variant_id, bar, new_variant);
    }) {
        eprintln!("section_editor: set activation variant failed: {e}");
    }
}

fn commit_merge_left(section_id: SectionId, track_id: TrackId, bar: u32) {
    let app = use_store::<AppState>();
    let Some(variant_id) = active_variant_id(&app) else { return };
    if let Err(e) = app.apply_project_edit(move |p| {
        merge_activation_variant_left(p, section_id, track_id, &variant_id, bar);
    }) {
        eprintln!("section_editor: merge-left failed: {e}");
    }
}

fn commit_merge_right(section_id: SectionId, track_id: TrackId, bar: u32) {
    let app = use_store::<AppState>();
    let Some(variant_id) = active_variant_id(&app) else { return };
    if let Err(e) = app.apply_project_edit(move |p| {
        merge_activation_variant_right(p, section_id, track_id, &variant_id, bar);
    }) {
        eprintln!("section_editor: merge-right failed: {e}");
    }
}

fn commit_clear_range(section_id: SectionId, track_id: TrackId, bar: u32) {
    let app = use_store::<AppState>();
    let Some(variant_id) = active_variant_id(&app) else { return };
    if let Err(e) = app.apply_project_edit(move |p| {
        clear_activation_variant_range(p, section_id, track_id, &variant_id, bar);
    }) {
        eprintln!("section_editor: clear-range failed: {e}");
    }
}
