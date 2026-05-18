//! Test fixtures live on the public `rawdaw_model::fixtures` module
//! (Phase E1 of the engine-wiring milestone). This file is a re-export
//! shim so the existing test imports (`mod common; common::build_tiny_project()`)
//! keep working without changes.

pub use rawdaw_model::fixtures::build_tiny_project;
