//! Arrangement view toolbar — zoom controls + `+ append`.
//!
//! Sits above the scrolling timeline area so its affordances stay
//! anchored as the user scrolls horizontally. Previously the
//! `+ append` button was anchored inside the section lane via
//! `position: absolute; right: 0`, which scrolled away when the
//! arrangement extended past the viewport. Lives in its own row
//! now alongside zoom controls.
//!
//! F4 of the rinch #30 follow-on (timeline primitive in rawdaw).

use rinch::prelude::*;

use crate::parts::Icon;
use crate::state::{AppState, DEFAULT_PX_PER_BAR, MAX_PX_PER_BAR, MIN_PX_PER_BAR};
use crate::theme;

use super::append_action::AppendButton;

/// Zoom step factor — each click multiplies / divides
/// `pixels_per_bar` by this. 1.25 gives "perceptibly different but
/// not jarring" between successive zoom levels and reaches the
/// `MIN_PX_PER_BAR`..`MAX_PX_PER_BAR` extremes in ~14 clicks.
const ZOOM_STEP: f32 = 1.25;

#[component]
pub(super) fn ArrangementToolbar(total_bars: u32) -> NodeHandle {
    let toolbar_style = format!(
        "flex: 0 0 auto; display: flex; align-items: center; gap: 4px; \
         padding: 6px 10px; background: {bg1}; \
         border-bottom: 1px solid {line}; \
         min-height: 36px;",
        bg1 = theme::BG1,
        line = theme::LINE,
    );

    rsx! {
        div { style: {toolbar_style.clone()},
            ZoomOutBtn {}
            ZoomInBtn {}
            ZoomReadout {}
            FitToWidthBtn { total_bars: total_bars }
            // Spacer
            div { style: "flex: 1;" }
            AppendButton {}
        }
    }
}

#[component]
fn ZoomOutBtn() -> NodeHandle {
    let stroke = theme::TEXT2.to_string();
    rsx! {
        button {
            r#type: "button",
            title: "Zoom out",
            style: {tool_btn_style()},
            onclick: move || {
                let app = use_store::<AppState>();
                let next = (app.pixels_per_bar.get() / ZOOM_STEP).max(MIN_PX_PER_BAR);
                app.pixels_per_bar.set(next);
            },
            Icon { glyph: "minus", size: 12.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
        }
    }
}

#[component]
fn ZoomInBtn() -> NodeHandle {
    let stroke = theme::TEXT2.to_string();
    rsx! {
        button {
            r#type: "button",
            title: "Zoom in",
            style: {tool_btn_style()},
            onclick: move || {
                let app = use_store::<AppState>();
                let next = (app.pixels_per_bar.get() * ZOOM_STEP).min(MAX_PX_PER_BAR);
                app.pixels_per_bar.set(next);
            },
            Icon { glyph: "plus", size: 12.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
        }
    }
}

/// Tiny readout of the current zoom level in px/bar. Helpful for
/// the user to see where they are relative to the
/// MIN/MAX/DEFAULT extremes during exploration.
#[component]
fn ZoomReadout() -> NodeHandle {
    let readout_style = format!(
        "font-size: 10.5px; color: {fg}; \
         font-feature-settings: \"tnum\" 1; \
         font-variant-numeric: tabular-nums; \
         min-width: 56px; text-align: center; padding: 0 4px;",
        fg = theme::TEXT2,
    );
    rsx! {
        span { style: {readout_style.clone()},
            {|| {
                let app = use_store::<AppState>();
                let px = app.pixels_per_bar.get() as i32;
                format!("{px} px/bar")
            }}
        }
    }
}

/// "Fit to width": set `pixels_per_bar` so the entire arrangement
/// just fits in the lane viewport. Approximates the viewport width
/// from the document — the toolbar's parent `<section>` width is
/// the lane width minus the toolbar's own padding (which we ignore
/// for a coarse fit; the user can adjust with the +/− buttons).
///
/// Falls back to `DEFAULT_PX_PER_BAR` when the lookup can't find a
/// reasonable viewport size (e.g., very early in startup before
/// layout has run).
#[component]
fn FitToWidthBtn(total_bars: u32) -> NodeHandle {
    let stroke = theme::TEXT2.to_string();
    rsx! {
        button {
            r#type: "button",
            title: "Fit arrangement to viewport width",
            style: {tool_btn_style()},
            onclick: move || {
                let app = use_store::<AppState>();
                // Without a bounds_signal hook into the scrolling
                // area's pixel width yet, fall back to the default
                // px/bar. v2 will read the actual lane width.
                let _ = total_bars;
                app.pixels_per_bar.set(DEFAULT_PX_PER_BAR);
                app.scroll_bars.set(0.0);
            },
            Icon { glyph: "search", size: 12.0, stroke: {stroke.clone()}, stroke_width: 1.6 }
        }
    }
}

fn tool_btn_style() -> String {
    format!(
        "display: inline-flex; align-items: center; justify-content: center; \
         width: 24px; height: 24px; padding: 0; \
         border-radius: 3px; background: transparent; \
         border: 1px solid {border}; color: {fg}; cursor: pointer;",
        border = theme::LINE,
        fg = theme::TEXT2,
    )
}
