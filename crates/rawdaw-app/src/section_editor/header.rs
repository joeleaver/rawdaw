//! Section editor top header: back chevron · breadcrumb · color stripe ·
//! editable section name · subtitle · Duplicate / Done buttons.
//!
//! Mirrors `docs/design/mockups/round-2/components/section-editor.jsx` ·
//! `SectionEditorHeader`. The variant tab strip lives in
//! `variant_tabs.rs` rather than here so each piece stays under the
//! 700-line cap and can be navigated independently.
//!
//! S3 (2026-05-22): inline-rename + Duplicate wired through
//! `section_actions::{rename_section, duplicate_section}`. The
//! NameControl shape — click name → input swap, Enter commits, blank
//! reverts — mirrors the C4 TopBar `NameControl` + the CL1 / P1 / S2
//! Library row implementations.

use std::rc::Rc;

use rinch::core::reactive::{untracked, Effect};
use rinch::prelude::*;

use rawdaw_model::id::SectionId;

use crate::parts::Icon;
use crate::section_actions::{duplicate_section, rename_section};
use crate::state::AppState;
use crate::theme;

#[component]
pub fn SectionEditorHeader(
    section_id: u64,
    section_color: String,
    section_name: String,
) -> NodeHandle {
    let app = use_store::<AppState>();
    let sid = SectionId::new(section_id);

    // Inline-rename state — owned by this component so each editor
    // session keeps its own typing buffer. Synced from the live project
    // via an Effect using the `untracked` peek pattern (the user's
    // mid-edit typing doesn't bounce off external project updates).
    let editing = Signal::new(false);
    let name_input = Signal::new(section_name.clone());
    let _ = Effect::new(move || {
        let project = use_store::<AppState>().project.get();
        let canonical = project
            .sections
            .get(&sid)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        let display = untracked(|| name_input.get());
        if display == canonical {
            return;
        }
        name_input.set(canonical);
    });

    let bar_style = format!(
        "flex: 0 0 auto; \
         border-bottom: 1px solid {line}; background: {bg1}; \
         padding: 10px 20px; \
         display: flex; align-items: center; gap: 12px;",
        line = theme::LINE,
        bg1 = theme::BG1,
    );

    let back_btn_style = format!(
        "width: 26px; height: 26px; padding: 0; \
         display: inline-flex; align-items: center; justify-content: center; \
         background: transparent; border: 1px solid {line}; border-radius: 4px; \
         color: {text1}; cursor: pointer;",
        line = theme::LINE,
        text1 = theme::TEXT1,
    );

    let breadcrumb_style = format!("font-size: 11px; color: {text3};", text3 = theme::TEXT3);

    let stripe_style = format!(
        "width: 4px; height: 18px; border-radius: 2px; \
         background: {col}; flex: 0 0 auto;",
        col = section_color,
    );
    // `title_style` / `title_input_style` are returned from free fns
    // so the `if editing` rsx branch above can call them per-render
    // without consuming a captured String (the chord_loops.rs C4 +
    // S2 sections.rs precedent). Capturing `format!` results in a
    // `let` makes the rsx outer-closure FnOnce.
    let subtitle_style = format!("font-size: 11px; color: {text2};", text2 = theme::TEXT2);

    let dup_style = format!(
        "padding: 5px 10px; border-radius: 4px; \
         background: {bg0}; border: 1px solid {line}; \
         color: {text1}; cursor: pointer; font-size: 11.5px; \
         font-family: inherit;",
        bg0 = theme::BG0,
        line = theme::LINE,
        text1 = theme::TEXT1,
    );
    let done_style = format!(
        "padding: 5px 10px; border-radius: 4px; \
         background: {bg2}; border: 1px solid {line}; \
         color: {text0}; cursor: pointer; font-size: 11.5px; \
         font-weight: 500; font-family: inherit;",
        bg2 = theme::BG2,
        line = theme::LINE,
        text0 = theme::TEXT0,
    );

    let chevron_stroke = theme::TEXT1.to_string();
    let chevron_size = 13.0_f32;
    let chevron_w = 1.6_f32;

    rsx! {
        div { style: {bar_style.clone()},
            button { r#type: "button", style: {back_btn_style.clone()},
                title: "Back to arrangement",
                onclick: move || app.close_section_editor(),
                Icon { glyph: "chevron-r", size: chevron_size,
                       stroke: {chevron_stroke.clone()}, stroke_width: chevron_w }
            }
            span { style: {breadcrumb_style.clone()}, "arrangement · sections ·" }
            span { style: {stripe_style.clone()} }
            if editing.get() {
                input {
                    r#type: "text",
                    title: "Rename section (Enter to commit, blank to revert)",
                    style: {title_input_style()},
                    value: {|| name_input.get()},
                    oninput: move |v: String| name_input.set(v),
                    onsubmit: move || commit_rename(sid, name_input, editing),
                }
            } else {
                h1 {
                    style: {title_style()},
                    title: "Click to rename",
                    onclick: move || editing.set(true),
                    {|| name_input.get()}
                }
            }
            span { style: {subtitle_style.clone()}, "section editor" }
            span { style: "flex: 1;" }
            button { r#type: "button", style: {dup_style.clone()},
                onclick: move || duplicate_action(sid),
                "Duplicate section"
            }
            button { r#type: "button", style: {done_style.clone()},
                onclick: move || app.close_section_editor(),
                "Done"
            }
        }
    }
}

fn title_style() -> String {
    format!(
        "margin: 0; font-size: 16px; font-weight: 700; \
         color: {text0}; letter-spacing: -0.3px; cursor: pointer; \
         user-select: none;",
        text0 = theme::TEXT0,
    )
}

fn title_input_style() -> String {
    format!(
        "margin: 0; height: 24px; padding: 0 6px; font-size: 16px; font-weight: 700; \
         color: {text0}; letter-spacing: -0.3px; \
         background: {bg0}; border: 1px solid {line}; border-radius: 3px; \
         font-family: inherit;",
        text0 = theme::TEXT0,
        bg0 = theme::BG0,
        line = theme::LINE,
    )
}

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
        eprintln!("section editor: rename failed: {e}");
    }
    editing.set(false);
}

fn duplicate_action(id: SectionId) {
    let app = use_store::<AppState>();
    let new_id_cell: Rc<std::cell::Cell<Option<SectionId>>> = Rc::new(std::cell::Cell::new(None));
    let cell = new_id_cell.clone();
    if let Err(e) = app.apply_project_edit(move |p| {
        cell.set(duplicate_section(p, id));
    }) {
        eprintln!("section editor: duplicate failed: {e}");
        return;
    }
    if let Some(new_id) = new_id_cell.get() {
        // Selecting the new section also opens the editor for it
        // (see `AppState::select_section` S3 update).
        app.select_section(Some(new_id));
    }
}
