//! Variant tab strip: `base default variant · stripped · + New variant`.
//!
//! S3 (2026-05-22) makes the strip CRUD-capable:
//! - Clicking a tab still switches via `set_variant`.
//! - Right-click on any tab opens a `ContextMenu` with Rename /
//!   Delete / Set as default. Default-variant is protected against
//!   delete (refuses with `RemoveVariantError::DefaultVariant`).
//! - The trailing `+ New variant` button creates an empty
//!   `SectionVariantOverride` via `add_section_variant`, then
//!   activates the new tab via `set_variant`.
//!
//! Decision 19 / 21 in the round-2 README still applies: the
//! "default variant" label is italic light-grey text next to the tab
//! name, NOT an uppercase chip.

use rinch::core::reactive::{untracked, Effect};
use rinch::prelude::*;

use rawdaw_model::id::{SectionId, VariantId};

use crate::parts::Icon;
use crate::regions::inspector::variant_options_for_section;
use crate::section_actions::{
    add_section_variant, remove_section_variant, rename_section_variant, set_default_variant,
    RemoveVariantError,
};
use crate::state::{AppState, EditorMode};
use crate::theme;

#[component]
pub fn VariantTabs(
    section_id: u64,
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

    let section_name = section_name_for_id(SectionId::new(section_id));
    let default = default_variant.clone();
    let color = section_color.clone();

    rsx! {
        div { style: {strip_style.clone()},
            for v in variant_options_for_section(section_name.clone()) {
                VariantTab {
                    key: v.id.clone(),
                    section_id: section_id,
                    variant_id: v.id.clone(),
                    variant_name: v.name,
                    is_default: v.id == default.as_str(),
                    section_color: color.clone(),
                }
            }
            NewVariantBtn { section_id: section_id }
        }
    }
}

fn section_name_for_id(id: SectionId) -> String {
    let app = use_store::<AppState>();
    app.project
        .get()
        .sections
        .get(&id)
        .map(|s| s.name.clone())
        .unwrap_or_default()
}

#[component]
fn VariantTab(
    section_id: u64,
    variant_id: String,
    variant_name: String,
    is_default: bool,
    section_color: String,
) -> NodeHandle {
    let app = use_store::<AppState>();
    let sid = SectionId::new(section_id);
    let target = variant_id.clone();

    // Inline-rename state — owned per-tab so each tab keeps its own
    // typing buffer. Synced from the project via Effect + `untracked`
    // peek (the C4 `NameControl` pattern).
    let editing = Signal::new(false);
    let name_input = Signal::new(variant_name.clone());
    let target_for_effect = target.clone();
    let _ = Effect::new(move || {
        let project = use_store::<AppState>().project.get();
        let canonical = project
            .sections
            .get(&sid)
            .and_then(|s| {
                let v = VariantId::new(target_for_effect.clone());
                if v == s.default_variant {
                    Some(v.as_str().to_string())
                } else {
                    s.variants
                        .keys()
                        .find(|k| k.as_str() == target_for_effect)
                        .map(|k| k.as_str().to_string())
                }
            })
            .unwrap_or_default();
        let display = untracked(|| name_input.get());
        if display == canonical || canonical.is_empty() {
            return;
        }
        name_input.set(canonical);
    });

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

    let target_for_style = target.clone();
    let target_for_click = target.clone();
    let target_for_rename_menu = target.clone();
    let target_for_delete = target.clone();
    let target_for_set_default = target.clone();
    let _ = section_color;

    rsx! {
        ContextMenu {
            ContextMenuTarget {
                button {
                    r#type: "button",
                    style: {
                        if matches!(
                            app.editor_mode.get(),
                            EditorMode::SectionEditor { ref variant, .. }
                                if variant.as_str() == target_for_style.as_str()
                        ) {
                            active_style.clone()
                        } else {
                            inactive_style.clone()
                        }
                    },
                    onclick: move || app.set_variant(VariantId::new(target_for_click.clone())),
                    if editing.get() {
                        input {
                            r#type: "text",
                            title: "Rename variant (Enter to commit, blank to cancel)",
                            style: {tab_rename_input_style()},
                            value: {|| name_input.get()},
                            oninput: move |v: String| name_input.set(v),
                            // Read the active variant from editor_mode
                            // at submit time so this closure captures
                            // only Copy types (sid + Signals). The
                            // Rename menu item activates the tab
                            // before flipping editing=true so the
                            // active variant equals what the user
                            // intended to rename.
                            onsubmit: move || commit_rename_active(sid, name_input, editing),
                        }
                    } else {
                        {|| name_input.get()}
                    }
                    if is_default {
                        span { style: "font-size: 9.5px; color: rgba(232,234,238,0.28); \
                                       letter-spacing: 0.4px; font-style: italic;",
                            "default variant"
                        }
                    }
                }
            }
            ContextMenuDropdown {
                DropdownMenuItem {
                    onclick: move || {
                        // Activate this tab, then flip into edit mode.
                        // `commit_rename_active` reads the active
                        // variant from editor_mode at submit time, so
                        // we must guarantee it matches the tab the
                        // user picked.
                        app.set_variant(VariantId::new(target_for_rename_menu.clone()));
                        editing.set(true);
                    },
                    "Rename"
                }
                DropdownMenuItem {
                    onclick: move || {
                        delete_variant(sid, VariantId::new(target_for_delete.clone()));
                    },
                    "Delete"
                }
                DropdownMenuItem {
                    onclick: move || {
                        promote_default(sid, VariantId::new(target_for_set_default.clone()));
                    },
                    "Set as default"
                }
            }
        }
    }
}

#[component]
fn NewVariantBtn(section_id: u64) -> NodeHandle {
    let sid = SectionId::new(section_id);
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
            onclick: move || create_variant_action(sid),
            Icon { glyph: "plus", size: sz, stroke: {stroke.clone()}, stroke_width: sw }
            span { "New variant" }
        }
    }
}

fn tab_rename_input_style() -> String {
    format!(
        "height: 20px; padding: 0 4px; \
         font-size: 12.5px; font-weight: 600; color: {text0}; \
         background: {bg0}; border: 1px solid {line}; border-radius: 3px; \
         font-family: inherit;",
        text0 = theme::TEXT0,
        bg0 = theme::BG0,
        line = theme::LINE,
    )
}

/// `+ New variant` handler. Creates an empty
/// `SectionVariantOverride` with a fresh `untitled`-suffixed id,
/// then activates the new tab via `set_variant` so the user sees the
/// blank override immediately. Inline-rename is left as a deliberate
/// next click (right-click → Rename) — the v1 surface doesn't
/// auto-focus the input.
fn create_variant_action(sid: SectionId) {
    let app = use_store::<AppState>();
    let chosen_cell: std::rc::Rc<std::cell::RefCell<Option<VariantId>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    let cell = chosen_cell.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        if let Ok(v) = add_section_variant(p, sid, "untitled") {
            *cell.borrow_mut() = Some(v);
        }
    }) {
        eprintln!("section editor: create variant failed: {e}");
        return;
    }
    if let Some(new_id) = chosen_cell.borrow().clone() {
        app.set_variant(new_id);
    }
}

/// Commit a rename on the currently-active variant tab. Reads the
/// active variant from `editor_mode` (NOT a captured String) so the
/// `onsubmit` closure only captures Copy types — required by the
/// rinch FnMut-closure boundary inside an `if editing` rsx branch.
///
/// The Rename context-menu item is responsible for activating the
/// targeted tab before flipping `editing` true; this fn always
/// renames whatever's active.
fn commit_rename_active(sid: SectionId, name_input: Signal<String>, editing: Signal<bool>) {
    let app = use_store::<AppState>();
    let old = match app.editor_mode.get() {
        EditorMode::SectionEditor { variant, .. } => variant,
        EditorMode::Arrangement => {
            editing.set(false);
            return;
        }
    };
    let typed = name_input.get();
    let trimmed = typed.trim();
    if trimmed.is_empty() || trimmed == old.as_str() {
        editing.set(false);
        return;
    }
    let new = VariantId::new(trimmed.to_string());
    let new_for_edit = new.clone();
    let old_for_edit = old.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        if let Err(err) = rename_section_variant(p, sid, &old_for_edit, new_for_edit.clone()) {
            eprintln!("section editor: rename variant failed: {err:?}");
        }
    }) {
        eprintln!("section editor: rename variant edit pump failed: {e}");
    }
    // The `rename_section_variant` helper rewrites arrangement-step
    // variant refs but does NOT touch `editor_mode`. Sync it here so
    // the active tab follows the rename.
    if let EditorMode::SectionEditor { section_id, variant } = app.editor_mode.get()
        && section_id == sid
        && variant == old
    {
        app.set_variant(new.clone());
    }
    editing.set(false);
}

fn delete_variant(sid: SectionId, vid: VariantId) {
    let app = use_store::<AppState>();
    let vid_for_edit = vid.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        if let Err(err) = remove_section_variant(p, sid, &vid_for_edit) {
            match err {
                RemoveVariantError::DefaultVariant => {
                    eprintln!(
                        "section editor: refusing to delete default variant '{}' — change \
                         the default first",
                        vid_for_edit.as_str()
                    );
                }
                RemoveVariantError::ReferencedBy(steps) => {
                    let positions: Vec<String> = steps.iter().map(|i| format!("step {i}")).collect();
                    eprintln!(
                        "section editor: refusing to delete variant '{}' — referenced by {}: {}",
                        vid_for_edit.as_str(),
                        if steps.len() == 1 { "arrangement step" } else { "arrangement steps" },
                        positions.join(", "),
                    );
                }
                RemoveVariantError::NotFound => {
                    eprintln!(
                        "section editor: delete variant '{}' failed — variant not found",
                        vid_for_edit.as_str()
                    );
                }
            }
        }
    }) {
        eprintln!("section editor: delete variant edit pump failed: {e}");
    }
    // If we deleted the currently-active variant, switch the tab to
    // the section's (new) default.
    let project = app.project.get();
    if let Some(section) = project.sections.get(&sid)
        && let EditorMode::SectionEditor { section_id, variant } = app.editor_mode.get()
        && section_id == sid
        && variant == vid
    {
        app.set_variant(section.default_variant.clone());
    }
}

fn promote_default(sid: SectionId, vid: VariantId) {
    let app = use_store::<AppState>();
    let vid_for_edit = vid.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        if let Err(err) = set_default_variant(p, sid, &vid_for_edit) {
            eprintln!("section editor: promote default failed: {err:?}");
        }
    }) {
        eprintln!("section editor: promote default edit pump failed: {e}");
    }
}
