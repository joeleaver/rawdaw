//! Pixel-based timeline primitive used by the arrangement view.
//!
//! The round-1 arrangement was percent-based: every block's `left`
//! and `width` were expressed as `{bar / total_bars * 100}%`, which
//! collapsed the entire arrangement into the lane's pixel width
//! regardless of length. Worked for the demo project; broke down
//! for anything longer than a screen.
//!
//! This module replaces the percent math with pixel-based
//! projection driven by two AppState signals:
//!
//! - [`AppState::pixels_per_bar`]: the zoom level (px per bar).
//! - [`AppState::scroll_bars`]: the leftmost visible bar (allows
//!   fractional values for smooth scrolling).
//!
//! Every consumer reads these reactively via [`bar_to_px`] and
//! [`px_to_bar`]. Change either signal and the lane re-lays out
//! surgically — no full re-render, no recompute of the section
//! data structures.
//!
//! Built directly on rinch's [`bounds_signal`] primitive (rinch
//! issue [#30](https://github.com/joeleaver/rinch/issues/30)
//! step 1). A future rinch [#31] follow-up may ship a
//! higher-level `Timeline` component; this module lives here
//! until that lands and can migrate then.
//!
//! [`bounds_signal`]: rinch::core::dom::NodeHandle::bounds_signal
//! [`AppState::pixels_per_bar`]: crate::state::AppState::pixels_per_bar
//! [`AppState::scroll_bars`]: crate::state::AppState::scroll_bars

mod projection;

#[allow(unused_imports)]
pub use projection::{bar_to_px, px_to_bar, visible_bar_width};
