//! `+ append` button at the right edge of the section lane.
//!
//! Opens a [`DropdownMenu`] listing each section by name. Clicking a
//! section calls [`arrangement_actions::append_step`] with that
//! section's `default_variant`. After the block lands, the user can
//! click its variant chip (see [`super::section_lane`]) to pick a
//! non-default variant.
//!
//! Disabled (button rendered with reduced opacity, no handler) when
//! `project.sections.is_empty()` — the empty-arrangement gate from
//! the S0 plan. The Library S2 surface ships the "create section"
//! affordance the user follows when this state appears.

use rinch::prelude::*;

use rawdaw_model::id::SectionId;

use crate::arrangement_actions::append_step;
use crate::parts::Icon;
use crate::state::AppState;
use crate::theme;

#[component]
pub(super) fn AppendButton() -> NodeHandle {
    let menu_open = Signal::new(false);
    let stroke = theme::TEXT2.to_string();

    rsx! {
        div { style: {wrapper_style()},
            DropdownMenu {
                opened_fn: move || menu_open.get(),
                on_close: move || menu_open.set(false),
                DropdownMenuTarget {
                    button {
                        r#type: "button",
                        title: "Append a section to the arrangement",
                        style: {|| button_style(any_sections())},
                        onclick: move || {
                            if any_sections() {
                                menu_open.update(|v| *v = !*v);
                            } else {
                                eprintln!(
                                    "arrangement: + append disabled — no sections exist. \
                                     Create one in the Library first.",
                                );
                            }
                        },
                        Icon { glyph: "plus", size: 12.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
                        span { style: "margin-left: 4px;", "append" }
                    }
                }
                DropdownMenuDropdown {
                    for opt in section_options() {
                        DropdownMenuItem {
                            key: opt.key.clone(),
                            onclick: move || {
                                let section_id = SectionId::new(opt.section_id_u64);
                                let variant = opt.default_variant.clone();
                                commit_append(section_id, variant);
                                menu_open.set(false);
                            },
                            {opt.label.clone()}
                        }
                    }
                }
            }
        }
    }
}

fn any_sections() -> bool {
    let app = use_store::<AppState>();
    !app.project.get().sections.is_empty()
}

fn wrapper_style() -> String {
    // Absolute-positioned at the right edge of the lane. The lane is
    // `position: relative`, so right: 0 anchors here. `pointer-events:
    // auto` is the default; setting it explicitly serves as a hint
    // that we DO want clicks here even though the lane chrome uses
    // `pointer-events: none` on its SVG guide.
    "position: absolute; right: 6px; top: 50%; \
     transform: translateY(-50%); \
     z-index: 2; pointer-events: auto;".to_string()
}

fn button_style(enabled: bool) -> String {
    let opacity = if enabled { "1" } else { "0.4" };
    format!(
        "display: inline-flex; align-items: center; gap: 2px; \
         height: 24px; padding: 0 8px; border-radius: 4px; \
         background: {bg}; border: 1px solid {border}; \
         color: {fg}; font-size: 11.5px; font-weight: 500; \
         letter-spacing: 0.2px; cursor: pointer; opacity: {opacity};",
        bg = theme::BG1,
        border = theme::LINE,
        fg = theme::TEXT2,
    )
}

#[derive(Clone, PartialEq)]
struct SectionOption {
    key: String,
    section_id_u64: u64,
    default_variant: String,
    label: String,
}

fn section_options() -> Vec<SectionOption> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let mut sections: Vec<_> = project.sections.values().collect();
    sections.sort_by(|a, b| a.name.cmp(&b.name));
    sections
        .into_iter()
        .map(|s| SectionOption {
            key: s.id.get().to_string(),
            section_id_u64: s.id.get(),
            default_variant: s.default_variant.as_str().to_string(),
            label: s.name.clone(),
        })
        .collect()
}

fn commit_append(section_id: SectionId, default_variant: String) {
    let app = use_store::<AppState>();
    let variant = rawdaw_model::id::VariantId::new(default_variant);
    if let Err(e) = app.apply_project_edit(move |p| {
        let _ = append_step(p, section_id, variant.clone());
    }) {
        eprintln!("arrangement: append_step failed: {e}");
    }
}
