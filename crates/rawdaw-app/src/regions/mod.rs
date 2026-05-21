//! UI regions: TopBar, Library, Arrangement, Inspector, BottomStrip.
//!
//! Each region maps 1:1 to a JSX file in
//! `docs/design/mockups/round-1/components/`. The regions accept a
//! `Selection` signal so they can react to selection changes (linked
//! highlight in the arrangement, populated inspector, library row tint).

pub mod arrangement;
pub mod bottom_strip;
pub mod chord_loop_editor;
pub mod inspector;
pub mod library;
pub mod pattern_editor;
pub mod topbar;
pub mod tracks_pane;

pub use arrangement::Arrangement;
pub use bottom_strip::BottomStrip;
pub use chord_loop_editor::ChordLoopEditor;
pub use inspector::Inspector;
pub use library::Library;
pub use pattern_editor::PatternEditor;
pub use topbar::TopBar;
pub use tracks_pane::TracksPane;
