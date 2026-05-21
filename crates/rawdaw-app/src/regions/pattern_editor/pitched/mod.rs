//! Pitched-pattern editor — the piano-roll surface.
//!
//! Mounted by `regions/pattern_editor/mod.rs`'s body dispatch when
//! the selected pattern's body is `PatternBody::Pitched`. Splits
//! into:
//!
//! - `helpers.rs` — pure helpers (grid snapping, sorted insert,
//!   delete-by-id, default-event factory). Unit-tested.
//! - `piano_roll.rs` — the grid surface itself (rows × cols).
//!   Note blocks are inlined per row (no separate component) to
//!   keep `rsx!`'s `for`-loop keying working with branching cell
//!   kinds.
//! - `realized_strip.rs` — strip of realized pitches under the
//!   chord-context preview chord.
//! - `inspector/` — per-note inspector split by field group.

pub mod helpers;
pub mod inspector;
pub mod piano_roll;
pub mod realized_strip;

use rinch::prelude::*;

use rawdaw_model::id::PatternId;

use crate::theme;

use piano_roll::PianoRoll;
use realized_strip::RealizedStrip;
use inspector::Inspector;

/// Pitched-pattern editor. The piano-roll grid takes the bulk of
/// horizontal real estate; the per-note inspector sits on the right;
/// the realized strip sits beneath the grid.
#[component]
pub fn PitchedPatternEditor(id: PatternId) -> NodeHandle {
    rsx! {
        div { style: "flex: 1; display: flex; min-height: 0;",
            div {
                style: "flex: 1; display: flex; flex-direction: column; \
                        min-width: 0; gap: 6px; padding: 10px;",
                PianoRoll { id: id }
                RealizedStrip { id: id }
            }
            Inspector { id: id }
        }
    }
}

/// Default pixel height per pitch row.
pub const ROW_HEIGHT_PX: u32 = 22;

/// Fallback color for note blocks when the pattern has no entry in
/// `ProjectOverlay.pattern_color`. Hex `#RRGGBB` because the
/// `parts::rgba(hex, alpha)` helper used for note backgrounds parses
/// hex — passing an rgba string here would silently fall back to
/// black, which (against the BG1 panel) renders as invisible notes.
pub const FALLBACK_NOTE_COLOR: &str = theme::ACCENT;
