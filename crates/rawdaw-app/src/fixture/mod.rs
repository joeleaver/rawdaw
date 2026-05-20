//! Round-1 fixture data + round-2 extensions. Stand-in for
//! `rawdaw_model::Project` until the engine is wired through.
//!
//! Phase E2 type-flip: the fixture types now hold owned `String` and
//! `Vec` data so a follow-on commit can populate them from
//! `rawdaw_model::fixtures::build_round1_project()` (which returns owned
//! strings). `round1()` returns `&'static Round1` backed by a
//! `std::sync::OnceLock`, so every component still pulls the same
//! reference for the cost of a one-time init.
//!
//! ## Module layout
//!
//! - `mod.rs` (this file) — types + lookup helpers + the public
//!   `round1()` entry point. Stays under the ~700-line cap.
//! - `data` — the round-1 build function (the bulky construction lives
//!   there so neither file approaches the cap) and the round-2 fixture
//!   invariant tests.

#![allow(dead_code)] // fixture fields accrete with the UI; not all are read yet

use std::sync::OnceLock;

mod chord_naming;
mod data;

// ─── Project / track / pattern types ──────────────────────────────────────

#[derive(Clone, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    pub key: String,
    pub time_sig: String,
    pub tempo: u32,
    pub playhead_bar: u32,
    pub playhead_beat: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum TrackKind {
    #[default]
    Pitched,
    Drum,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub kind: TrackKind,
    pub role: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Pattern {
    pub id: String,
    pub name: String,
    pub color: String,
    pub kind: String, // "Pitched" | "Drum"
    pub variants: u32,
    /// Variant id that plays for any bar range not covered by an explicit
    /// entry in an `Activation::variant_schedule`. Mirrors
    /// `Pattern.default_variant` in `composition-model.md`. The schedule
    /// builder uses this at render time — there must be NO phantom default
    /// entry in `variant_schedule`.
    pub default_variant: String,
    pub meta: String,
}

// ─── Realization model (round-2 additions) ────────────────────────────────
//
// The decoration types previously defined here (Voicing, OctaveSpec,
// Humanization, Realization, RoleDefaults, role_defaults, ActivationState,
// ScheduleEntry) moved to `crate::overlay::types` in C1 of the
// composition-writability milestone. Re-exported below so existing
// `use crate::fixture::Voicing` imports keep compiling while the rest of
// the fixture is dismantled phase by phase. New code should import from
// `crate::overlay` directly.

pub use crate::overlay::ActivationState;
pub use crate::overlay::Realization;
pub use crate::overlay::ScheduleEntry;
// The remaining decoration types (Voicing / OctaveSpec / Humanization /
// role_defaults) are imported directly from `crate::overlay` by the
// section-editor cell modules now that the type-relocation lands. The
// re-exports above stay only because `fixture::data` still uses them
// internally; subsequent C1c slices fold them away.

#[derive(Clone, PartialEq, Eq)]
pub struct ChordEvent {
    pub roman: String,
    pub quality: String, // empty for the default quality of the case
    pub absolute: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChordLoop {
    pub id: String,
    pub name: String,
    pub color: String,
    pub length_bars: u32,
    pub events: Vec<ChordEvent>,
}

// `ActivationState` moved to `crate::overlay::types` in C1; re-exported
// at the top of this module for back-compat.

#[derive(Clone, PartialEq, Debug)]
pub struct Activation {
    /// Pattern name, or empty when state is `Inherit`.
    pub pattern: String,
    pub state: ActivationState,
    /// True when this variant of the section silences a base-active
    /// activation. Drives the "*" mark next to the state pill.
    pub overridden: bool,
    /// Round-2: realization parameters. `None` for round-1 use sites
    /// (the inspector activation table doesn't read them); `Some` when
    /// the section editor needs them. Drum activations carry a
    /// `Realization` whose `voicing` / `octave` are `None`.
    pub realization: Option<Realization>,
    /// Round-2: sparse `Vec<(BarRange, Option<VariantId>)>`. Empty list
    /// means "default variant plays the entire activation." Implicit-
    /// default fills are computed at render time — never store a
    /// phantom default entry here.
    pub variant_schedule: Vec<ScheduleEntry>,
    /// Count of pinned per-note overrides (round-3 drill-in target).
    /// Drives the "N pinned" / "no pinned notes" footer in the cell.
    pub per_note_overrides: u32,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Variant {
    pub id: String,
    pub name: String,
}

/// How a variant overrides a single track's activation. Mirrors
/// `ActivationOverride` in `section-variants.md`:
/// - `Silent`: the full activation is silenced for this variant.
///   Realization parameters are preserved but inert; the cell dims and
///   shows a `silent*` pill + "silenced in this variant" footer tag.
/// - `Replace(Activation)`: the entire activation entry is replaced for
///   this variant. Used when the variant needs a different schedule or
///   different realization params (e.g. verse-stripped's lead drops the
///   last bar via a sub-range silence).
///
/// "Inherit" is implicit by absence from the variant's override list.
#[derive(Clone, PartialEq, Debug)]
pub enum ActivationOverride {
    Silent,
    Replace(Activation),
}

/// Sparse per-variant override list: pairs of (track id, override).
pub type VariantOverride = Vec<(String, ActivationOverride)>;

/// `Section` only derives `PartialEq` (not `Eq`) because it transitively
/// contains `Humanization`'s `f32` fields. Identity-by-id is the right
/// comparison anyway — there's no semantic reason to do structural
/// equality including humanization values.
#[derive(Clone, PartialEq)]
pub struct Section {
    pub id: String,
    pub name: String,
    pub color: String,
    pub variants: Vec<Variant>,
    pub default_variant: String,
    pub base_duration_bars: u32,
    /// Chord-loop names attached to this section, in order.
    pub chord_loops: Vec<String>,
    /// Activations keyed by track id.
    pub activations: Vec<(String, Activation)>,
    /// Sparse variant overrides keyed by variant id.
    pub variant_overrides: Vec<(String, VariantOverride)>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SectionRef {
    pub idx: usize,
    pub section_key: String,
    pub variant: String,
    pub start_bar: u32,
    pub bars: u32,
}

pub struct Round1 {
    pub project: Project,
    pub tracks: Vec<Track>,
    pub patterns: Vec<Pattern>,
    pub chord_loops: Vec<ChordLoop>,
    pub sections: Vec<Section>,
    pub arrangement: Vec<SectionRef>,
    pub total_bars: u32,
}

// ─── Round-1 singleton ────────────────────────────────────────────────────

static ROUND1: OnceLock<Round1> = OnceLock::new();

/// Returns the round-1 fixture. Built once on first call; every caller
/// shares the same `&'static Round1`.
pub fn round1() -> &'static Round1 {
    ROUND1.get_or_init(data::build_round1)
}

// ─── Lookup helpers ───────────────────────────────────────────────────────

pub fn section_by_key<'a>(r: &'a Round1, key: &str) -> Option<&'a Section> {
    r.sections.iter().find(|s| s.name == key)
}

pub fn pattern_by_name<'a>(r: &'a Round1, name: &str) -> Option<&'a Pattern> {
    r.patterns.iter().find(|p| p.name == name)
}

pub fn chord_loop_by_name<'a>(r: &'a Round1, name: &str) -> Option<&'a ChordLoop> {
    r.chord_loops.iter().find(|c| c.name == name)
}

pub fn track_by_id<'a>(r: &'a Round1, id: &str) -> Option<&'a Track> {
    r.tracks.iter().find(|t| t.id == id)
}

/// Number of arrangement blocks that point at `section_key`. Drives the
/// "N instances in arrangement" readout in the inspector header.
pub fn instance_count(r: &Round1, section_key: &str) -> usize {
    r.arrangement.iter().filter(|b| b.section_key == section_key).count()
}

/// Look up the base activation for a track in a section. Returns `None`
/// when the track has no entry in that section's activation list — which
/// the round-2 section editor renders as a dashed-border placeholder.
pub fn base_activation<'a>(section: &'a Section, track_id: &str) -> Option<&'a Activation> {
    section
        .activations
        .iter()
        .find(|(tid, _)| tid == track_id)
        .map(|(_, a)| a)
}

/// Look up the override for a track under a given variant. Returns
/// `None` when the variant doesn't override that track (the implicit
/// "inherit from base" case per `section-variants.md`).
pub fn variant_override<'a>(
    section: &'a Section,
    variant_id: &str,
    track_id: &str,
) -> Option<&'a ActivationOverride> {
    for (vid, ov) in section.variant_overrides.iter() {
        if vid != variant_id {
            continue;
        }
        for (tid, entry) in ov.iter() {
            if tid == track_id {
                return Some(entry);
            }
        }
    }
    None
}
