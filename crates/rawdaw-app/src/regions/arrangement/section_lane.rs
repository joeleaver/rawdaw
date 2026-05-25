//! Section lane: the editable strip of `SectionBlock`s plus the
//! `+ append` button anchored at the right edge.
//!
//! Interactive affordances landed in S5 of
//! `docs/section-arrangement-editing-plan.md`:
//!
//! 1. **Drag-to-move** on each block. The block's onclick registers a
//!    [`Drag::absolute`] (same primitive CL2.x uses for chord-event
//!    timeline drag). `on_move` writes an [`ArrangementDragPreview`]
//!    to [`AppState::arrangement_drag_preview`]; the block reads it
//!    each render to apply `transform: translateX(...)` for live
//!    visual feedback. `on_end` commits via
//!    [`arrangement_actions::move_step`] and clears the preview.
//! 2. **Right-click `ContextMenu` AND a redundant top-left `⋯`
//!    button** — both fire the same Duplicate / Insert before… /
//!    Insert after… / Delete items. S5 originally shipped only the
//!    `⋯` button because rinch v0.3 `ContextMenu`'s `display:
//!    contents` wrapper collapsed percent-positioned children to
//!    `0 × 0` (rinch issue
//!    [#25](https://github.com/joeleaver/rinch/issues/25)). Fixed
//!    upstream in 817aa1c so right-click works now; `⋯` stays
//!    alongside for first-time discoverability.
//! 3. **Variant chip → `DropdownMenu` button** listing the section's
//!    variants (default-decorated). Selecting a variant routes
//!    through `arrangement_actions::set_step_variant`.
//!
//! The lane writes to `AppState::arrangement_drag_preview` (an
//! `Option<ArrangementDragPreview>` signal living on `state.rs`).
//! Pattern + lifetime mirrors [`crate::regions::chord_loop_editor::
//! DragPreview`] exactly so the rinch effect tracker re-evaluates
//! each block's style closure on every drag tick.

use rinch::core::events::{find_click_ancestor, get_click_context, Drag};
use rinch::prelude::*;

/// CSS class the SectionLane carries so `find_click_ancestor` can
/// locate its bounds from inside a SectionBlock's drag handler.
const ARRANGEMENT_SECTION_LANE_CLASS: &str = "arrangement-section-lane";

use rawdaw_model::id::{SectionId, VariantId};

use crate::arrangement_actions::{
    duplicate_step, insert_step_after, insert_step_at, move_step, remove_step,
    set_step_variant,
};

use crate::state::AppState;
use crate::theme;

use super::{
    arrangement_total_bars, build_arrangement_blocks, current_selected_section_id, playhead_bars,
    row_style, ArrangementBlockData,
};

/// Live preview of an in-flight arrangement drag.
///
/// `from_idx` is the index of the block being dragged. `delta_bars`
/// is signed integer bars (snap-to-bar) from the block's committed
/// start. `target_idx` is where the block would land in the post-move
/// arrangement if released right now — clamped to `[0, len)`.
///
/// Mirrors [`crate::regions::chord_loop_editor::DragPreview`] in
/// shape; lives on its own signal so chord-loop and arrangement drags
/// can't cross-contaminate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ArrangementDragPreview {
    pub from_idx: usize,
    pub delta_bars: i32,
    pub target_idx: usize,
}

#[component]
pub(super) fn SectionLane(total_bars: u32) -> NodeHandle {
    let row = row_style(Some(theme::H_LANE), theme::BG0, true);

    // Per-bar guide path (single SVG path, like the ruler).
    let mut guide_d = String::new();
    for i in 0..=total_bars {
        guide_d.push_str(&format!("M {x} 0 L {x} 100 ", x = i));
    }
    let total_bars_str = total_bars.to_string();

    rsx! {
        div {
            style: {row.clone()},
            // Class for `find_click_ancestor` lookups from SectionBlock's
            // drag handler. The handler reads the lane row's pixel
            // width to convert pointer-x → bars.
            class: ARRANGEMENT_SECTION_LANE_CLASS,
            svg {
                viewBox: format!("0 0 {total_bars_str} 100"),
                preserveAspectRatio: "none",
                fill: "none",
                stroke: theme::LINE_SOFT,
                stroke-width: "0.5",
                style: "width: 100%; height: 100%; display: block; \
                        position: absolute; inset: 0; pointer-events: none;",
                path { d: {guide_d.clone()} }
            }
            // Section blocks. `build_arrangement_blocks()` is called
            // inside the for source closure so the iteration stays a
            // `Fn` callable that builds a fresh `Vec` on each render.
            for block in build_arrangement_blocks() {
                SectionBlock {
                    key: block.idx,
                    block: block,
                }
            }
            // Playhead vertical line. Reactive on transport tick +
            // zoom change.
            div {
                style: {|| {
                    let app = use_store::<AppState>();
                    let left_px = playhead_bars() * app.pixels_per_bar.get();
                    format!(
                        "position: absolute; left: {left_px}px; top: 0; bottom: 0; \
                         width: 1px; background: {acc}; opacity: 0.85; \
                         transform: translateX(-0.5px); pointer-events: none;",
                        acc = theme::ACCENT,
                    )
                }},
            }
        }
    }
}

#[component]
fn SectionBlock(block: ArrangementBlockData) -> NodeHandle {
    let app = use_store::<AppState>();
    let ArrangementBlockData {
        idx,
        section_id,
        section_name,
        color,
        variant,
        default_variant,
        start_bar,
        bars,
    } = block;

    let internal_bars = bars.saturating_sub(1);
    let mut inner_d = String::new();
    for i in 1..=internal_bars {
        inner_d.push_str(&format!("M {x} 6 L {x} 94 ", x = i));
    }
    let inner_stroke = with_alpha(color.as_str(), 0.22);
    let bars_str = bars.to_string();

    let length_text = format!("{} {}", bars, if bars == 1 { "bar" } else { "bars" });
    let color_for_style = color.clone();
    let color_for_chip = color.clone();
    let variant_for_chip = variant.clone();
    let default_variant_for_chip = default_variant.clone();

    rsx! {
        ContextMenu {
            ContextMenuTarget {
                div {
                    style: {
                        let sel = app.selected_idx.get();
                        let is_selected = sel == Some(idx);
                        // is_linked: highlight every block that references the
                        // same section as the selected block. Reads model
                        // signals through `current_selected_section_id`.
                        let is_linked = !is_selected
                            && current_selected_section_id() == Some(section_id);
                        // Drag preview offset: translate the block by
                        // (delta_bars / bars) * 100% of its own width when
                        // it's the active drag target. Other blocks render
                        // statically — sibling shift is round-2 polish.
                        let preview = app.arrangement_drag_preview.get();
                        let is_dragging = preview
                            .as_ref()
                            .map(|p| p.from_idx == idx)
                            .unwrap_or(false);
                        let px_per_bar = app.pixels_per_bar.get();
                        let left_px = start_bar as f32 * px_per_bar;
                        let width_px = bars as f32 * px_per_bar;
                        // Drag preview offset is in bars too — multiply by
                        // px_per_bar to get the live transform offset.
                        let translate_px = preview
                            .as_ref()
                            .filter(|p| p.from_idx == idx)
                            .map(|p| p.delta_bars as f32 * px_per_bar)
                            .unwrap_or(0.0);

                        let bg = with_alpha(
                            color_for_style.as_str(),
                            if is_selected {
                                0.20
                            } else if is_linked {
                                0.14
                            } else {
                                0.10
                            },
                        );
                        let border = if is_selected {
                            format!("1px solid {}", color_for_style)
                        } else if is_linked {
                            format!("1px solid {}", with_alpha(color_for_style.as_str(), 0.55))
                        } else {
                            format!("1px solid {}", with_alpha(color_for_style.as_str(), 0.28))
                        };
                        let box_shadow = if is_dragging {
                            format!(
                                "0 4px 14px {}, 0 0 0 1px {}",
                                with_alpha(color_for_style.as_str(), 0.45),
                                with_alpha(color_for_style.as_str(), 0.55),
                            )
                        } else if is_selected {
                            format!("0 0 0 1px {}", with_alpha(color_for_style.as_str(), 0.30))
                        } else {
                            "none".to_string()
                        };
                        let opacity = if is_dragging { "0.92" } else { "1" };
                        let z_index = if is_dragging { "3" } else { "1" };
                        // Pixel-based positioning — `left` and `width`
                        // are derived from start_bar / bars times the
                        // current `pixels_per_bar`. The lane's
                        // scrolled-content wrapper applies the
                        // horizontal scroll translate to the entire
                        // group, so blocks don't need to know about
                        // scroll themselves.
                        format!(
                            "position: absolute; left: {left_px}px; top: 6px; \
                             width: {width_px}px; bottom: 6px; \
                             box-sizing: border-box; \
                             border-left: 3px solid {col}; border-radius: 3px; \
                             background: {bg}; border: {border}; \
                             box-shadow: {box_shadow}; \
                             transform: translateX({translate_px:.2}px); \
                             opacity: {opacity}; z-index: {z_index}; \
                             cursor: pointer; overflow: visible;",
                            col = color_for_style,
                        )
                    },
                    onclick: move || on_block_click(idx, bars),
                    svg {
                        viewBox: format!("0 0 {bars_str} 100"),
                        preserveAspectRatio: "none",
                        fill: "none",
                        stroke: {inner_stroke.clone()},
                        stroke-width: "0.5",
                        style: "width: 100%; height: 100%; display: block; \
                                position: absolute; inset: 0; pointer-events: none;",
                        path { d: {inner_d.clone()} }
                    }
                    // Actions menu (top-left, sits in front of the name).
                    BlockActionsMenu { step_idx: idx }
                    // Name (top-left, after the menu button).
                    div {
                        style: "position: absolute; left: 28px; top: 6px; \
                                font-size: 12.5px; font-weight: 600; \
                                color: rgba(232,234,238,0.96); letter-spacing: -0.1px; \
                                pointer-events: none;",
                        {section_name.clone()}
                    }
                    // Variant chip — DropdownMenu button. Top-right.
                    VariantChipSelect {
                        step_idx: idx,
                        section_id_u64: section_id.get(),
                        current_variant: variant_for_chip,
                        default_variant: default_variant_for_chip,
                        color: color_for_chip,
                    }
                    // Length readout (bottom-right).
                    div {
                        style: "position: absolute; right: 6px; bottom: 4px; \
                                font-size: 10px; color: rgba(232,234,238,0.62); \
                                font-feature-settings: \"tnum\" 1; \
                                font-variant-numeric: tabular-nums; \
                                letter-spacing: 0.2px; pointer-events: none;",
                        {length_text.clone()}
                    }
                }
            }
            ContextMenuDropdown {
                DropdownMenuItem {
                    onclick: move || duplicate_action(idx),
                    "Duplicate"
                }
                DropdownMenuDivider {}
                DropdownMenuItem {
                    onclick: move || insert_before_action(idx),
                    "Insert section before…"
                }
                DropdownMenuItem {
                    onclick: move || insert_after_action(idx),
                    "Insert section after…"
                }
                DropdownMenuDivider {}
                DropdownMenuItem {
                    onclick: move || delete_action(idx),
                    "Delete"
                }
            }
        }
    }
}

/// Per-block `⋯` button at the top-left of the section block. Opens
/// a [`DropdownMenu`] with the same Duplicate / Insert before /
/// Insert after / Delete items the wrapping right-click
/// [`ContextMenu`] fires. Keeping both: the button is the
/// discoverable affordance for first-time users; right-click is the
/// power-user shortcut.
#[component]
fn BlockActionsMenu(step_idx: usize) -> NodeHandle {
    let menu_open = Signal::new(false);

    rsx! {
        div { style: {actions_menu_wrapper_style()},
            DropdownMenu {
                opened_fn: move || menu_open.get(),
                on_close: move || menu_open.set(false),
                DropdownMenuTarget {
                    button {
                        r#type: "button",
                        title: "Section block actions",
                        style: {actions_btn_style()},
                        onclick: move || menu_open.update(|v| *v = !*v),
                        "⋯"
                    }
                }
                DropdownMenuDropdown {
                    DropdownMenuItem {
                        onclick: move || {
                            duplicate_action(step_idx);
                            menu_open.set(false);
                        },
                        "Duplicate"
                    }
                    DropdownMenuDivider {}
                    DropdownMenuItem {
                        onclick: move || {
                            insert_before_action(step_idx);
                            menu_open.set(false);
                        },
                        "Insert section before…"
                    }
                    DropdownMenuItem {
                        onclick: move || {
                            insert_after_action(step_idx);
                            menu_open.set(false);
                        },
                        "Insert section after…"
                    }
                    DropdownMenuDivider {}
                    DropdownMenuItem {
                        onclick: move || {
                            delete_action(step_idx);
                            menu_open.set(false);
                        },
                        "Delete"
                    }
                }
            }
        }
    }
}

fn actions_menu_wrapper_style() -> String {
    "position: absolute; left: 5px; top: 5px; \
     z-index: 3; pointer-events: auto;".to_string()
}

fn actions_btn_style() -> String {
    format!(
        "width: 18px; height: 18px; padding: 0; \
         display: inline-flex; align-items: center; justify-content: center; \
         border-radius: 3px; background: rgba(232,234,238,0.05); \
         border: 1px solid {border}; color: {text2}; \
         font-size: 12px; line-height: 1; cursor: pointer; \
         font-family: inherit;",
        border = theme::LINE_SOFT,
        text2 = theme::TEXT2,
    )
}

// ─── Drag-to-move ─────────────────────────────────────────────────────────

/// Onclick handler: select the block and start a [`Drag::absolute`].
/// The drag captures the lane layout snapshot at click time so the
/// `target_idx` computation in `on_move` doesn't have to re-walk the
/// arrangement signal each frame.
fn on_block_click(idx: usize, bars: u32) {
    let app = use_store::<AppState>();
    app.set_selected_idx(Some(idx));

    let ctx = get_click_context();
    let start_x = ctx.mouse_x;
    // Read the lane's pixel width via `find_click_ancestor` (rinch
    // #29.2). `bars_per_px = total_bars / lane_width` is the
    // canonical scale — the dragged block's `bars` aren't load-
    // bearing for the drag math any more, so the helper takes the
    // block's own width as a fallback only.
    let lane_width = find_click_ancestor(|a| a.has_class(ARRANGEMENT_SECTION_LANE_CLASS))
        .map(|a| a.width.max(1.0))
        .unwrap_or_else(|| {
            // Defensive fallback: if the lane ancestor isn't found
            // (which shouldn't happen — the lane is always the
            // SectionBlock's positioned ancestor), reuse the pre-
            // rinch-#29 derivation: `bars_per_px = bars /
            // block_width`, which gives the same scale because
            // `block_width / lane_width == bars / total_bars`.
            let block_width = ctx.element_width.max(1.0);
            // Returning a synthetic lane_width that makes the
            // subsequent total_bars-based calc produce the right
            // ratio. lane_width = total_bars * block_width / bars.
            // We don't have total_bars here — use the block-relative
            // form directly instead by encoding it in the final
            // expression. See below.
            block_width * arrangement_total_bars().max(1) as f32 / bars.max(1) as f32
        });
    let total_bars = arrangement_total_bars().max(1) as f32;
    let bars_per_px = total_bars / lane_width;

    // Snapshot the layout: (idx, start_bar + bars/2) for each step.
    // Used to find the closest sibling when computing target_idx.
    let layout: Vec<(usize, i32)> = build_arrangement_blocks()
        .into_iter()
        .map(|b| (b.idx, b.start_bar as i32 + b.bars as i32 / 2))
        .collect();

    // Source center, derived from the same snapshot so the math is
    // self-consistent under bar rounding.
    let src_center = layout
        .iter()
        .find(|(i, _)| *i == idx)
        .map(|(_, c)| *c)
        .unwrap_or(0);

    start_drag(idx, start_x, bars_per_px, layout, src_center);
}

fn start_drag(
    from_idx: usize,
    start_x: f32,
    bars_per_px: f32,
    layout: Vec<(usize, i32)>,
    src_center: i32,
) {
    let preview_signal = use_store::<AppState>().arrangement_drag_preview;
    let layout_for_move = layout.clone();
    let layout_for_end = layout;
    Drag::absolute()
        .on_move(move |x, _y| {
            let delta_bars = ((x - start_x) * bars_per_px).round() as i32;
            let target_idx = target_idx_for_delta(
                from_idx,
                src_center,
                delta_bars,
                &layout_for_move,
            );
            preview_signal.set(Some(ArrangementDragPreview {
                from_idx,
                delta_bars,
                target_idx,
            }));
        })
        .on_end(move |x, _y| {
            let delta_bars = ((x - start_x) * bars_per_px).round() as i32;
            let target_idx = target_idx_for_delta(
                from_idx,
                src_center,
                delta_bars,
                &layout_for_end,
            );
            preview_signal.set(None);
            if target_idx == from_idx {
                return;
            }
            let app = use_store::<AppState>();
            let result =
                app.apply_project_edit(move |p| move_step(p, from_idx, target_idx));
            if let Err(e) = result {
                eprintln!("arrangement: move_step commit failed: {e}");
            }
        })
        .start();
}

/// Pick the closest sibling whose center the dragged block's new
/// center is nearest to. Returns the target post-move index.
/// `from_idx` itself is excluded so a sub-bar wobble doesn't keep
/// committing the block to its current position; the no-op short-
/// circuit lives at the commit site.
fn target_idx_for_delta(
    from_idx: usize,
    src_center: i32,
    delta_bars: i32,
    layout: &[(usize, i32)],
) -> usize {
    let new_center = src_center + delta_bars;
    // For zero delta the answer is from_idx (no move). For small deltas
    // that don't cross a neighbor's midpoint, the closest center is
    // still `from_idx` — so the commit no-ops correctly.
    let mut best_idx = from_idx;
    let mut best_dist = i32::MAX;
    for (i, center) in layout {
        let dist = (new_center - *center).abs();
        if dist < best_dist {
            best_dist = dist;
            best_idx = *i;
        }
    }
    best_idx
}

// ─── ContextMenu commit handlers ──────────────────────────────────────────

fn duplicate_action(idx: usize) {
    let app = use_store::<AppState>();
    if let Err(e) = app.apply_project_edit(move |p| {
        let _ = duplicate_step(p, idx);
    }) {
        eprintln!("arrangement: duplicate_step failed: {e}");
    }
}

fn delete_action(idx: usize) {
    let app = use_store::<AppState>();
    if let Err(e) = app.apply_project_edit(move |p| remove_step(p, idx)) {
        eprintln!("arrangement: remove_step failed: {e}");
    }
}

/// "Insert section before…" — pick the section to use as
/// `section_id` then commit via [`insert_step_at`]. Until the
/// section-picker popover lands (also S5), the v1 path is a
/// chain through `selected_section`: if a section is currently
/// selected in the library, use it; otherwise eprintln a hint.
fn insert_before_action(idx: usize) {
    let app = use_store::<AppState>();
    let Some((section_id, default_variant)) = picked_section_or_warn() else {
        return;
    };
    if let Err(e) = app.apply_project_edit(move |p| {
        let _ = insert_step_at(p, idx, section_id, default_variant.clone());
    }) {
        eprintln!("arrangement: insert_step_at failed: {e}");
    }
}

/// "Insert section after…" — same as insert_before, but routes via
/// [`insert_step_after`].
fn insert_after_action(idx: usize) {
    let app = use_store::<AppState>();
    let Some((section_id, default_variant)) = picked_section_or_warn() else {
        return;
    };
    if let Err(e) = app.apply_project_edit(move |p| {
        let _ = insert_step_after(p, idx, section_id, default_variant.clone());
    }) {
        eprintln!("arrangement: insert_step_after failed: {e}");
    }
}

/// Resolve the section to insert. v1 reads
/// [`AppState::selected_section`] so the workflow is "click a
/// Library row to pick the section, then right-click a block to
/// insert it." A future bite can swap this for an inline popover
/// picker on the context-menu item itself.
fn picked_section_or_warn() -> Option<(SectionId, VariantId)> {
    let app = use_store::<AppState>();
    let Some(sid) = app.selected_section.get() else {
        eprintln!(
            "arrangement: insert needs a section selected in the Library — \
             click a section row first.",
        );
        return None;
    };
    let project = app.project.get();
    let section = project.sections.get(&sid)?;
    Some((sid, section.default_variant.clone()))
}

// ─── Variant chip Select ──────────────────────────────────────────────────

/// Compact variant chip on the top-right of a `SectionBlock`. Click
/// opens a `DropdownMenu` listing the section's variants; selecting
/// one routes through [`set_step_variant`].
///
/// Earlier S5 iterations used an inline `Select` here, but the
/// `Select` component has a fixed natural width that overflows the
/// 4-bar block widths in the demo arrangement, swamping the section
/// name label. The button-triggered dropdown matches the AppendButton
/// pattern: the chip sizes to its label, the option list opens as a
/// portal-rendered popup that's free to extend beyond the block.
///
/// Component takes the section_id as `u64` because rinch's
/// `#[component]` macro can't derive `Default` for prop types it
/// doesn't know how to default — `u64` keeps the surface universal.
#[component]
fn VariantChipSelect(
    step_idx: usize,
    section_id_u64: u64,
    current_variant: String,
    default_variant: String,
    color: String,
) -> NodeHandle {
    let section_id = SectionId::new(section_id_u64);
    let menu_open = Signal::new(false);
    let is_default = current_variant == default_variant;

    let chip_bg = with_alpha(color.as_str(), if is_default { 0.20 } else { 0.32 });
    let chip_border = with_alpha(color.as_str(), if is_default { 0.36 } else { 0.55 });
    let chip_label = format!("{current_variant} ▾");
    let chip_style = format!(
        "position: absolute; right: 6px; top: 5px; \
         height: 22px; padding: 0 8px; \
         display: inline-flex; align-items: center; \
         border-radius: 4px; background: {chip_bg}; \
         border: 1px solid {chip_border}; \
         color: rgba(232,234,238,0.94); font-size: 12px; \
         font-weight: 500; letter-spacing: 0.2px; \
         font-family: inherit; cursor: pointer; \
         max-width: calc(100% - 12px); \
         overflow: hidden; text-overflow: ellipsis; white-space: nowrap; \
         pointer-events: auto; z-index: 2;",
    );

    rsx! {
        DropdownMenu {
            opened_fn: move || menu_open.get(),
            on_close: move || menu_open.set(false),
            DropdownMenuTarget {
                button {
                    r#type: "button",
                    title: "Change variant",
                    style: {chip_style.clone()},
                    onclick: move || menu_open.update(|v| *v = !*v),
                    {chip_label.clone()}
                }
            }
            DropdownMenuDropdown {
                for opt in variant_picker_options(section_id, default_variant.clone()) {
                    DropdownMenuItem {
                        key: opt.value.clone(),
                        onclick: move || {
                            commit_variant_change(step_idx, opt.value.clone());
                            menu_open.set(false);
                        },
                        {opt.label.clone()}
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Default)]
struct VariantPickerOption {
    value: String,
    label: String,
}

/// Build the variant-picker option list for a section. Labels the
/// section's default with a `(default)` suffix so the user can read
/// which option matches at a glance.
fn variant_picker_options(
    section_id: SectionId,
    default_variant: String,
) -> Vec<VariantPickerOption> {
    let app = use_store::<AppState>();
    let project = app.project.get();
    let Some(section) = project.sections.get(&section_id) else {
        // Section gone (rare race). Empty list lets the dropdown render
        // safely with no items.
        return Vec::new();
    };
    let mut out = Vec::new();
    let base_is_default = section.default_variant.as_str() == VariantId::BASE_STR;
    let base_label = if base_is_default {
        format!("{} (default)", VariantId::BASE_STR)
    } else {
        VariantId::BASE_STR.to_string()
    };
    out.push(VariantPickerOption {
        value: VariantId::BASE_STR.to_string(),
        label: base_label,
    });
    for v in section.variants.keys() {
        let is_default = v.as_str() == default_variant;
        let label = if is_default {
            format!("{} (default)", v.as_str())
        } else {
            v.as_str().to_string()
        };
        out.push(VariantPickerOption {
            value: v.as_str().to_string(),
            label,
        });
    }
    out
}

fn commit_variant_change(step_idx: usize, value: String) {
    let app = use_store::<AppState>();
    let v = VariantId::new(value);
    if let Err(e) = app.apply_project_edit(move |p| set_step_variant(p, step_idx, v.clone())) {
        eprintln!("arrangement: set_step_variant failed: {e}");
    }
}

