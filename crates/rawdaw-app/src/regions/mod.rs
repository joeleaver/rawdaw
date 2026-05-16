//! UI regions: TopBar, Library, Arrangement, Inspector, BottomStrip.
//!
//! Each region maps 1:1 to a JSX file in
//! `docs/design/mockups/round-1/components/`. The regions accept a
//! `Selection` signal so they can react to selection changes (linked
//! highlight in the arrangement, populated inspector, library row tint).

pub mod arrangement;
pub mod bottom_strip;
pub mod inspector;
pub mod library;
pub mod topbar;

pub use arrangement::Arrangement;
pub use bottom_strip::BottomStrip;
pub use inspector::Inspector;
pub use library::Library;
pub use topbar::TopBar;
