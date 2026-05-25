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
use crate::regions::arrangement::ArrangementDragPreview;
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
    ///
    /// **S3 refactor (2026-05-22):** previously used `section_key:
    /// String` (the section's name) which became stale when the user
    /// renamed the section while the editor was open. Now uses the
    /// typed `SectionId` so renames don't desync the editor. The
    /// variant tab is similarly typed.
    SectionEditor {
        section_id: SectionId,
        /// Currently active variant tab. `VariantId::base()` for the
        /// section's base body; otherwise one of the keys in
        /// `Section.variants`.
        variant: VariantId,
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
/// `selected_pattern` (library pattern), `selected_section`
/// (library section template), and `selected_master_fx` (master
/// chain slot) are six distinct selection axes that the inspector
/// / editor surfaces branch on. They're mutually exclusive at the
/// UI level — choosing one clears the others — so the inspector
/// always has a single thing to render.
/// `set_selected_idx`, `select_track`, `select_chord_loop`,
/// `select_pattern`, `select_section`, and `select_master_fx`
/// enforce this so callers don't have to coordinate clears at
/// each click-handler site.
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
    /// Currently selected master-chain slot, by index into
    /// `project.master_chain.fx`. `None` by default; clicking the
    /// "Master" row in the TracksPane sets it to `Some(0)` (the
    /// chain's first slot — round-1 + most projects use the
    /// single safety-net soft-clipper slot). Drives the
    /// `MasterFxEditor` branch of the Inspector (X6 of
    /// `docs/master-fx-chain-plan.md`). X5 only wires the
    /// selection axis + Inspector placeholder branch; X6 ships
    /// the actual editor body.
    pub selected_master_fx: Signal<Option<usize>>,
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

    /// In-flight drag state for the arrangement view's section lane.
    /// Same shape + lifecycle as [`Self::drag_preview`], but tracks
    /// a `SectionBlock` move instead of a chord-event timeline edit.
    /// Lives on its own signal so chord-loop and arrangement drags
    /// stay independent (the user can't drag both at once, but the
    /// types are different and a single signal would have to enum
    /// them — not worth the indirection).
    ///
    /// S5 of `docs/section-arrangement-editing-plan.md`.
    pub arrangement_drag_preview: Signal<Option<ArrangementDragPreview>>,

    /// Arrangement view zoom level: pixels per bar.
    ///
    /// Drives the bar→pixel projection in the timeline primitive
    /// (see [`crate::timeline::projection`]). Bounded by `MIN_PX_PER_BAR`
    /// / `MAX_PX_PER_BAR`. Default `DEFAULT_PX_PER_BAR` matches the
    /// pre-zoom percent-based scale that fits ~24 bars in an
    /// 820px-wide lane at boot.
    ///
    /// Rinch #30 step-1 follow-on — the arrangement view used to
    /// be percent-based (one fixed scale, no zoom). Pixel-based
    /// positioning lets the user zoom into a tight section or
    /// zoom out to see the whole song.
    pub pixels_per_bar: Signal<f32>,

    /// Arrangement view horizontal scroll position, measured in
    /// bars (fractional allowed for smooth scroll). The leftmost
    /// visible bar in the lane viewport.
    ///
    /// Mutated by the wheel-scroll handler on the section lane and
    /// by the zoom controls (zooming around a cursor position
    /// adjusts scroll so the bar under the cursor stays put).
    pub scroll_bars: Signal<f32>,
}

/// Default zoom level: ~34 pixels per bar. Matches the implicit
/// scale the percent-based v1 arrangement view used (820px / 24
/// bars = 34.17 px/bar at 1600×900 with the standard left+right
/// pane widths).
pub const DEFAULT_PX_PER_BAR: f32 = 34.0;

/// Minimum zoom — 8 px/bar lets a ~200-bar arrangement fit in a
/// 1600px window. Smaller than this collapses bar labels into
/// each other and is hard to use.
pub const MIN_PX_PER_BAR: f32 = 8.0;

/// Maximum zoom — 200 px/bar shows ~4 bars per 800px viewport,
/// enough resolution to drop section blocks precisely without
/// turning the lane into a single block.
pub const MAX_PX_PER_BAR: f32 = 200.0;

impl AppState {
    pub fn new() -> Self {
        Self {
            editor_mode: Signal::new(EditorMode::Arrangement),
            selected_idx: Signal::new(Some(1usize)),
            selected_track: Signal::new(None),
            selected_chord_loop: Signal::new(None),
            selected_pattern: Signal::new(None),
            selected_section: Signal::new(None),
            selected_master_fx: Signal::new(None),
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
            arrangement_drag_preview: Signal::new(None),
            pixels_per_bar: Signal::new(DEFAULT_PX_PER_BAR),
            scroll_bars: Signal::new(0.0),
        }
    }

    /// Switch to the section editor for `section_id` at `variant`.
    pub fn open_section_editor(&self, section_id: SectionId, variant: VariantId) {
        self.editor_mode.set(EditorMode::SectionEditor {
            section_id,
            variant,
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
    pub fn set_variant(&self, variant: VariantId) {
        if let EditorMode::SectionEditor { section_id, .. } = self.editor_mode.get() {
            self.editor_mode.set(EditorMode::SectionEditor {
                section_id,
                variant,
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
            self.selected_master_fx.set(None);
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
            self.selected_master_fx.set(None);
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
            self.selected_master_fx.set(None);
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
            self.selected_master_fx.set(None);
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
    /// **S3 (2026-05-22):** also opens the section editor for the
    /// targeted section. Looks up the section's `default_variant`
    /// from the current project and routes through
    /// [`Self::open_section_editor`]. If the id doesn't resolve
    /// (race), falls back to `VariantId::base()`. Closing the
    /// selection (`select_section(None)`) does NOT close the editor
    /// — closing is the Done button's job. This keeps the "Library
    /// row click → editor mount" flow ergonomic without coupling
    /// the close affordances.
    pub fn select_section(&self, id: Option<SectionId>) {
        if let Some(sid) = id {
            self.selected_idx.set(None);
            self.selected_track.set(None);
            self.selected_chord_loop.set(None);
            self.selected_pattern.set(None);
            self.selected_master_fx.set(None);
            let variant = self
                .project
                .get()
                .sections
                .get(&sid)
                .map(|s| s.default_variant.clone())
                .unwrap_or_else(VariantId::base);
            self.open_section_editor(sid, variant);
        }
        self.selected_section.set(id);
    }

    /// Set the currently-selected master-chain slot. `None` clears
    /// the selection. Selecting a slot clears the five other
    /// selection axes (mirrors the other selection setters) so the
    /// inspector branches deterministically on a single axis.
    /// MIDI routing (`midi_target_track`) is untouched — master-FX
    /// selection is not a synth-target switch. Editor mode is also
    /// reset to `Arrangement` so the master-FX editor mounts
    /// inside the standard arrangement row regardless of whether
    /// the user was previously in the section editor.
    ///
    /// X5 of `docs/master-fx-chain-plan.md`. X6 wires the actual
    /// editor body; X5 only provides the selection axis + the
    /// inspector placeholder branch.
    pub fn select_master_fx(&self, slot: Option<usize>) {
        if slot.is_some() {
            self.selected_idx.set(None);
            self.selected_track.set(None);
            self.selected_chord_loop.set(None);
            self.selected_pattern.set(None);
            self.selected_section.set(None);
            self.editor_mode.set(EditorMode::Arrangement);
        }
        self.selected_master_fx.set(slot);
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
