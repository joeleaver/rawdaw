#![allow(dead_code)] // enum variants accrete with the editor; not all are consumed yet

//! Decoration types that the UI layers on top of `rawdaw_model::Project`.
//!
//! These types describe per-cell realization values, role defaults, schedule
//! entries, and the visual state of an activation row. They were originally
//! defined in `crate::fixture::mod` as part of the static round-1 Round1
//! view. C1 of the composition-writability milestone relocates them here so
//! they can be imported as types from a stable address (no longer tangled
//! with the fixture's lifecycle, which is being dismantled).
//!
//! Some of these will move into `rawdaw-model` when Tier-1 patterns ship
//! (`OctaveSpec`, `Humanization`, `Realization` correspond directly to
//! design types in `docs/design/composition-model.md`). Until then they
//! stay app-side as UI decorations.

/// Voicing strategies available in v1 (per `realization.md`).
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
    /// overlay; the real engine surface exposes the full range.
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
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScheduleEntry {
    pub start_bar: u32,
    pub end_bar: u32,
    pub variant: Option<String>,
}

/// Visual state of an activation row / cell. Derived from the model's
/// `ActivationEntry` plus the current variant context (Inherit when the
/// section has no entry at all; Silent when an explicit variant override
/// silences the base; Active otherwise).
///
/// Per the round-1 README this MUST NOT become an
/// `ActivationOverride::Inherit` variant in the engine-side data model —
/// `Inherit` is a UI-side derivation, not a stored state.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ActivationState {
    Active,
    Silent,
    #[default]
    Inherit,
}

/// Per-cell decoration: realization params plus pinned-override count.
/// Keyed by `(SectionId, variant-name, TrackId)` inside [`ProjectOverlay`].
///
/// `pinned` drives the "N pinned" / "no pinned notes" footer in the
/// activation cell; round-3 will populate it from the model's per-note
/// override table.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CellOverlay {
    pub realization: Realization,
    pub pinned: u32,
}

/// UI tag for which kind of track a row represents. Distinct from
/// `rawdaw_model::track::TrackKind` (which carries Role / DrumKitId
/// payloads) — this is the flat discriminator the section editor and
/// inspector branch on for layout decisions (drum rows skip the
/// realization column, etc.).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum TrackKindTag {
    #[default]
    Pitched,
    Drum,
}

impl TrackKindTag {
    /// Build from the model's typed `TrackKind`. Drops the payload
    /// (Role / DrumKitId) — the section-editor view doesn't need it
    /// at the tag level.
    pub fn from_model(kind: &rawdaw_model::track::TrackKind) -> Self {
        match kind {
            rawdaw_model::track::TrackKind::Pitched { .. } => Self::Pitched,
            rawdaw_model::track::TrackKind::Drum { .. } => Self::Drum,
        }
    }
}
