//! Chord Loops section of the Library panel.
//!
//! CL1 of `docs/chord-loop-editing-plan.md` carves this section out
//! of `regions/library.rs` proper — the new interactive surface
//! (create / rename / delete / duplicate / pick color, plus
//! selection via [`AppState::select_chord_loop`]) pushed the parent
//! file over the workspace 700-line cap, and the chord-loop UI is
//! cohesive enough to live in its own module.
//!
//! Patterns + Sections sections of the Library remain read-only in
//! v1 and live in `mod.rs`. When their CL1-equivalents land they
//! follow this module's structure: own sub-file under
//! `regions/library/`.

use std::rc::Rc;

use rinch::core::reactive::{untracked, Effect};
use rinch::prelude::*;

use rawdaw_model::id::ChordLoopId;

use crate::chord_display::roman_label;
use crate::chord_loop_actions::{
    create_chord_loop, delete_chord_loop, duplicate_chord_loop, rename_chord_loop,
    set_chord_loop_color, DeleteRefused,
};

use crate::state::AppState;
use crate::theme;

use super::{group_outer_style, GroupHeader, NewRow};

/// Chord Loops group entry point. Composes the header, the
/// interactive rows, and the `+ new chord loop` affordance.
///
/// `build_chord_loop_rows()` is called *inside* the rsx (via the
/// `for` source expression and a reactive count closure) so its read
/// of `app.project` lands inside rinch's auto-tracked control-flow
/// closure (rinch Rule 14). A `let rows = build_chord_loop_rows()`
/// binding above the rsx — the original CL1 shape — captures a
/// one-shot snapshot and the library never refreshes on create /
/// rename / delete (the component itself doesn't re-run; rinch
/// Rule 1). Patterns P1 surfaced the bug; the fix is applied here in
/// parallel so the chord-loop library refreshes consistently.
#[component]
pub(super) fn ChordLoopsGroup() -> NodeHandle {
    rsx! {
        div { style: {group_outer_style()},
            GroupHeader {
                title: "Chord Loops",
                count: {|| build_chord_loop_rows().len() as u32},
                glyph: "chord"
            }
            div { style: "padding-bottom: 4px;",
                for row in build_chord_loop_rows() {
                    ChordLoopRow {
                        key: row.id.get(),
                        id: row.id,
                        color: row.color,
                        name: row.name,
                        meta: row.meta,
                    }
                }
                NewRow {
                    label: "new chord loop",
                    onclick: create_chord_loop_action,
                }
            }
        }
    }
}

/// Pre-resolved chord-loop row data: keyed by the typed model id so
/// reordering / renames don't drop the per-row Signal state held by
/// the [`ChordLoopRow`] component instance (rinch Rule 9).
#[derive(Clone, PartialEq)]
struct ChordLoopRowData {
    id: ChordLoopId,
    color: String,
    name: String,
    meta: String,
}

fn build_chord_loop_rows() -> Vec<ChordLoopRowData> {
    use rawdaw_model::chord::ChordSpec;

    let app = use_store::<AppState>();
    let project = app.project.get();
    let overlay = app.overlay.get();
    let beats_per_bar = project
        .tempo_map
        .beats_per_bar_at(rawdaw_model::time::MusicalTime::ZERO);
    project
        .chord_loops
        .values()
        .map(|cl| {
            let romans: Vec<String> = cl
                .events
                .iter()
                .map(|e| match &e.chord {
                    ChordSpec::Functional { roman, suffix, .. } => {
                        roman_label(*roman, &suffix.quality)
                    }
                    ChordSpec::Absolute { .. } => String::new(),
                })
                .collect();
            ChordLoopRowData {
                id: cl.id,
                color: overlay
                    .chord_loop_color
                    .get(&cl.id)
                    .cloned()
                    .unwrap_or_else(|| theme::TEXT2.to_string()),
                name: cl.name.clone(),
                meta: format!(
                    "{} bars · {}",
                    duration_in_bars(cl.length, beats_per_bar),
                    romans.join(" "),
                ),
            }
        })
        .collect()
}

/// Round-1 chord loops are stored as `Duration::bars(n, beats_per_bar)`,
/// which encodes the length in PPQ ticks. Reconstruct the bar count by
/// dividing the tick count by `PPQ * beats_per_bar`.
fn duration_in_bars(d: rawdaw_model::time::Duration, beats_per_bar: u32) -> u32 {
    let ticks_per_bar = rawdaw_model::time::PPQ * beats_per_bar.max(1) as i64;
    (d.as_ticks() / ticks_per_bar).max(0) as u32
}

/// `+ new chord loop` click handler. Routes through the C2 edit pump
/// so the new loop is reachable from every UI surface in lockstep,
/// then selects it so the user sees feedback immediately.
fn create_chord_loop_action() {
    let app = use_store::<AppState>();
    let new_id_cell: Rc<std::cell::Cell<Option<ChordLoopId>>> = Rc::new(std::cell::Cell::new(None));
    let cell = new_id_cell.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        cell.set(Some(create_chord_loop(p)));
    }) {
        eprintln!("library: create chord loop failed: {e}");
        return;
    }
    if let Some(id) = new_id_cell.get() {
        app.select_chord_loop(Some(id));
    }
}

/// Interactive chord-loop row with selection, inline rename, and a
/// `⋯` action menu (Duplicate / Delete / pick color). Built as a
/// `#[component]` rather than inlined so each row carries its own
/// `editing` + `name_input` Signal scope. The `key: id.get()` prop
/// on the call site preserves these Signals across reorder /
/// refresh (rinch Rule 9).
#[component]
fn ChordLoopRow(id: ChordLoopId, color: String, name: String, meta: String) -> NodeHandle {
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
            .chord_loops
            .get(&id)
            .map(|cl| cl.name.clone())
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
    // Color needs to outlive the rsx! reactive closures below; each
    // `{|| ... color ...}` captures by move, so clone one copy per
    // closure call site.
    let color_for_row = color.clone();
    let color_for_menu = color.clone();

    rsx! {
        div {
            style: {|| {
                let selected =
                    use_store::<AppState>().selected_chord_loop.get() == Some(id);
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
                // Clicking anywhere on the row body (not the menu /
                // input) selects the loop. The menu button stops
                // propagation by virtue of being a separate sibling
                // — rinch's onclick on the inner button doesn't
                // bubble to the wrapper.
                use_store::<AppState>().select_chord_loop(Some(id));
            },
            span { style: {swatch_style.clone()} }
            div { style: {row_stack_style()},
                if editing.get() {
                    input {
                        r#type: "text",
                        title: "Rename chord loop (Enter to commit, blank to revert)",
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
                        title: "Chord-loop actions",
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
                    // Color sub-options as flat items. A nested
                    // submenu would be cleaner but rinch's
                    // DropdownMenu doesn't compose recursively
                    // today; one row per palette entry keeps the
                    // UX honest and matches the
                    // "placeholder visuals" memo
                    // [[project-ui-redesign-pending]].
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
// bound String across each re-run.
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
/// color with a `✓` prefix so the menu reads as a control rather
/// than a static list. v1 palette: the 10 `theme::PAL_*` entries
/// plus a "Default" option that clears the overlay entry.
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
/// contract: empty input reverts to canonical, otherwise commits
/// via the edit pump.
fn commit_rename(id: ChordLoopId, name_input: Signal<String>, editing: Signal<bool>) {
    let typed = name_input.get();
    let trimmed = typed.trim();
    if trimmed.is_empty() {
        let canonical = use_store::<AppState>()
            .project
            .get()
            .chord_loops
            .get(&id)
            .map(|cl| cl.name.clone())
            .unwrap_or_default();
        name_input.set(canonical);
        editing.set(false);
        return;
    }
    let new_name = trimmed.to_string();
    let app = use_store::<AppState>();
    if let Err(e) = app.apply_project_edit(move |p| rename_chord_loop(p, id, new_name.clone())) {
        eprintln!("library: rename chord loop failed: {e}");
    }
    editing.set(false);
}

/// `Duplicate` action handler. Routes through the edit pump and
/// selects the new loop on success.
fn duplicate_action(id: ChordLoopId) {
    let app = use_store::<AppState>();
    let new_id_cell: Rc<std::cell::Cell<Option<ChordLoopId>>> = Rc::new(std::cell::Cell::new(None));
    let cell = new_id_cell.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        cell.set(duplicate_chord_loop(p, id));
    }) {
        eprintln!("library: duplicate chord loop failed: {e}");
        return;
    }
    if let Some(new_id) = new_id_cell.get() {
        app.select_chord_loop(Some(new_id));
    }
}

/// `Delete` action handler. Refuses-with-message when the loop is
/// referenced; the CL1 surface uses stderr until the toast / alert
/// primitive ships. Always-safe — never silently corrupts the
/// project (per CL plan CL1 contract).
fn delete_action(id: ChordLoopId) {
    let app = use_store::<AppState>();
    // Walk for refusal first so the error message matches what the
    // edit pump would surface; doing this outside the closure also
    // avoids a needless realize cycle when the delete is refused.
    let project = app.project.get();
    if let Err(refusal) = delete_chord_loop(&mut (*project).clone(), id) {
        match refusal {
            DeleteRefused::ReferencedBy(names) => {
                eprintln!(
                    "library: refusing delete — chord loop referenced by sections: {}",
                    names.join(", "),
                );
            }
            DeleteRefused::NotFound => {
                eprintln!("library: delete failed — chord loop id not found");
            }
        }
        return;
    }
    if let Err(e) = app.apply_project_edit(move |p| {
        // The dry-run above told us delete will succeed; surface
        // any unexpected error verbatim.
        let _ = delete_chord_loop(p, id);
    }) {
        eprintln!("library: delete chord loop failed: {e}");
        return;
    }
    // Clear selection if the deleted loop was the selected one.
    if app.selected_chord_loop.get() == Some(id) {
        app.select_chord_loop(None);
    }
}

/// `Pick color` action handler. Writes the overlay signal directly
/// (overlays don't drive audio, so the edit pump isn't required —
/// same pattern as the C3 load path's overlay swap).
fn set_color_action(id: ChordLoopId, hex: String) {
    let app = use_store::<AppState>();
    let mut overlay = (*app.overlay.get()).clone();
    set_chord_loop_color(&mut overlay, id, hex);
    app.overlay.set(Rc::new(overlay));
}
