//! Pure bar↔pixel projection helpers reading the AppState timeline
//! signals.
//!
//! All three helpers are read-only on [`AppState`] and don't
//! allocate, so they're cheap to call inside rsx style closures
//! every render.
//!
//! **Phase state.** Scaffolded in F4a but the v1 implementation
//! ended up doing the bar→px math inline (just
//! `bar * pixels_per_bar`) because each row's children sit inside
//! a scrolled-content wrapper that already applies the scroll
//! offset. `bar_to_px` / `px_to_bar` will be used by the cursor-
//! anchored zoom + the future scrollbar component once those land.

#![allow(dead_code)]

use rinch::prelude::*;

use crate::state::AppState;

/// Project a bar position (possibly fractional) to a pixel offset
/// within the section lane's viewport.
///
/// Result is signed: a bar to the left of the viewport returns a
/// negative px; a bar to the right returns a px beyond
/// [`visible_bar_width`]. Callers that want to clip to the
/// viewport (e.g. for culling) check the returned value against
/// `0..lane_width` themselves.
pub fn bar_to_px(bar: f32) -> f32 {
    let app = use_store::<AppState>();
    (bar - app.scroll_bars.get()) * app.pixels_per_bar.get()
}

/// Inverse of [`bar_to_px`] — project a pixel offset within the
/// lane viewport back to a (possibly fractional) bar position.
///
/// Used by the wheel-scroll + cursor-anchored zoom handlers to map
/// "the bar under the cursor" to a coordinate the scroll state
/// can be adjusted against.
pub fn px_to_bar(px: f32) -> f32 {
    let app = use_store::<AppState>();
    px / app.pixels_per_bar.get().max(1.0) + app.scroll_bars.get()
}

/// How many bars fit in a lane viewport of `lane_width_px` pixels
/// at the current zoom. Inverse of `pixels_per_bar`; used by the
/// visible-range computations + the wheel-scroll clamps.
pub fn visible_bar_width(lane_width_px: f32) -> f32 {
    let app = use_store::<AppState>();
    lane_width_px / app.pixels_per_bar.get().max(1.0)
}
