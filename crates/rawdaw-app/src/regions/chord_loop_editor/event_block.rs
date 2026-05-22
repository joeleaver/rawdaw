//! One chord-event cell on the timeline. Sized proportionally to
//! the event's duration relative to the loop length; carries the
//! Roman + absolute labels and the focused-state highlight.
//!
//! Focus state lives on [`AppState::focused_chord_event_idx`] so
//! the block reads it reactively and the timeline + inspector share
//! a single source of truth.
//!
//! ## Drag-to-move + resize (CL2.x)
//!
//! Clicking the body of the block focuses it AND starts a
//! [`Drag::absolute`] for moving the event along the timeline.
//! Clicking the rightmost ~15% of the block starts a resize drag
//! that grows/shrinks the duration from the right edge. Live
//! preview is written to [`AppState::drag_preview`] on each
//! `on_move` invocation; the commit through
//! `chord_loop_actions::{move_chord_event, resize_chord_event}`
//! lands once at `on_end`.
//!
//! The block's own `element_width` (captured at click time) is the
//! pixel scale for the px→ticks conversion: `ticks_per_px =
//! duration_ticks / element_width`. That keeps the conversion
//! self-contained — we never need the parent strip's bounds.

use rinch::core::events::{get_click_context, Drag};
use rinch::prelude::*;

use rawdaw_model::id::ChordLoopId;

use crate::chord_loop_actions::{move_chord_event, resize_chord_event};

use crate::regions::chord_loop_editor::{DragKind, DragPreview};
use crate::state::AppState;
use crate::theme;

/// Fraction of the block's width reserved for the resize-handle hit
/// zone on the right edge. Click anywhere inside this zone to start
/// a resize drag instead of a move drag.
const RESIZE_HANDLE_FRAC: f32 = 0.15;

/// Minimum block width (in pixels) that activates the resize zone.
/// Blocks narrower than this collapse the resize zone — otherwise a
/// 24-px-wide block would have a sub-4-px resize zone that's
/// impossible to click reliably.
const RESIZE_HANDLE_MIN_PX: f32 = 40.0;

#[component]
pub(super) fn EventBlock(
    idx: usize,
    loop_id: ChordLoopId,
    duration_ticks: i64,
    roman: String,
    absolute: String,
    width_frac: f32,
    color: String,
) -> NodeHandle {
    let color_for_style = color.clone();
    rsx! {
        div {
            style: {|| {
                let app = use_store::<AppState>();
                let focused = app.focused_chord_event_idx.get() == Some(idx);
                let preview = app.drag_preview.get();
                let (translate_pct, extra_pct) = preview_offsets(
                    preview, loop_id, idx, duration_ticks, width_frac,
                );
                let pct = (width_frac * 100.0 + extra_pct).max(0.5);
                let border_color = with_alpha(color_for_style.as_str(), 0.45);
                let bg = if focused {
                    with_alpha(color_for_style.as_str(), 0.20)
                } else {
                    with_alpha(color_for_style.as_str(), 0.08)
                };
                let stripe = if focused {
                    format!("3px solid {}", color_for_style)
                } else {
                    format!("1px solid {}", border_color)
                };
                // Resize handle hint: thin vertical strip at the
                // right edge. Drawn via border-right so the block
                // itself still owns the click target.
                let handle_hint = format!("border-right: 4px solid {};", border_color);
                format!(
                    "flex: 0 0 {pct}%; min-width: 24px; \
                     transform: translateX({translate_pct:.2}%); \
                     box-sizing: border-box; \
                     padding: 4px 6px; margin-right: 2px; \
                     background: {bg}; \
                     border: 1px solid {border_color}; \
                     border-left: {stripe}; {handle_hint} \
                     border-radius: 3px; \
                     cursor: pointer; position: relative; \
                     display: flex; flex-direction: column; \
                     justify-content: center; gap: 2px;",
                )
            }},
            onclick: move || on_block_click(loop_id, idx, duration_ticks),
            div {
                style: format!(
                    "font-size: 13px; font-weight: 600; \
                     letter-spacing: 0.4px; color: {text}; \
                     font-feature-settings: \"tnum\" 1; line-height: 1; \
                     pointer-events: none;",
                    text = theme::TEXT0,
                ),
                {roman.clone()}
            }
            div {
                style: format!(
                    "font-size: 10px; color: {text2}; \
                     font-feature-settings: \"tnum\" 1; line-height: 1; \
                     overflow: hidden; text-overflow: ellipsis; \
                     white-space: nowrap; pointer-events: none;",
                    text2 = theme::TEXT2,
                ),
                {absolute.clone()}
            }
        }
    }
}

/// Compute the live-preview transform offsets for the block:
/// `(translate_pct, extra_width_pct)`. Move drags translate the
/// block via `transform: translateX(N%)` (N is delta as a percent
/// of the block's own width); resize drags grow/shrink the block's
/// flex basis by adding to its percentage of the strip's width.
fn preview_offsets(
    preview: Option<DragPreview>,
    loop_id: ChordLoopId,
    idx: usize,
    duration_ticks: i64,
    width_frac: f32,
) -> (f32, f32) {
    let Some(p) = preview else { return (0.0, 0.0) };
    if p.loop_id != loop_id || p.event_idx != idx {
        return (0.0, 0.0);
    }
    let dur = duration_ticks.max(1) as f32;
    match p.kind {
        DragKind::Move => {
            let frac = p.delta_ticks as f32 / dur;
            (frac * 100.0, 0.0)
        }
        DragKind::Resize => {
            // delta_ticks affects total duration; the block's width as
            // a percent of the strip = width_frac * (1 + delta/dur).
            // Extra percent = width_frac * delta/dur * 100.
            let frac = p.delta_ticks as f32 / dur;
            (0.0, width_frac * frac * 100.0)
        }
    }
}

/// Decide whether the click lands in the resize-handle zone (right
/// edge) or the move zone (everything else), then start the
/// appropriate drag. Either way the event is focused first so the
/// inspector follows.
fn on_block_click(loop_id: ChordLoopId, idx: usize, duration_ticks: i64) {
    let app = use_store::<AppState>();
    app.focused_chord_event_idx.set(Some(idx));

    let ctx = get_click_context();
    let block_width = ctx.element_width.max(1.0);
    let start_x = ctx.mouse_x;
    let ticks_per_px = duration_ticks as f32 / block_width;

    // Resize zone is the rightmost RESIZE_HANDLE_FRAC of the block,
    // but only if the block is wide enough that the zone is at least
    // RESIZE_HANDLE_MIN_PX wide (so narrow blocks don't accidentally
    // collapse into "only resize, never move").
    let resize_zone_px = (block_width * RESIZE_HANDLE_FRAC).max(RESIZE_HANDLE_MIN_PX);
    let in_resize_zone =
        block_width >= RESIZE_HANDLE_MIN_PX && ctx.mouse_x - ctx.element_x >= block_width - resize_zone_px;

    let kind = if in_resize_zone { DragKind::Resize } else { DragKind::Move };
    start_drag(loop_id, idx, kind, start_x, ticks_per_px);
}

fn start_drag(
    loop_id: ChordLoopId,
    event_idx: usize,
    kind: DragKind,
    start_x: f32,
    ticks_per_px: f32,
) {
    let preview_signal = use_store::<AppState>().drag_preview;
    Drag::absolute()
        .on_move(move |x, _y| {
            let delta_px = x - start_x;
            let delta_ticks = (delta_px * ticks_per_px) as i64;
            preview_signal.set(Some(DragPreview {
                loop_id,
                event_idx,
                kind,
                delta_ticks,
            }));
        })
        .on_end(move |x, _y| {
            let delta_px = x - start_x;
            let delta_ticks = (delta_px * ticks_per_px) as i64;
            preview_signal.set(None);
            if delta_ticks == 0 {
                return;
            }
            let app = use_store::<AppState>();
            let result = app.apply_project_edit(move |p| match kind {
                DragKind::Move => move_chord_event(p, loop_id, event_idx, delta_ticks),
                DragKind::Resize => resize_chord_event(p, loop_id, event_idx, delta_ticks),
            });
            if let Err(e) = result {
                eprintln!("chord_loop_editor: drag commit failed: {e}");
            }
        })
        .start();
}
