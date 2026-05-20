//! rawdaw — desktop entry point.
//!
//! Round-1 shipped the main window mirroring
//! `docs/design/mockups/round-1/main-window.html`. Round 2 adds the
//! section editor (`docs/design/mockups/round-2/`); both surfaces share
//! the top bar and toggle below it via `state::EditorMode`. The live
//! project + overlay come from [`initial_project::build_initial`] at
//! boot and live on [`state::AppState`] for the UI to read reactively.
//!
//! Architecture:
//!
//! - `theme` — design tokens (dark surfaces, identity palette, sizes).
//! - `initial_project` — one-shot factory that builds the
//!   `(Project, ProjectOverlay)` pair `AppState` boots into. Replaced
//!   the old `fixture` adapter at C1c of the composition-writability
//!   milestone.
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
mod chord_loop_actions;
mod chord_shorthand;
mod initial_project;
mod midi_input;
mod overlay;
mod parts;
mod pattern_actions;
mod presets;
mod project_display;
mod project_io;
mod regions;
mod section_editor;
mod state;
mod theme;

fn main() {
    rinch::run("rawdaw", 1600, 900, app::main_window);
}
