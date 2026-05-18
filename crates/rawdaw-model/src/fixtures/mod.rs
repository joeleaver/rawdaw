//! Public fixture constructors.
//!
//! Builds well-formed `Project`s for tests and runtime. Gated behind
//! the `fixtures` Cargo feature so release builds of the model crate
//! that don't need them stay minimal.
//!
//! ## Contents
//!
//! - `tiny_project::build_tiny_project()` — 4-bar verse with three
//!   tracks (bass / lead / drums) and a `stripped` variant. The
//!   `rawdaw-model` smoke + roundtrip tests use this.
//!
//! Add `build_round1_project()` here when Phase E1 of the engine-
//! wiring milestone lands — it'll be the canonical fixture for the
//! `rawdaw-app` runtime.

mod round1;
mod tiny_project;

pub use round1::{
    build_round1_project, Round1ChordLoopIds, Round1Keys, Round1PatternIds, Round1SectionIds,
    Round1TrackIds,
};
pub use tiny_project::build_tiny_project;
