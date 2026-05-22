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

use rawdaw_model::id::{ChordLoopId, NoteId, PatternId, SectionId, VariantId};
use rawdaw_model::pitch::PitchClass;
use rawdaw_model::project::Project;
use rawdaw_model::scale::Scale;

use crate::overlay::ProjectOverlay;
use crate::regions::chord_loop_editor::DragPreview;

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
/// track), `selected_chord_loop` (library chord-loop),
/// `selected_pattern` (library pattern), and `selected_section`
/// (library section template) are five distinct selection axes that
/// the inspector / editor surfaces branch on. They're mutually
/// exclusive at the UI level — choosing one clears the others — so
/// the inspector always has a single thing to render.
/// `set_selected_idx`, `select_track`, `select_chord_loop`,
/// `select_pattern`, and `select_section` enforce this so callers
/// don't have to coordinate clears at each click-handler site.
///
/// `selected_section` is the S2 addition (one deviation from the
/// section-arrangement-editing plan's S0 decision 10, which said
/// "no new top-level axis"). Library section-row clicks need
/// somewhere to write — the existing `selected_idx` axis is an
/// arrangement-step index, not a section id, and the section editor
/// today is opened via an `EditorMode::SectionEditor { section_key,
/// ... }` *name string* set by `open_section_editor`. Adding the axis
/// at S2 keeps the four-other-axis precedent intact; S3's meta-bar
/// work will re-wire the section editor to be section-id-aware off
/// this signal.
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
    /// Currently selected section template in the library, by id.
    /// `None` by default; clicking a section row in the Library panel
    /// sets it (and clears the four other selection axes per the
    /// mutex). S2 only uses this for the library row's visual
    /// selection highlight; S3 reads it to open the section editor
    /// for a section that isn't yet placed in the arrangement.
    pub selected_section: Signal<Option<SectionId>>,
    /// Index into the focused chord loop's event vec of the
    /// currently-focused chord event. Drives the chord-loop editor's
    /// inspector pane (CL2). Lives on AppState rather than as a
    /// component-local Signal so the timeline + inspector + future
    /// CL4 realized strip share a single source of truth and
    /// don't need to plumb a `Signal<Option<usize>>` through props
    /// (rinch's `#[component]` requires every prop's type to
    /// implement `Default`, which `Signal<Option<usize>>` does not).
    pub focused_chord_event_idx: Signal<Option<usize>>,
    /// Currently focused pitched-pattern note inside the selected
    /// pattern's currently-focused variant. Drives the pattern editor's
    /// per-note inspector (P2 of `docs/pattern-editor-plan.md`).
    ///
    /// **Keyed by durable `NoteId`, not by index** (P2 design
    /// decision 9): patterns are mutable and id-stable; idx-keying
    /// would break under insert/delete. The inspector's value-fn
    /// dispatchers look up the note by id on every read.
    pub focused_pattern_note: Signal<Option<NoteId>>,
    /// Currently-focused variant tab inside the selected pattern.
    /// `None` when no pattern is selected; reset to the pattern's
    /// `default_variant` whenever a new pattern is selected via
    /// [`Self::select_pattern`]. Shared across pitched + drum editor
    /// bodies so variant tab affordances behave identically.
    pub focused_variant: Signal<Option<VariantId>>,
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

    /// In-flight drag state for the chord-loop editor's timeline.
    /// `None` when no drag is active; `Some(preview)` while an
    /// `EventBlock` is being moved or resized so the block can
    /// render a live preview off the signal without going through
    /// `apply_project_edit` per pointer-move. The drag handler
    /// writes once on `on_end` to commit, then clears the signal.
    ///
    /// CL2.x of `docs/chord-loop-editing-plan.md`.
    pub drag_preview: Signal<Option<DragPreview>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            editor_mode: Signal::new(EditorMode::Arrangement),
            selected_idx: Signal::new(Some(1usize)),
            selected_track: Signal::new(None),
            selected_chord_loop: Signal::new(None),
            selected_pattern: Signal::new(None),
            selected_section: Signal::new(None),
            focused_chord_event_idx: Signal::new(None),
            focused_pattern_note: Signal::new(None),
            focused_variant: Signal::new(None),
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
            drag_preview: Signal::new(None),
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
            self.selected_section.set(None);
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
            self.selected_section.set(None);
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
            self.selected_section.set(None);
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
            self.selected_section.set(None);
            self.editor_mode.set(EditorMode::Arrangement);
        }
        // P2: switching which pattern is open invalidates whatever
        // note was focused — clear it so the inspector lands in its
        // empty state. The variant focus resets to the pattern's
        // `default_variant` (or `None` on clear) so the variant tab
        // bar always renders a defined initial state.
        self.focused_pattern_note.set(None);
        self.focused_variant.set(match id {
            Some(pid) => self
                .project
                .get()
                .patterns
                .get(&pid)
                .map(|p| p.default_variant.clone()),
            None => None,
        });
        self.selected_pattern.set(id);
    }

    /// Set the currently-selected library section template. `None`
    /// clears the selection. Selecting a section clears the four
    /// other selection axes (mirrors [`Self::select_chord_loop`] and
    /// [`Self::select_pattern`]). MIDI routing
    /// (`midi_target_track`) is untouched — section selection is
    /// not a synth-target switch.
    ///
    /// Unlike [`Self::select_chord_loop`] / [`Self::select_pattern`]
    /// this does NOT reset `editor_mode` — the section editor mounts
    /// off `EditorMode::SectionEditor` independently. S3 will wire
    /// `selected_section` into the section editor's open path so the
    /// flow becomes "click Library row → editor opens for that
    /// section template, no arrangement step required."
    pub fn select_section(&self, id: Option<SectionId>) {
        if id.is_some() {
            self.selected_idx.set(None);
            self.selected_track.set(None);
            self.selected_chord_loop.set(None);
            self.selected_pattern.set(None);
        }
        self.selected_section.set(id);
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

// State tests live in a sibling file so `state.rs` stays under the
// 700-line cap once S2's five-way selection-mutex tests landed. The
// `#[path]` attribute lets us keep a flat module layout (no
// `state/mod.rs` rename); cargo test picks them up identically.
#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
