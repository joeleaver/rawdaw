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
/// changes propagate reactively to every component that reads it, plus
/// the currently-selected arrangement-block index and the currently-
/// selected project-track index.
///
/// `Copy` is intentional and load-bearing: `Signal` is `Copy` (per the
/// rinch framework contract — never `.clone()` a Signal), and this
/// struct only wraps Signals, so the whole struct is cheaply copyable
/// into closures and props.
///
/// ## Selection axes
///
/// `selected_idx` (arrangement-block) and `selected_track` (project-
/// track) are two distinct selection axes that the inspector branches
/// on. They're mutually exclusive at the UI level — choosing one
/// clears the other — so the inspector always has a single thing to
/// render. `set_selected_idx` and `select_track` enforce this so
/// callers don't have to coordinate clears at each click-handler site.
#[derive(Clone, Copy)]
pub struct AppState {
    pub editor_mode: Signal<EditorMode>,
    /// Index into `fixture::round1().arrangement` of the currently
    /// selected SectionRef. Round-1 boot value is `Some(1)`
    /// (verse@bar5) so the inspector lands populated; users can change
    /// it by clicking a SectionBlock in the arrangement.
    pub selected_idx: Signal<Option<usize>>,
    /// Index into the model `Project.tracks` of the currently
    /// selected project track. `None` by default; clicking a row in
    /// the arrangement's TracksPane sets it (and clears
    /// `selected_idx`). Drives the synth-editor branch of the
    /// Inspector (U4+).
    pub selected_track: Signal<Option<usize>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            editor_mode: Signal::new(EditorMode::Arrangement),
            selected_idx: Signal::new(Some(1usize)),
            selected_track: Signal::new(None),
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

    /// Set the currently-selected SectionRef index. `None` clears the
    /// selection (inspector goes to its empty state). Selecting a
    /// section-block clears any track selection so the inspector
    /// branches deterministically on a single axis.
    pub fn set_selected_idx(&self, idx: Option<usize>) {
        if idx.is_some() {
            self.selected_track.set(None);
        }
        self.selected_idx.set(idx);
    }

    /// Set the currently-selected project-track index. `None` clears
    /// the selection. Selecting a track clears any section-block
    /// selection so the inspector branches deterministically on a
    /// single axis (see [`Self::set_selected_idx`] for the mirror).
    pub fn select_track(&self, idx: Option<usize>) {
        if idx.is_some() {
            self.selected_idx.set(None);
        }
        self.selected_track.set(idx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_track_clears_section_selection() {
        let app = AppState::new();
        app.selected_idx.set(Some(2));
        assert_eq!(app.selected_idx.get(), Some(2));

        app.select_track(Some(0));
        assert_eq!(app.selected_track.get(), Some(0));
        assert_eq!(app.selected_idx.get(), None);
    }

    #[test]
    fn set_selected_idx_clears_track_selection() {
        let app = AppState::new();
        app.select_track(Some(1));
        assert_eq!(app.selected_track.get(), Some(1));

        app.set_selected_idx(Some(3));
        assert_eq!(app.selected_idx.get(), Some(3));
        assert_eq!(app.selected_track.get(), None);
    }

    #[test]
    fn clearing_selection_does_not_touch_other_axis() {
        // Setting either axis to `None` is a pure clear — it must
        // never disturb the other axis. The mutex only fires on
        // `Some(_)` selections.
        let app = AppState::new();
        app.select_track(Some(2));

        app.set_selected_idx(None);
        assert_eq!(app.selected_track.get(), Some(2));

        app.select_track(None);
        assert_eq!(app.selected_idx.get(), None);
    }
}
