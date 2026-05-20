//! rawdaw — desktop entry point.
//!
//! Round-1 shipped the main window mirroring
//! `docs/design/mockups/round-1/main-window.html`. Round 2 adds the
//! section editor (`docs/design/mockups/round-2/`); both surfaces share
//! the top bar and toggle below it via `state::EditorMode`. The engine
//! is not yet wired in; the view is driven from a static Rust fixture
//! (`fixture::round1`).
//!
//! Architecture:
//!
//! - `theme` — design tokens (dark surfaces, identity palette, sizes).
//! - `fixture` — UI view of the round-1 project, built via an adapter
//!   over `rawdaw_model::fixtures::build_round1_project()`. Being
//!   dismantled in the composition-writability milestone (C1) —
//!   structural data moves to `AppState.project`, decorations to
//!   `crate::overlay`.
//! - `overlay` — UI-only decorations layered on `rawdaw_model::Project`
//!   (colors, library meta strings, per-cell realization values).
//!   `ProjectOverlay` is the parallel store to the model project.
//! - `state` — `EditorMode` + `AppState` shared via a Rinch store.
//! - `audio` — `AudioResources`: engine instantiation + per-track sine
//!   graph + realize→translate→push, also shared via a Rinch store.
//! - `parts` — shared visual primitives (`rgba`, `Icon`).
//! - `regions` — round-1 panes: `TopBar`, `Library`, `Arrangement`,
//!   `Inspector`, `BottomStrip`.
//! - `section_editor` — round-2 surface; replaces the regions row when
//!   `EditorMode::SectionEditor` is active.
//! - `app` — the `MainWindow` composition + mode switch.

mod app;
mod audio;
mod chord_display;
mod fixture;
mod initial_project;
mod midi_input;
mod overlay;
mod parts;
mod presets;
mod project_display;
mod regions;
mod section_editor;
mod state;
mod theme;

fn main() {
    rinch::run("rawdaw", 1600, 900, app::main_window);
}
