//! Patterns section of the Library panel.
//!
//! P1 of `docs/pattern-editor-plan.md` carves this section out of
//! `regions/library/mod.rs` — the new interactive surface (create
//! pitched / create drum / rename / delete / duplicate / pick color,
//! plus selection via [`AppState::select_pattern`]) is large enough
//! to warrant its own module, mirroring CL1's
//! [`super::chord_loops`] split.
//!
//! The pattern *editor* (P2+) reads [`AppState::selected_pattern`] to
//! mount a piano-roll surface over the standard arrangement row —
//! same "center stage" pattern the chord-loop editor uses. P1 only
//! plumbs the row selection + library CRUD; the editor stub doesn't
//! render anything yet.

use std::rc::Rc;

use rinch::core::reactive::{untracked, Effect};
use rinch::prelude::*;

use rawdaw_model::id::PatternId;
use rawdaw_model::pattern::{Pattern, PatternBody};


use crate::pattern_actions::{
    create_drum_pattern, create_pitched_pattern, delete_pattern, duplicate_pattern,
    rename_pattern, set_pattern_color, DeleteRefused,
};
use crate::state::AppState;
use crate::theme;

use super::{group_outer_style, GroupHeader, NewRow};

/// Patterns group entry point. Composes the header, the interactive
/// rows, and the two `+` affordances (pitched + drum kinds are
/// distinct at creation time since the body type can't change
/// afterwards — P0 design decision 2 of the pattern-editor plan).
///
/// `build_pattern_rows()` is called *inside* the rsx (via the `for`
/// source expression and a reactive count closure) so its read of
/// `app.project` lands inside rinch's auto-tracked control-flow
/// closure (rinch Rule 14). A `let rows = build_pattern_rows()`
/// binding above the rsx would capture a one-shot snapshot and the
/// library would never refresh on create / rename / delete — the
/// component itself doesn't re-run (rinch Rule 1).
#[component]
pub(super) fn PatternsGroup() -> NodeHandle {
    rsx! {
        div { style: {group_outer_style()},
            GroupHeader {
                title: "Patterns",
                count: {|| build_pattern_rows().len() as u32},
                glyph: "pattern"
            }
            div { style: "padding-bottom: 4px;",
                for row in build_pattern_rows() {
                    PatternRow {
                        key: row.id.get(),
                        id: row.id,
                        color: row.color,
                        name: row.name,
                        meta: row.meta,
                    }
                }
                NewRow {
                    label: "new pitched pattern",
                    onclick: create_pitched_action,
                }
                NewRow {
                    label: "new drum pattern",
                    onclick: create_drum_action,
                }
            }
        }
    }
}

/// Pre-resolved pattern row data: keyed by the typed model id so
/// reordering / renames don't drop the per-row Signal state held by
/// the [`PatternRow`] component instance (rinch Rule 9).
#[derive(Clone, PartialEq)]
struct PatternRowData {
    id: PatternId,
    color: String,
    name: String,
    meta: String,
}

fn build_pattern_rows() -> Vec<PatternRowData> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let overlay = app.overlay.get();
    let beats_per_bar = project
        .tempo_map
        .beats_per_bar_at(rawdaw_model::time::MusicalTime::ZERO);
    project
        .patterns
        .values()
        .map(|p| PatternRowData {
            id: p.id,
            color: overlay
                .pattern_color
                .get(&p.id)
                .cloned()
                .unwrap_or_else(|| theme::TEXT2.to_string()),
            name: p.name.clone(),
            meta: build_meta(p, beats_per_bar, overlay.pattern_meta.get(&p.id)),
        })
        .collect()
}

/// Pattern row meta line: `"<kind> · <N> bars"`, optionally followed
/// by `" · <K> variants"` when more than one variant exists, and the
/// user-set overlay meta (if any) appended in parentheses. Mirrors
/// the chord-loop row's `"<N> bars · <progression>"` shape but leads
/// with the kind glyph-word because users disambiguate patterns by
/// kind first.
fn build_meta(p: &Pattern, beats_per_bar: u32, overlay_meta: Option<&String>) -> String {
    let kind = match &p.body {
        PatternBody::Pitched(_) => "pitched",
        PatternBody::Drum(_) => "drum",
    };
    let bars = duration_in_bars(p.length(), beats_per_bar);
    let variant_count = match &p.body {
        PatternBody::Pitched(b) => b.variants.len(),
        PatternBody::Drum(b) => b.variants.len(),
    };
    let mut meta = format!("{kind} · {bars} bars");
    if variant_count > 1 {
        meta.push_str(&format!(" · {variant_count} variants"));
    }
    if let Some(extra) = overlay_meta.filter(|s| !s.is_empty()) {
        meta.push_str(&format!(" ({extra})"));
    }
    meta
}

/// Round-1 patterns are stored as `Duration::bars(n, beats_per_bar)`,
/// which encodes the length in PPQ ticks. Reconstruct the bar count
/// by dividing the tick count by `PPQ * beats_per_bar`. Mirrors
/// [`super::chord_loops::duration_in_bars`].
fn duration_in_bars(d: rawdaw_model::time::Duration, beats_per_bar: u32) -> u32 {
    let ticks_per_bar = rawdaw_model::time::PPQ * beats_per_bar.max(1) as i64;
    (d.as_ticks() / ticks_per_bar).max(0) as u32
}

/// `+ new pitched pattern` click handler. Routes through the C2 edit
/// pump so the new pattern is reachable from every UI surface in
/// lockstep, then selects it so the user sees feedback immediately.
fn create_pitched_action() {
    create_action(create_pitched_pattern);
}

/// `+ new drum pattern` click handler. Same shape as
/// [`create_pitched_action`]; the underlying mutation differs in
/// body kind only.
fn create_drum_action() {
    create_action(create_drum_pattern);
}

/// Shared create-and-select plumbing. The factory mutates the
/// `&mut Project` and returns the new id; on success we update the
/// selection signal so the new row highlights immediately.
fn create_action(factory: fn(&mut rawdaw_model::project::Project) -> PatternId) {
    let app = use_store::<AppState>();
    let new_id_cell: Rc<std::cell::Cell<Option<PatternId>>> = Rc::new(std::cell::Cell::new(None));
    let cell = new_id_cell.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        cell.set(Some(factory(p)));
    }) {
        eprintln!("library: create pattern failed: {e}");
        return;
    }
    if let Some(id) = new_id_cell.get() {
        app.select_pattern(Some(id));
    }
}

/// Interactive pattern row with selection, inline rename, and a
/// `⋯` action menu (Duplicate / Delete / pick color). Built as a
/// `#[component]` rather than inlined so each row carries its own
/// `editing` + `name_input` Signal scope; `key: id.get()` on the
/// call site preserves these Signals across reorder / refresh
/// (rinch Rule 9).
#[component]
fn PatternRow(id: PatternId, color: String, name: String, meta: String) -> NodeHandle {
    let editing = Signal::new(false);
    let name_input = Signal::new(name.clone());
    let menu_open = Signal::new(false);

    // Sync the input from the live project when the canonical name
    // changes externally (rename via another surface, load, etc.).
    // Reads `name_input` via `untracked` so the user's mid-edit
    // typing doesn't bounce off the Effect (the C4 `NameControl`
    // pattern). The Effect subscribes to `project` only.
    let _ = Effect::new(move || {
        let project = use_store::<AppState>().project.get();
        let canonical = project
            .patterns
            .get(&id)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let display = untracked(|| name_input.get());
        if display == canonical {
            return;
        }
        name_input.set(canonical);
    });

    let swatch_style = format!(
        "width: 10px; height: 10px; border-radius: 2px; flex: 0 0 auto; \
         background: {color}; border: 1px solid {border};",
        color = color,
        border = with_alpha(color.as_str(), 0.6),
    );
    let color_for_row = color.clone();
    let color_for_menu = color.clone();

    rsx! {
        div {
            style: {|| {
                let selected =
                    use_store::<AppState>().selected_pattern.get() == Some(id);
                let bg = if selected {
                    with_alpha(color_for_row.as_str(), 0.10)
                } else {
                    "transparent".to_string()
                };
                let border_left = if selected {
                    format!("2px solid {}", color_for_row)
                } else {
                    "2px solid transparent".to_string()
                };
                format!(
                    "display: flex; align-items: center; gap: 8px; \
                     padding: 4px 8px; \
                     background: {bg}; border-left: {border_left}; \
                     cursor: pointer; min-height: 28px;",
                )
            }},
            onclick: move || {
                use_store::<AppState>().select_pattern(Some(id));
            },
            span { style: {swatch_style.clone()} }
            div { style: {row_stack_style()},
                if editing.get() {
                    input {
                        r#type: "text",
                        title: "Rename pattern (Enter to commit, blank to revert)",
                        style: {rename_input_style()},
                        value: {|| name_input.get()},
                        oninput: move |v: String| name_input.set(v),
                        onsubmit: move || commit_rename(id, name_input, editing),
                    }
                } else {
                    div { style: {row_name_style()}, {|| name_input.get()} }
                    div { style: {row_meta_style()}, {meta.clone()} }
                }
            }
            DropdownMenu { opened_fn: move || menu_open.get(),
                DropdownMenuTarget {
                    button {
                        r#type: "button",
                        title: "Pattern actions",
                        style: {dots_btn_style()},
                        onclick: move || menu_open.update(|v| *v = !*v),
                        "⋯"
                    }
                }
                DropdownMenuDropdown {
                    DropdownMenuItem {
                        onclick: move || {
                            editing.set(true);
                            menu_open.set(false);
                        },
                        "Rename"
                    }
                    DropdownMenuItem {
                        onclick: move || {
                            duplicate_action(id);
                            menu_open.set(false);
                        },
                        "Duplicate"
                    }
                    DropdownMenuItem {
                        onclick: move || {
                            delete_action(id);
                            menu_open.set(false);
                        },
                        "Delete"
                    }
                    DropdownMenuDivider {}
                    for swatch in palette_swatches(&color_for_menu) {
                        DropdownMenuItem {
                            key: swatch.label.clone(),
                            onclick: move || {
                                let hex = swatch.value.clone();
                                set_color_action(id, hex);
                                menu_open.set(false);
                            },
                            {swatch.label.clone()}
                        }
                    }
                }
            }
        }
    }
}

// Per-row styling helpers. Returned fresh from a free fn so the rsx
// reactive closures don't need to capture-and-clone a `let`-bound
// String across each re-run. Same shape as
// [`super::chord_loops`]'s helpers — keep aligned so the planned
// design pass restyles both groups consistently.

fn row_stack_style() -> String {
    "display: flex; flex-direction: column; min-width: 0; gap: 0; flex: 1;".into()
}

fn row_name_style() -> String {
    "font-size: 12.5px; color: rgba(232,234,238,0.96); \
     font-weight: 500; overflow: hidden; text-overflow: ellipsis; \
     white-space: nowrap; line-height: 1.25;"
        .into()
}

fn row_meta_style() -> String {
    "font-size: 10.5px; color: rgba(232,234,238,0.42); \
     line-height: 1.25; font-feature-settings: \"tnum\" 1;"
        .into()
}

fn rename_input_style() -> String {
    format!(
        "width: 100%; box-sizing: border-box; \
         height: 22px; padding: 0 6px; \
         border-radius: 3px; background: {bg0}; border: 1px solid {line}; \
         color: rgba(232,234,238,0.96); font-size: 12.5px; font-weight: 500;",
        bg0 = theme::BG0,
        line = theme::LINE,
    )
}

fn dots_btn_style() -> String {
    "width: 22px; height: 22px; padding: 0; flex: 0 0 auto; \
     border-radius: 3px; background: transparent; border: 1px solid transparent; \
     color: rgba(232,234,238,0.42); cursor: pointer; \
     display: inline-flex; align-items: center; justify-content: center; \
     font-size: 13px; line-height: 1;"
        .into()
}

#[derive(Clone, PartialEq)]
struct ColorSwatch {
    label: String,
    value: String,
}

/// Build the per-row palette pick-list. Marks the currently-applied
/// color with a `✓` prefix so the menu reads as a control rather than
/// a static list. v1 palette: the 10 `theme::PAL_*` entries plus a
/// "Default" option that clears the overlay entry. Same shape as
/// [`super::chord_loops::palette_swatches`].
fn palette_swatches(current: &str) -> Vec<ColorSwatch> {
    let entries: &[(&str, &str)] = &[
        ("Blue", theme::PAL_BLUE),
        ("Teal", theme::PAL_TEAL),
        ("Sage", theme::PAL_SAGE),
        ("Olive", theme::PAL_OLIVE),
        ("Sand", theme::PAL_SAND),
        ("Terra", theme::PAL_TERRA),
        ("Clay", theme::PAL_CLAY),
        ("Rose", theme::PAL_ROSE),
        ("Plum", theme::PAL_PLUM),
        ("Slate", theme::PAL_SLATE),
    ];
    let mut out: Vec<ColorSwatch> = entries
        .iter()
        .map(|(name, hex)| ColorSwatch {
            label: if (*hex).eq_ignore_ascii_case(current) {
                format!("✓ {name}")
            } else {
                format!("  {name}")
            },
            value: (*hex).to_string(),
        })
        .collect();
    out.push(ColorSwatch {
        label: "  Default".to_string(),
        value: String::new(),
    });
    out
}

/// Inline-rename commit handler. Mirrors the C4 `NameControl`
/// contract: empty input reverts to canonical, otherwise commits via
/// the edit pump.
fn commit_rename(id: PatternId, name_input: Signal<String>, editing: Signal<bool>) {
    let typed = name_input.get();
    let trimmed = typed.trim();
    if trimmed.is_empty() {
        let canonical = use_store::<AppState>()
            .project
            .get()
            .patterns
            .get(&id)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        name_input.set(canonical);
        editing.set(false);
        return;
    }
    let new_name = trimmed.to_string();
    let app = use_store::<AppState>();
    if let Err(e) = app.apply_project_edit(move |p| rename_pattern(p, id, new_name.clone())) {
        eprintln!("library: rename pattern failed: {e}");
    }
    editing.set(false);
}

/// `Duplicate` action handler. Routes through the edit pump and
/// selects the new pattern on success.
fn duplicate_action(id: PatternId) {
    let app = use_store::<AppState>();
    let new_id_cell: Rc<std::cell::Cell<Option<PatternId>>> = Rc::new(std::cell::Cell::new(None));
    let cell = new_id_cell.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        cell.set(duplicate_pattern(p, id));
    }) {
        eprintln!("library: duplicate pattern failed: {e}");
        return;
    }
    if let Some(new_id) = new_id_cell.get() {
        app.select_pattern(Some(new_id));
    }
}

/// `Delete` action handler. Refuses-with-message when the pattern is
/// referenced by any activation; the P1 surface uses stderr until the
/// toast / alert primitive ships. Always-safe — never silently
/// corrupts the project (mirrors CL1's chord-loop delete contract).
fn delete_action(id: PatternId) {
    let app = use_store::<AppState>();
    // Walk for refusal first so the error message matches what the
    // edit pump would surface; doing this outside the closure also
    // avoids a needless realize cycle when the delete is refused.
    let project = app.project.get();
    if let Err(refusal) = delete_pattern(&mut (*project).clone(), id) {
        match refusal {
            DeleteRefused::ReferencedBy(names) => {
                eprintln!(
                    "library: refusing delete — pattern referenced by sections: {}",
                    names.join(", "),
                );
            }
            DeleteRefused::NotFound => {
                eprintln!("library: delete failed — pattern id not found");
            }
        }
        return;
    }
    if let Err(e) = app.apply_project_edit(move |p| {
        let _ = delete_pattern(p, id);
    }) {
        eprintln!("library: delete pattern failed: {e}");
        return;
    }
    if app.selected_pattern.get() == Some(id) {
        app.select_pattern(None);
    }
}

/// `Pick color` action handler. Writes the overlay signal directly
/// (overlays don't drive audio, so the edit pump isn't required —
/// same pattern as the C3 load path's overlay swap).
fn set_color_action(id: PatternId, hex: String) {
    let app = use_store::<AppState>();
    let mut overlay = (*app.overlay.get()).clone();
    set_pattern_color(&mut overlay, id, hex);
    app.overlay.set(Rc::new(overlay));
}
