//! Top-level app state shared across panes via a Rinch store.
//!
//! Round 2 adds the section editor as a second top-level surface. The app
//! now toggles between *arrangement mode* (the round-1 main window with
//! library / arrangement / inspector / bottom strip) and *section editor
//! mode* (the round-2 surface that replaces the lower content area below
//! the top bar).
//!
//! `EditorMode` is the discriminator. It lives inside `AppState`, which is
//! installed once at `main_window` start-up via `rinch::create_store` and
//! read from any nested component with `rinch::use_store::<AppState>()`.
//!
//! ## Naming: `section_key` vs `section_idx`
//!
//! The round-2 port plan called for `section_idx: usize`. The existing
//! fixture API is keyed by section name (`fixture::section_by_key`), and
//! every other piece of UI code identifies sections by their `name` field
//! rather than their index in `fixture::round1().sections`. We use
//! `section_key: String` here for consistency — the role in `EditorMode`
//! is identical (uniquely identifies which section is being edited).

use rinch::prelude::*;

/// Which top-level surface the app is rendering below the top bar.
#[derive(Clone, PartialEq, Debug, Default)]
pub enum EditorMode {
    /// Round-1 arrangement view: library / arrangement / inspector / bottom
    /// strip. The default surface when the app launches.
    #[default]
    Arrangement,
    /// Round-2 section editor for a specific section + variant. Replaces
    /// the arrangement view's middle row; the top bar stays.
    SectionEditor {
        /// Section identifier — `Section.name` from the fixture. Looked up
        /// via `fixture::section_by_key`.
        section_key: String,
        /// Currently active variant tab (`"base"`, `"stripped"`, …).
        variant: String,
    },
}

/// Shared app state. Holds a single `Signal<EditorMode>` so navigation
/// changes propagate reactively to every component that reads it.
///
/// `Copy` is intentional and load-bearing: `Signal` is `Copy` (per the
/// rinch framework contract — never `.clone()` a Signal), and this
/// struct only wraps Signals, so the whole struct is cheaply copyable
/// into closures and props.
#[derive(Clone, Copy)]
pub struct AppState {
    pub editor_mode: Signal<EditorMode>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            editor_mode: Signal::new(EditorMode::Arrangement),
        }
    }

    /// Switch to the section editor for `section_key` at `variant`.
    pub fn open_section_editor(&self, section_key: impl Into<String>, variant: impl Into<String>) {
        self.editor_mode.set(EditorMode::SectionEditor {
            section_key: section_key.into(),
            variant: variant.into(),
        });
    }

    /// Return to the arrangement.
    pub fn close_section_editor(&self) {
        self.editor_mode.set(EditorMode::Arrangement);
    }

    /// Switch which variant the section editor is displaying. No-op if
    /// the app isn't currently in section-editor mode (defensive — the
    /// variant tab strip only renders inside the section editor, so this
    /// branch shouldn't fire in practice).
    pub fn set_variant(&self, variant: impl Into<String>) {
        if let EditorMode::SectionEditor { section_key, .. } = self.editor_mode.get() {
            self.editor_mode.set(EditorMode::SectionEditor {
                section_key,
                variant: variant.into(),
            });
        }
    }
}
