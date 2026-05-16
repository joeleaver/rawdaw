//! Round-1 fixture data + round-2 extensions. Stand-in for
//! `rawdaw_model::Project` until the engine is wired through.
//!
//! The shapes line up with `composition-model.md` so swapping in a real
//! project is mostly a matter of building the view from `Project` fields
//! instead of these statics.
//!
//! ## Module layout
//!
//! - `mod.rs` (this file) — types + lookup helpers + the public
//!   `round1()` entry point. Stays under the ~700-line cap.
//! - `data` — the bulky static tables (tracks / patterns / chord loops /
//!   activations / variant overrides / sections / arrangement) and the
//!   round-2 fixture invariant tests.

#![allow(dead_code)] // fixture fields accrete with the UI; not all are read yet

mod data;

pub use data::round1;

// ─── Project / track / pattern types ──────────────────────────────────────

#[derive(Clone, PartialEq, Eq)]
pub struct Project {
    pub name: &'static str,
    pub key: &'static str,
    pub time_sig: &'static str,
    pub tempo: u32,
    pub playhead_bar: u32,
    pub playhead_beat: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Pitched,
    Drum,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Track {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: TrackKind,
    pub role: &'static str,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Pattern {
    pub id: &'static str,
    pub name: &'static str,
    pub color: &'static str,
    pub kind: &'static str, // "Pitched" | "Drum"
    pub variants: u32,
    /// Variant id that plays for any bar range not covered by an explicit
    /// entry in an `Activation::variant_schedule`. Mirrors
    /// `Pattern.default_variant` in `composition-model.md`. The schedule
    /// builder uses this at render time — there must be NO phantom default
    /// entry in `variant_schedule`.
    pub default_variant: &'static str,
    pub meta: &'static str,
}

// ─── Realization model (round-2 additions) ────────────────────────────────
//
// Mirrors the realization vocabulary in `docs/design/realization.md` and
// `docs/design/composition-model.md`. Pitched cells have a voicing
// strategy + octave spec + humanization; drum cells have only
// humanization (drums are pitch-symbolic — voicing & octave do not
// apply, per round-2 decision 17).
//
// `voicingFromRole` / `octaveFromRole` flags from the JS fixture are
// intentionally NOT modelled here. The "matches role default"
// inheritance signal is computed at render time by comparing the
// activation's value against `role_defaults(track.role)`. Storing both
// the value and a flag would be two pieces of state for one fact — a
// drift hazard called out in the round-2 README's port-time notes.

/// Voicing strategies available v1 (per `realization.md`).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Voicing {
    #[default]
    TriadClose,
    TriadOpen,
    FourWayClose,
    Drop2,
    Drop3,
    Shell,
    Rootless,
    Power,
}

impl Voicing {
    /// Mockup-facing label.
    pub fn label(self) -> &'static str {
        match self {
            Self::TriadClose => "triad-close",
            Self::TriadOpen => "triad-open",
            Self::FourWayClose => "four-way-close",
            Self::Drop2 => "drop2",
            Self::Drop3 => "drop3",
            Self::Shell => "shell",
            Self::Rootless => "rootless",
            Self::Power => "power",
        }
    }
}

/// Per-event octave choice (`OctaveSpec` in `composition-model.md`).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum OctaveSpec {
    /// Voice-leading minimal-motion (default).
    #[default]
    Nearest,
    /// Pin to a specific octave.
    Anchored(u32),
    /// Force an upward leap from the previous note.
    UpFromPrev,
    /// Force a downward leap from the previous note.
    DownFromPrev,
    /// Use the role's default register, ignoring `last_pitch`.
    RelativeToRole,
}

impl OctaveSpec {
    /// Mockup-facing label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Nearest => "Nearest",
            Self::Anchored(2) => "Anchored · 2",
            Self::Anchored(3) => "Anchored · 3",
            Self::Anchored(4) => "Anchored · 4",
            Self::Anchored(5) => "Anchored · 5",
            // Other octaves fall back to a generic format (no allocation
            // for the common pre-baked octaves above).
            Self::Anchored(_) => "Anchored · n",
            Self::UpFromPrev => "Up from prev",
            Self::DownFromPrev => "Down from prev",
            Self::RelativeToRole => "Relative to role",
        }
    }
}

/// Humanization parameters (per `realization.md`). Seed lives on the
/// activation entry, so two activations using the same pattern can
/// humanize differently.
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct Humanization {
    /// Velocity jitter as a fraction (0.04 = ±4 %).
    pub velocity: f32,
    /// Timing jitter in ticks (PPQ 960).
    pub timing: u32,
    /// 0.0 = straight, 0.5 = full triplet swing.
    pub swing: f32,
    /// `u64` per `realization.md`. Display as a 5–6 digit decimal in the
    /// fixture; the real engine surface exposes the full range.
    pub seed: u64,
}

/// Pitched cells carry a full `Realization`; drum cells have only
/// `humanization` (the other two fields are `None`).
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct Realization {
    pub voicing: Option<Voicing>,
    pub octave: Option<OctaveSpec>,
    pub humanization: Humanization,
}

/// Per-role defaults for the realization parameters. The activation cell
/// renders a `↳ role default` tag when the activation's value equals the
/// role's default for the corresponding field.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RoleDefaults {
    pub voicing: Voicing,
    pub octave: OctaveSpec,
    pub humanization: Humanization,
}

/// Look up the per-role defaults table. Drum tracks have no role and
/// receive `None`. Returns `None` for unknown roles too — the activation
/// cell will treat that as "no role default applies."
pub fn role_defaults(role: &str) -> Option<RoleDefaults> {
    match role {
        "bass" => Some(RoleDefaults {
            voicing: Voicing::Power,
            octave: OctaveSpec::Nearest,
            humanization: Humanization { velocity: 0.04, timing: 4, swing: 0.0, seed: 0 },
        }),
        "voicing" => Some(RoleDefaults {
            voicing: Voicing::FourWayClose,
            octave: OctaveSpec::Nearest,
            humanization: Humanization { velocity: 0.06, timing: 6, swing: 0.0, seed: 0 },
        }),
        "arp" => Some(RoleDefaults {
            voicing: Voicing::TriadClose,
            octave: OctaveSpec::Nearest,
            humanization: Humanization { velocity: 0.05, timing: 3, swing: 0.0, seed: 0 },
        }),
        "melodic" => Some(RoleDefaults {
            voicing: Voicing::TriadClose,
            octave: OctaveSpec::Nearest,
            humanization: Humanization { velocity: 0.06, timing: 5, swing: 0.0, seed: 0 },
        }),
        "pad" => Some(RoleDefaults {
            voicing: Voicing::TriadOpen,
            octave: OctaveSpec::Anchored(3),
            humanization: Humanization { velocity: 0.02, timing: 2, swing: 0.0, seed: 0 },
        }),
        "countermel" => Some(RoleDefaults {
            voicing: Voicing::Shell,
            octave: OctaveSpec::Nearest,
            humanization: Humanization { velocity: 0.05, timing: 4, swing: 0.0, seed: 0 },
        }),
        _ => None,
    }
}

/// One entry in an activation's `variant_schedule`:
/// `(BarRange, Option<VariantId>)`.
///
/// - `range = (start, end)` — inclusive-start, exclusive-end bars.
/// - `variant = Some(id)` — non-default pattern variant for this range.
/// - `variant = None` — silenced sub-range (`(BarRange, None)`).
///
/// The list is **sparse**. Bar ranges not covered by any entry play the
/// pattern's `default_variant`. The render-time schedule builder fills
/// implicit-default gaps; there must never be a phantom default entry in
/// the data.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScheduleEntry {
    pub start_bar: u32,
    pub end_bar: u32,
    pub variant: Option<&'static str>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChordEvent {
    pub roman: &'static str,
    pub quality: &'static str, // empty for the default quality of the case
    pub absolute: &'static str,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ChordLoop {
    pub id: &'static str,
    pub name: &'static str,
    pub color: &'static str,
    pub length_bars: u32,
    pub events: &'static [ChordEvent],
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ActivationState {
    Active,
    Silent,
    /// No entry in the section's activation map for this track. Displayed
    /// as a third pill purely so the user sees something; per the round-1
    /// README this MUST NOT become an `ActivationOverride::Inherit`
    /// variant in the engine-side data model.
    #[default]
    Inherit,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Activation {
    /// Pattern name, or empty when state is `Inherit`.
    pub pattern: &'static str,
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
    pub variant_schedule: &'static [ScheduleEntry],
    /// Count of pinned per-note overrides (round-3 drill-in target).
    /// Drives the "N pinned" / "no pinned notes" footer in the cell.
    pub per_note_overrides: u32,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Variant {
    pub id: &'static str,
    pub name: &'static str,
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
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActivationOverride {
    Silent,
    Replace(Activation),
}

/// Sparse per-variant override list: pairs of (track id, override).
pub type VariantOverride = &'static [(&'static str, ActivationOverride)];

/// `Section` only derives `PartialEq` (not `Eq`) because it transitively
/// contains `Humanization`'s `f32` fields. Identity-by-id is the right
/// comparison anyway — there's no semantic reason to do structural
/// equality including humanization values.
#[derive(Clone, PartialEq)]
pub struct Section {
    pub id: &'static str,
    pub name: &'static str,
    pub color: &'static str,
    pub variants: &'static [Variant],
    pub default_variant: &'static str,
    pub base_duration_bars: u32,
    /// Chord-loop names attached to this section, in order.
    pub chord_loops: &'static [&'static str],
    /// Activations keyed by track id.
    pub activations: &'static [(&'static str, Activation)],
    /// Sparse variant overrides keyed by variant id.
    pub variant_overrides: &'static [(&'static str, VariantOverride)],
}

#[derive(Clone, PartialEq, Eq)]
pub struct SectionRef {
    pub idx: usize,
    pub section_key: &'static str,
    pub variant: &'static str,
    pub start_bar: u32,
    pub bars: u32,
}

pub struct Round1 {
    pub project: Project,
    pub tracks: &'static [Track],
    pub patterns: &'static [Pattern],
    pub chord_loops: &'static [ChordLoop],
    pub sections: &'static [Section],
    pub arrangement: &'static [SectionRef],
    pub total_bars: u32,
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
        .find(|(tid, _)| *tid == track_id)
        .map(|(_, a)| a)
}

/// Look up the override for a track under a given variant. Returns
/// `None` when the variant doesn't override that track (the implicit
/// "inherit from base" case per `section-variants.md`).
pub fn variant_override(
    section: &Section,
    variant_id: &str,
    track_id: &str,
) -> Option<ActivationOverride> {
    for (vid, ov) in section.variant_overrides.iter() {
        if *vid != variant_id {
            continue;
        }
        for (tid, entry) in ov.iter() {
            if *tid == track_id {
                return Some(*entry);
            }
        }
    }
    None
}
