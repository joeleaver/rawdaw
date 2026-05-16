//! rawdaw — desktop entry point.
//!
//! Round-1 status: the app launches a window that mirrors
//! `docs/design/mockups/round-1/main-window.html`. The engine is not yet
//! wired in; the view is driven from a static Rust fixture
//! (`fixture::round1`) that mirrors the JS round-1 fixture.
//!
//! Architecture:
//!
//! - `theme` — design tokens (dark surfaces, identity palette, sizes).
//! - `fixture` — Rust mirror of the round-1 JS fixture; will be
//!   replaced with a real `rawdaw_model::Project` once the engine is
//!   wired through.
//! - `parts` — shared visual primitives (`rgba`, `Icon`).
//! - `regions` — `TopBar`, `Library`, `Arrangement`, `Inspector`,
//!   `BottomStrip`.
//! - `app` — the `MainWindow` composition + selection state.

mod app;
mod fixture;
mod parts;
mod regions;
mod theme;

fn main() {
    rinch::run("rawdaw", 1600, 900, app::main_window);
}
