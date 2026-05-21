//! Drum step-grid editor — P3 of `docs/pattern-editor-plan.md`.
//!
//! Mounted by `regions/pattern_editor/mod.rs`'s body dispatch when
//! the selected pattern's body is `PatternBody::Drum`. Lays out:
//!
//! - `VoiceManager` header — current voice chips + add-voice dropdown.
//! - `StepGrid` — the rows × cols step surface.
//! - `Inspector` — per-event editor (right pane).
//!
//! File layout (per P2 design decision 10 — split eagerly):
//! - `mod.rs` (this file) — `DrumPatternEditor` shell.
//! - `helpers.rs` — pure helpers (step ↔ time, default-event factory,
//!   voice display names). Unit-tested.
//! - `cell.rs` + `step_grid.rs` — grid surface and its cell component.
//! - `voice_manager.rs` — voice chips + add-voice dropdown header.
//! - `inspector/` — per-event inspector (timing / dynamics /
//!   humanization / voice selector), parallels `pitched/inspector/`
//!   minus the PitchSpec sub-editors.

mod cell;
pub mod helpers;
mod inspector;
mod step_grid;
mod voice_manager;

use rinch::prelude::*;

use rawdaw_model::id::PatternId;

use self::inspector::Inspector;
use self::step_grid::StepGrid;
use self::voice_manager::VoiceManager;

/// Drum-pattern editor. The step-grid takes the bulk of horizontal
/// real estate; the per-event inspector sits on the right; the
/// voice-management header sits above the grid.
#[component]
pub fn DrumPatternEditor(id: PatternId) -> NodeHandle {
    rsx! {
        div { style: "flex: 1; display: flex; min-height: 0;",
            div {
                style: "flex: 1; display: flex; flex-direction: column; \
                        min-width: 0; gap: 6px; padding: 10px;",
                VoiceManager { id: id }
                StepGrid { id: id }
            }
            Inspector { id: id }
        }
    }
}
