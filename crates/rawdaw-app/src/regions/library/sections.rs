//! Sections section of the Library panel.
//!
//! S2 of `docs/section-arrangement-editing-plan.md` carves this
//! section out of `regions/library/mod.rs` proper — the new
//! interactive surface (create / rename / delete / duplicate / pick
//! color, plus selection via [`AppState::select_section`]) matches
//! the CL1 chord-loop + P1 pattern shape exactly.
//!
//! Selection (Library row click) only highlights the row in S2.
//! The S3 meta-bar work wires `selected_section` into the section
//! editor open path so click → editor mount, no arrangement step
//! required.

use std::rc::Rc;

use rinch::core::reactive::{untracked, Effect};
use rinch::prelude::*;

use rawdaw_model::id::SectionId;


use crate::section_actions::{
    create_section, delete_section, duplicate_section, rename_section, set_section_color,
    DeleteRefused,
};
use crate::state::AppState;
use crate::theme;

use super::{group_outer_style, GroupHeader, NewRow};

/// Sections group entry point. Mirrors `ChordLoopsGroup` /
/// `PatternsGroup`: header, interactive rows, and the
/// `+ new section` affordance.
///
/// `build_section_rows()` is called inside the rsx (via the `for`
/// source expression and the reactive count closure) so its read of
/// `app.project` + `app.overlay` lands inside rinch's auto-tracked
/// control-flow closure (rinch Rule 14). A `let rows = …` binding
/// above the rsx would snapshot once and miss create / rename /
/// delete refreshes (the bug surfaced in P1 and fixed in
/// `chord_loops.rs`).
#[component]
pub(super) fn SectionsGroup() -> NodeHandle {
    rsx! {
        div { style: {group_outer_style()},
            GroupHeader {
                title: "Sections",
                count: {|| build_section_rows().len() as u32},
                glyph: "section"
            }
            div { style: "padding-bottom: 4px;",
                for row in build_section_rows() {
                    SectionRow {
                        key: row.id.get(),
                        id: row.id,
                        color: row.color,
                        name: row.name,
                        meta: row.meta,
                    }
                }
                NewRow {
                    label: "new section",
                    onclick: create_section_action,
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct SectionRowData {
    id: SectionId,
    color: String,
    name: String,
    meta: String,
}

fn build_section_rows() -> Vec<SectionRowData> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let overlay = app.overlay.get();
    project
        .sections
        .values()
        .map(|s| {
            // Variant count: base + each named variant override.
            // Mirrors the read-only build_section_rows that lived in
            // mod.rs pre-S2.
            let variant_count = 1 + s.variants.len();
            let meta = if variant_count > 1 {
                format!("{} bars · {} variants", s.base.duration_bars, variant_count)
            } else {
                format!("{} bars", s.base.duration_bars)
            };
            SectionRowData {
                id: s.id,
                // Fall back to `theme::ACCENT` (a hex) NOT `theme::TEXT2`
                // (an rgba string) because `parts::rgba` debug-panics on
                // non-`#RRGGBB` input — see the P4 piano-roll lesson in
                // [[project-next-session-pickup]] / [[project-status]].
                // The chord_loops + patterns Library rows still flow
                // `theme::TEXT2` into rgba; same bug, scope-tight S2 fix
                // here only — chord_loop / pattern hygiene tracked as a
                // follow-up.
                color: overlay
                    .section_color
                    .get(&s.id)
                    .cloned()
                    .unwrap_or_else(|| theme::ACCENT.to_string()),
                name: s.name.clone(),
                meta,
            }
        })
        .collect()
}

/// `+ new section` click handler. Routes through the C2 edit pump
/// so the new section is reachable from every UI surface in
/// lockstep, then selects it so the user sees feedback immediately.
fn create_section_action() {
    let app = use_store::<AppState>();
    let new_id_cell: Rc<std::cell::Cell<Option<SectionId>>> = Rc::new(std::cell::Cell::new(None));
    let cell = new_id_cell.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        cell.set(Some(create_section(p)));
    }) {
        eprintln!("library: create section failed: {e}");
        return;
    }
    if let Some(id) = new_id_cell.get() {
        app.select_section(Some(id));
    }
}

/// Interactive section row with selection, inline rename, and the
/// `⋯` action menu. Component-scoped so each row carries its own
/// `editing` / `name_input` / `menu_open` Signals; `key: id.get()`
/// at the call site preserves them across reorder (rinch Rule 9).
#[component]
fn SectionRow(id: SectionId, color: String, name: String, meta: String) -> NodeHandle {
    let editing = Signal::new(false);
    let name_input = Signal::new(name.clone());
    let menu_open = Signal::new(false);

    // Sync the input from the live project when the canonical name
    // changes externally (rename via another surface, load, etc.).
    // The `untracked` peek at `name_input` mirrors the C4
    // `NameControl` pattern so the user's mid-edit typing doesn't
    // bounce off the Effect.
    let _ = Effect::new(move || {
        let project = use_store::<AppState>().project.get();
        let canonical = project
            .sections
            .get(&id)
            .map(|s| s.name.clone())
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
                    use_store::<AppState>().selected_section.get() == Some(id);
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
                use_store::<AppState>().select_section(Some(id));
            },
            span { style: {swatch_style.clone()} }
            div { style: {row_stack_style()},
                if editing.get() {
                    input {
                        r#type: "text",
                        title: "Rename section (Enter to commit, blank to revert)",
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
            DropdownMenu {
                opened_fn: move || menu_open.get(),
                on_close: move || menu_open.set(false),
                DropdownMenuTarget {
                    button {
                        r#type: "button",
                        title: "Section actions",
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

// Per-row styling helpers. Returned fresh from a free fn so the
// rsx reactive closures don't need to capture-and-clone a `let`-
// bound String across each re-run. Same shape as `chord_loops.rs`.
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
/// contract used in `chord_loops.rs` + `patterns.rs`: empty input
/// reverts to canonical, otherwise commits via the edit pump.
fn commit_rename(id: SectionId, name_input: Signal<String>, editing: Signal<bool>) {
    let typed = name_input.get();
    let trimmed = typed.trim();
    if trimmed.is_empty() {
        let canonical = use_store::<AppState>()
            .project
            .get()
            .sections
            .get(&id)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        name_input.set(canonical);
        editing.set(false);
        return;
    }
    let new_name = trimmed.to_string();
    let app = use_store::<AppState>();
    if let Err(e) = app.apply_project_edit(move |p| rename_section(p, id, new_name.clone())) {
        eprintln!("library: rename section failed: {e}");
    }
    editing.set(false);
}

/// `Duplicate` action handler. Routes through the edit pump and
/// selects the new section on success.
fn duplicate_action(id: SectionId) {
    let app = use_store::<AppState>();
    let new_id_cell: Rc<std::cell::Cell<Option<SectionId>>> = Rc::new(std::cell::Cell::new(None));
    let cell = new_id_cell.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        cell.set(duplicate_section(p, id));
    }) {
        eprintln!("library: duplicate section failed: {e}");
        return;
    }
    if let Some(new_id) = new_id_cell.get() {
        app.select_section(Some(new_id));
    }
}

/// `Delete` action handler. Refuses-with-message when the section
/// is referenced by any arrangement step. The S2 surface uses
/// `eprintln!` until the toast / alert primitive ships; the model
/// dry-run pre-empts the edit pump so a refusal doesn't trigger a
/// realize cycle.
fn delete_action(id: SectionId) {
    let app = use_store::<AppState>();
    let project = app.project.get();
    if let Err(refusal) = delete_section(&mut (*project).clone(), id) {
        match refusal {
            DeleteRefused::ReferencedBy(steps) => {
                let positions: Vec<String> =
                    steps.iter().map(|i| format!("step {i}")).collect();
                eprintln!(
                    "library: refusing delete — section referenced by {} arrangement {}: {}",
                    steps.len(),
                    if steps.len() == 1 { "step" } else { "steps" },
                    positions.join(", "),
                );
            }
            DeleteRefused::NotFound => {
                eprintln!("library: delete failed — section id not found");
            }
        }
        return;
    }
    if let Err(e) = app.apply_project_edit(move |p| {
        // Dry-run above told us the delete will succeed; surface any
        // unexpected error verbatim.
        let _ = delete_section(p, id);
    }) {
        eprintln!("library: delete section failed: {e}");
        return;
    }
    // Clear selection if the deleted section was the selected one.
    if app.selected_section.get() == Some(id) {
        app.select_section(None);
    }
}

/// `Pick color` action handler. Writes the overlay signal directly
/// (overlays don't drive audio, so the edit pump isn't required —
/// same pattern as chord-loop / pattern color picks).
fn set_color_action(id: SectionId, hex: String) {
    let app = use_store::<AppState>();
    let mut overlay = (*app.overlay.get()).clone();
    set_section_color(&mut overlay, id, hex);
    app.overlay.set(Rc::new(overlay));
}
