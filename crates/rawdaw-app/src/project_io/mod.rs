//! Project save / load wiring for the desktop app.
//!
//! Composition-writability C3. The model crate already ships
//! [`rawdaw_model::project::Project::save`] (→ RON + version header)
//! and [`rawdaw_model::project::Project::load`] (→ version-checked
//! deserialize with a typed [`rawdaw_model::project::LoadError`]).
//! This module wraps both with the **overlay half** (UI-only
//! decorations from [`crate::overlay::ProjectOverlay`]) so the
//! TopBar's "Save" button persists the full editing state, not just
//! the model.
//!
//! ```text
//!   .rawd file
//!   ├── bundle_version: u32        ← independent of project schema
//!   ├── project: Project           ← serde via Project::save inner
//!   └── overlay: ProjectOverlay    ← UI-only decorations (colors,
//!                                     meta strings, per-cell)
//! ```
//!
//! `bundle_version` is independent of [`rawdaw_model::project::SCHEMA_VERSION`]
//! — the overlay format can grow (Tier-1 patterns will accrete new
//! per-cell fields) without touching the model schema, and vice
//! versa. v1 of the bundle is the only format today.
//!
//! ## Modules
//!
//! - [`bundle`] — the on-disk struct, version constants, error types.
//! - [`save`] — write a [`SavedBundle`] to a path.
//! - [`load`] — read + version-check a [`SavedBundle`] from a path.
//! - [`dialog`] — cross-thread native file picker (rfd) wrapper.
//!
//! ## Load semantics
//!
//! Loading is a **whole-project replacement**, not a structural edit
//! — but it goes through the same audio-side re-arm path as
//! [`crate::audio::AudioResources::apply_project_edit`] so the engine
//! drains its song queue and re-realizes the new project without
//! resetting the playhead. See [`load::apply_loaded_bundle`] for the
//! integration point.
//!
//! ## Error surfacing
//!
//! Every error returned from this module carries a human-readable
//! `String`. The TopBar's File-menu handlers print errors to stderr
//! today; a follow-up dialog UI lands when the round-1 toast / alert
//! primitives ship.

pub mod bundle;
pub mod dialog;
pub mod load;
pub mod save;

pub use bundle::SavedBundle;
pub use load::{apply_loaded_bundle, load_from_path};
pub use save::save_to_path;
