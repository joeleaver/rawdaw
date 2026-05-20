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
//! The round-2 port plan called for `section_idx: usize`. Round-1 / C1c
//! migrated UI lookups to `Project.sections.values().find(|s| s.name ==
//! key)`, so the discriminator stays a name string for now. A follow-up
//! C1 slice flips this to the typed `SectionId` (design decision 4 in
//! the composition-writability plan) once the section_editor and
//! inspector both consume the new type end-to-end.

use std::path::PathBuf;
use std::rc::Rc;

use rinch::prelude::*;

use rawdaw_model::id::{ChordLoopId, PatternId};
use rawdaw_model::pitch::PitchClass;
use rawdaw_model::project::Project;
use rawdaw_model::scale::Scale;

use crate::overlay::ProjectOverlay;

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
        /// Section identifier — `Section.name` from the live project.
        /// Resolved at consumer sites via
        /// `project.sections.values().find(|s| s.name == key)`. Will
        /// become the typed `SectionId` in a follow-up C1 slice.
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
/// `selected_idx` (arrangement-block), `selected_track` (project-
/// track), `selected_chord_loop` (library chord-loop), and
/// `selected_pattern` (library pattern) are four distinct selection
/// axes that the inspector / editor surfaces branch on. They're
/// mutually exclusive at the UI level — choosing one clears the
/// others — so the inspector always has a single thing to render.
/// `set_selected_idx`, `select_track`, `select_chord_loop`, and
/// `select_pattern` enforce this so callers don't have to
/// coordinate clears at each click-handler site.
#[derive(Clone, Copy)]
pub struct AppState {
    pub editor_mode: Signal<EditorMode>,
    /// Index into the live `project.arrangement.sections` Vec of the
    /// currently selected SectionRef. Round-1 boot value is `Some(1)`
    /// (verse@bar5) so the inspector lands populated; users change it
    /// by clicking a SectionBlock in the arrangement.
    pub selected_idx: Signal<Option<usize>>,
    /// Index into the model `Project.tracks` of the currently
    /// selected project track. `None` by default; clicking a row in
    /// the arrangement's TracksPane sets it (and clears
    /// `selected_idx`). Drives the synth-editor branch of the
    /// Inspector (U4+).
    pub selected_track: Signal<Option<usize>>,
    /// Currently selected chord loop in the library, by id. `None` by
    /// default; clicking a chord-loop row in the Library panel sets it
    /// (and clears `selected_idx` + `selected_track` + `selected_pattern`
    /// per the selection mutex). Drives the chord-loop editor surface
    /// introduced in CL2 of `docs/chord-loop-editing-plan.md`.
    pub selected_chord_loop: Signal<Option<ChordLoopId>>,
    /// Currently selected pattern in the library, by id. `None` by
    /// default; clicking a pattern row in the Library panel sets it
    /// (and clears the three other selection axes per the mutex).
    /// Drives the pattern editor surface introduced in P2 of
    /// `docs/pattern-editor-plan.md`. P1 only uses this for the
    /// library row's visual selection highlight.
    pub selected_pattern: Signal<Option<PatternId>>,
    /// Index into the focused chord loop's event vec of the
    /// currently-focused chord event. Drives the chord-loop editor's
    /// inspector pane (CL2). Lives on AppState rather than as a
    /// component-local Signal so the timeline + inspector + future
    /// CL4 realized strip share a single source of truth and
    /// don't need to plumb a `Signal<Option<usize>>` through props
    /// (rinch's `#[component]` requires every prop's type to
    /// implement `Default`, which `Signal<Option<usize>>` does not).
    pub focused_chord_event_idx: Signal<Option<usize>>,
    /// MIDI input routing target — *sticky* version of
    /// `selected_track`. Updates whenever the user picks a track
    /// (`select_track(Some(_))`); does NOT clear when a section
    /// block is selected or `select_track(None)` is called. So
    /// MIDI continues to play through whatever synth you were last
    /// editing even after you click away to inspect a section.
    ///
    /// K3 contract — see `docs/midi-input-plan.md`. The TracksPane
    /// renders a "♪" badge on whichever row matches this value;
    /// an Effect in `MainWindow` propagates changes to
    /// [`AudioResources::set_midi_target_track`](crate::audio::AudioResources::set_midi_target_track).
    pub midi_target_track: Signal<Option<usize>>,

    /// The live, mutable project. C1 of the composition-writability
    /// milestone introduces this signal as the replacement for the old
    /// `fixture::round1()` adapter; every UI region reads through it
    /// now. C2's edit pump writes into it via `Signal::set` with a
    /// freshly cloned `Rc<Project>` per structural edit.
    ///
    /// Wrapped in `Rc` so reads are O(refcount bump) regardless of
    /// project size. Mutations replace the inner `Rc` wholesale via
    /// `Signal::set` after cloning-on-write the inner `Project`.
    /// Granular per-field signals can come later if profiling shows
    /// the whole-project re-read is too coarse (see C1 design
    /// decision 1 in `docs/composition-writability-plan.md`).
    ///
    /// Seeded with an empty default in `AppState::new()`; the real
    /// round-1 demo project is installed by `app::main_window` right
    /// after `create_store(AppState::new())` runs, before any region
    /// renders.
    pub project: Signal<Rc<Project>>,

    /// UI-only decorations layered on top of [`Self::project`]: colors,
    /// library meta strings, per-cell realization values. Parallel
    /// signal — same lifecycle as `project`. See [`ProjectOverlay`]
    /// for the structure.
    pub overlay: Signal<Rc<ProjectOverlay>>,

    /// Filesystem path the live project was last saved to or loaded
    /// from. `None` for a fresh boot (no file yet) and after `New`.
    /// `Save` writes back to this path; `Save As` prompts a dialog
    /// and updates the signal on success.
    ///
    /// C3 of the composition-writability milestone: the TopBar's
    /// `Project ▾` menu reads this to decide whether `Save` is a
    /// no-prompt write or should fall through to `Save As`.
    pub current_path: Signal<Option<PathBuf>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            editor_mode: Signal::new(EditorMode::Arrangement),
            selected_idx: Signal::new(Some(1usize)),
            selected_track: Signal::new(None),
            selected_chord_loop: Signal::new(None),
            selected_pattern: Signal::new(None),
            focused_chord_event_idx: Signal::new(None),
            // The MIDI target seed is filled in by an Effect at boot
            // that reads the first Pitched track's index from
            // `AudioResources`. Starting as `None` keeps the contract
            // pure (no special-case for "before-init"); the Effect
            // overwrites on first run.
            midi_target_track: Signal::new(None),
            // Project + overlay are seeded with an empty default here;
            // `app::main_window` overwrites them with the real
            // `initial_project::build_initial()` payload right after
            // `create_store(AppState::new())` runs, before any
            // component renders. Tests that don't care about project
            // content (selection-mutex tests below) can construct
            // `AppState::new()` and ignore these.
            project: Signal::new(Rc::new(Project::new(Scale::major(PitchClass::C)))),
            overlay: Signal::new(Rc::new(ProjectOverlay::empty())),
            current_path: Signal::new(None),
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
    /// section-block clears any track / chord-loop selection so the
    /// inspector branches deterministically on a single axis.
    pub fn set_selected_idx(&self, idx: Option<usize>) {
        if idx.is_some() {
            self.selected_track.set(None);
            self.selected_chord_loop.set(None);
            self.selected_pattern.set(None);
        }
        self.selected_idx.set(idx);
    }

    /// Set the currently-selected project-track index. `None` clears
    /// the selection. Selecting a track clears any section-block /
    /// chord-loop selection so the inspector branches deterministically
    /// on a single axis (see [`Self::set_selected_idx`] for the mirror).
    ///
    /// Also updates `midi_target_track` when `idx` is `Some(_)` so
    /// MIDI input follows the track the user is actively editing.
    /// Crucially, **clearing the visual track selection
    /// (`select_track(None)`) does NOT clear `midi_target_track`**
    /// — MIDI continues to play through the last-selected track
    /// even after the user clicks away to look at a section block.
    /// This is the K3 "sticky routing" contract.
    pub fn select_track(&self, idx: Option<usize>) {
        if idx.is_some() {
            self.selected_idx.set(None);
            self.selected_chord_loop.set(None);
            self.selected_pattern.set(None);
        }
        self.selected_track.set(idx);
        if let Some(track_idx) = idx {
            self.midi_target_track.set(Some(track_idx));
        }
    }

    /// Set the currently-selected library chord-loop. `None` clears
    /// the selection. Selecting a chord loop clears any section /
    /// track selection so the inspector branches deterministically
    /// on a single axis (see [`Self::set_selected_idx`] +
    /// [`Self::select_track`] for the mirrors). MIDI routing
    /// (`midi_target_track`) is untouched — chord-loop selection is
    /// not a synth-target switch.
    ///
    /// Also resets [`EditorMode`] to `Arrangement` so the chord-
    /// loop editor mounts inside the arrangement surface regardless
    /// of whether the user was previously in the section editor —
    /// the chord-loop editor is the new "center stage" content
    /// (CL0 design decision 1 of `chord-loop-editing-plan.md`).
    pub fn select_chord_loop(&self, id: Option<ChordLoopId>) {
        if id.is_some() {
            self.selected_idx.set(None);
            self.selected_track.set(None);
            self.selected_pattern.set(None);
            self.editor_mode.set(EditorMode::Arrangement);
        }
        // Switching which loop is open invalidates whatever event
        // index was focused — clear it so the inspector lands in
        // its empty state rather than pointing at a stale event
        // in a different loop.
        self.focused_chord_event_idx.set(None);
        self.selected_chord_loop.set(id);
    }

    /// Set the currently-selected library pattern. `None` clears the
    /// selection. Selecting a pattern clears the other three
    /// selection axes so the inspector branches deterministically
    /// (mirrors [`Self::select_chord_loop`]). MIDI routing
    /// (`midi_target_track`) is untouched — pattern selection is
    /// not a synth-target switch.
    ///
    /// Also resets [`EditorMode`] to `Arrangement` so the pattern
    /// editor (P2+ of `docs/pattern-editor-plan.md`) mounts inside
    /// the arrangement surface regardless of whether the user was
    /// previously in the section editor — same "center stage"
    /// pattern the chord-loop editor uses.
    pub fn select_pattern(&self, id: Option<PatternId>) {
        if id.is_some() {
            self.selected_idx.set(None);
            self.selected_track.set(None);
            self.selected_chord_loop.set(None);
            self.editor_mode.set(EditorMode::Arrangement);
        }
        self.selected_pattern.set(id);
    }

    /// Apply a structural edit to the live project and mirror the
    /// result through both the audio engine and the UI signal.
    ///
    /// Composition-writability C2's mutation entry point. The
    /// closure receives a mutable reference to a clone of the
    /// current project; on return, the mutated value is installed
    /// as the new live snapshot via
    /// [`AudioResources::apply_project_edit`] (which re-realizes
    /// and re-arms the engine), and the AppState `project` signal
    /// is `set` to the same `Rc` so every UI region subscribed to
    /// it re-renders.
    ///
    /// Single mutation surface for every Tier-1 editing UI. UI
    /// handlers call this — never `Signal::set` on `project`
    /// directly — so the audio side never drifts from the UI's
    /// view of the project.
    ///
    /// Returns the engine's error string verbatim on failure. The
    /// signal is **not** updated on error, so a failed edit leaves
    /// the UI showing the pre-edit project. See
    /// [`AudioResources::apply_project_edit`] for the audio-side
    /// failure modes.
    // Release builds today only reach this through the cfg-gated
    // +1 BPM debug button — C3 (project load) / C4 (real tempo/key/
    // name controls) add release call sites and the allow goes away.
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    pub fn apply_project_edit<F>(&self, f: F) -> Result<(), String>
    where
        F: FnOnce(&mut Project),
    {
        let audio = use_store::<crate::audio::AudioResources>();
        let new_rc = audio.apply_project_edit(f)?;
        self.project.set(new_rc);
        Ok(())
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

    #[test]
    fn select_track_sets_midi_target_track() {
        // K3: selecting a track also updates the MIDI routing
        // target so live MIDI plays through that synth.
        let app = AppState::new();
        app.select_track(Some(2));
        assert_eq!(app.midi_target_track.get(), Some(2));

        app.select_track(Some(3));
        assert_eq!(app.midi_target_track.get(), Some(3));
    }

    #[test]
    fn midi_target_track_is_sticky_across_section_selection() {
        // K3 sticky-routing contract: clicking a section block
        // clears `selected_track` but PRESERVES `midi_target_track`.
        // The user can audition a synth, click away to inspect a
        // section, and still play the audited synth.
        let app = AppState::new();
        app.select_track(Some(2));
        assert_eq!(app.midi_target_track.get(), Some(2));

        app.set_selected_idx(Some(5));
        assert_eq!(
            app.midi_target_track.get(),
            Some(2),
            "midi target must survive section-block selection",
        );
        assert_eq!(app.selected_track.get(), None);
    }

    #[test]
    fn midi_target_track_is_sticky_across_clear() {
        // Calling select_track(None) explicitly also preserves
        // midi_target_track — the K3 contract.
        let app = AppState::new();
        app.select_track(Some(1));
        app.select_track(None);
        assert_eq!(
            app.midi_target_track.get(),
            Some(1),
            "midi target must survive an explicit track clear",
        );
        assert_eq!(app.selected_track.get(), None);
    }

    #[test]
    fn select_chord_loop_clears_section_and_track_selection() {
        // CL1 selection mutex: picking a chord loop clears both
        // other axes so the inspector branches deterministically.
        let app = AppState::new();
        app.selected_idx.set(Some(2));
        app.selected_track.set(Some(1));

        let id = ChordLoopId::new(7);
        app.select_chord_loop(Some(id));

        assert_eq!(app.selected_chord_loop.get(), Some(id));
        assert_eq!(app.selected_idx.get(), None);
        assert_eq!(app.selected_track.get(), None);
    }

    #[test]
    fn other_axes_clear_chord_loop_selection() {
        // Symmetric: setting section or track to Some(_) clears the
        // chord-loop selection, completing the three-way mutex.
        let app = AppState::new();
        app.select_chord_loop(Some(ChordLoopId::new(3)));

        app.set_selected_idx(Some(0));
        assert_eq!(app.selected_chord_loop.get(), None);

        app.select_chord_loop(Some(ChordLoopId::new(3)));
        app.select_track(Some(2));
        assert_eq!(app.selected_chord_loop.get(), None);
    }

    #[test]
    fn select_chord_loop_does_not_touch_midi_target() {
        // Chord-loop selection isn't a synth-target switch, so the
        // K3 sticky MIDI target is untouched.
        let app = AppState::new();
        app.select_track(Some(2));
        assert_eq!(app.midi_target_track.get(), Some(2));

        app.select_chord_loop(Some(ChordLoopId::new(1)));
        assert_eq!(
            app.midi_target_track.get(),
            Some(2),
            "chord-loop selection must not redirect MIDI input",
        );
    }

    #[test]
    fn select_chord_loop_none_does_not_touch_other_axes() {
        // Clearing chord-loop selection is a pure clear; it must not
        // disturb the section or track selection.
        let app = AppState::new();
        app.set_selected_idx(Some(4));

        app.select_chord_loop(None);
        assert_eq!(app.selected_idx.get(), Some(4));
        assert_eq!(app.selected_chord_loop.get(), None);
    }

    #[test]
    fn select_pattern_clears_other_three_axes() {
        // P1 selection mutex: picking a pattern clears section,
        // track, and chord-loop selection so the inspector branches
        // deterministically on a single axis.
        let app = AppState::new();
        app.selected_idx.set(Some(2));
        app.selected_track.set(Some(1));
        app.selected_chord_loop.set(Some(ChordLoopId::new(5)));

        let id = PatternId::new(11);
        app.select_pattern(Some(id));

        assert_eq!(app.selected_pattern.get(), Some(id));
        assert_eq!(app.selected_idx.get(), None);
        assert_eq!(app.selected_track.get(), None);
        assert_eq!(app.selected_chord_loop.get(), None);
    }

    #[test]
    fn other_axes_clear_pattern_selection() {
        // Symmetric: setting any other axis to Some(_) clears the
        // pattern selection, completing the four-way mutex.
        let app = AppState::new();
        let pid = PatternId::new(7);

        app.select_pattern(Some(pid));
        app.set_selected_idx(Some(0));
        assert_eq!(app.selected_pattern.get(), None);

        app.select_pattern(Some(pid));
        app.select_track(Some(1));
        assert_eq!(app.selected_pattern.get(), None);

        app.select_pattern(Some(pid));
        app.select_chord_loop(Some(ChordLoopId::new(3)));
        assert_eq!(app.selected_pattern.get(), None);
    }

    #[test]
    fn select_pattern_does_not_touch_midi_target() {
        // Pattern selection isn't a synth-target switch, so the K3
        // sticky MIDI target is untouched (mirrors chord-loop's
        // behavior).
        let app = AppState::new();
        app.select_track(Some(2));
        assert_eq!(app.midi_target_track.get(), Some(2));

        app.select_pattern(Some(PatternId::new(4)));
        assert_eq!(
            app.midi_target_track.get(),
            Some(2),
            "pattern selection must not redirect MIDI input",
        );
    }

    #[test]
    fn select_pattern_none_does_not_touch_other_axes() {
        // Clearing pattern selection is a pure clear.
        let app = AppState::new();
        app.set_selected_idx(Some(3));

        app.select_pattern(None);
        assert_eq!(app.selected_idx.get(), Some(3));
        assert_eq!(app.selected_pattern.get(), None);
    }

    #[test]
    fn select_pattern_resets_editor_mode_to_arrangement() {
        // Mirrors select_chord_loop: opening a pattern editor mounts
        // the new region inside the arrangement surface even if the
        // user was in section-editor mode.
        let app = AppState::new();
        app.open_section_editor("verse", "base");
        assert!(matches!(
            app.editor_mode.get(),
            EditorMode::SectionEditor { .. },
        ));

        app.select_pattern(Some(PatternId::new(9)));
        assert_eq!(app.editor_mode.get(), EditorMode::Arrangement);
    }
}
