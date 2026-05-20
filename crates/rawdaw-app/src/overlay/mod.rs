//! UI-only decorations layered on top of `rawdaw_model::Project`.
//!
//! Per the composition-writability plan ([C1](../../../../docs/composition-writability-plan.md)),
//! anything the UI needs that is NOT structural project data (pattern /
//! section / chord-loop colors, the `Pitched · N variants` library meta
//! strings, per-cell realization decorations) lives in this parallel
//! [`ProjectOverlay`] store. The split keeps `rawdaw-model` free of
//! presentation concerns (CLAUDE.md rule 1).
//!
//! The struct is keyed by the model's typed IDs (`PatternId`, `SectionId`,
//! `ChordLoopId`, `TrackId`) so overlay entries can survive renames /
//! reorders of model items. The per-cell entries are also keyed by
//! variant name; once Tier-1 patterns ship and the model grows a
//! `VariantId` newtype on activation entries, switch the cell key's
//! middle field from `String` to `VariantId`.
//!
//! Colors and meta strings are stored as owned `String` — Tier-1 work
//! includes letting the user pick colors, and C3 of the writability plan
//! persists overlays alongside projects via serde, both of which need
//! owned strings rather than `&'static str` literals.

use std::collections::BTreeMap;

use rawdaw_model::id::{ChordLoopId, PatternId, SectionId, TrackId};

pub mod types;

pub use types::{
    role_defaults, ActivationState, CellOverlay, Humanization, OctaveSpec, Realization,
    ScheduleEntry, TrackKindTag, Voicing,
};

// `RoleDefaults` (the struct returned by `role_defaults`) is intentionally
// not re-exported at this level — callers consume it via the function's
// return type and access fields directly. Add a re-export here when an
// external use site needs to name the type.

/// UI-only decorations layered on top of a [`rawdaw_model::project::Project`].
///
/// Stored in `AppState` alongside the project itself; saved alongside the
/// project as part of `SavedBundle` in C3 of the writability plan.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectOverlay {
    /// Per-pattern accent color (`#RRGGBB`). Absent entries fall back to
    /// `theme::TEXT2` at render time.
    pub pattern_color: BTreeMap<PatternId, String>,
    /// Per-pattern library meta string (e.g. `"Pitched · 2 variants"`).
    /// Computable from the model in principle, but the wording is a UI
    /// concern; storing it lets the library panel render without
    /// reaching back into pattern bodies on every paint.
    pub pattern_meta: BTreeMap<PatternId, String>,
    /// Per-section accent color (`#RRGGBB`).
    pub section_color: BTreeMap<SectionId, String>,
    /// Per-chord-loop accent color (`#RRGGBB`).
    pub chord_loop_color: BTreeMap<ChordLoopId, String>,
    /// Per-cell realization decoration keyed by `(section, variant, track)`.
    /// Variant key is the variant's name string today; switches to a typed
    /// `VariantId` when patterns + activations get their full model.
    pub cell: BTreeMap<CellKey, CellOverlay>,
}

/// Composite key into [`ProjectOverlay::cell`].
///
/// The triple `(section, variant, track)` uniquely identifies a single
/// activation row under one variant tab. Variant names are owned because
/// the round-1 fixture mixes literal `"base"` / `"stripped"` / `"chorus-
/// final"` and lookups by dynamic strings need owned keys.
pub type CellKey = (SectionId, String, TrackId);

impl ProjectOverlay {
    /// Empty overlay — every lookup returns `None`. Useful as a default
    /// for tests that don't care about decorations.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Look up the per-cell decoration for one activation. Returns `None`
    /// when no entry exists; callers should treat that as "use role
    /// defaults" for realization fields.
    pub fn lookup_cell(
        &self,
        section: SectionId,
        variant: &str,
        track: TrackId,
    ) -> Option<CellOverlay> {
        self.cell
            .iter()
            .find(|((s, v, t), _)| *s == section && *t == track && v == variant)
            .map(|(_, c)| *c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_overlay_returns_none_for_all_lookups() {
        let o = ProjectOverlay::empty();
        assert!(o.pattern_color.is_empty());
        assert!(
            o.lookup_cell(SectionId::new(0), "base", TrackId::new(0))
                .is_none()
        );
    }

    #[test]
    fn cell_lookup_matches_section_variant_track_triple() {
        let mut o = ProjectOverlay::empty();
        let sid = SectionId::new(7);
        let tid = TrackId::new(3);
        let overlay = CellOverlay {
            realization: Realization::default(),
            pinned: 4,
        };
        o.cell.insert((sid, "stripped".to_string(), tid), overlay);

        assert_eq!(o.lookup_cell(sid, "stripped", tid), Some(overlay));
        assert!(o.lookup_cell(sid, "base", tid).is_none());
        assert!(o.lookup_cell(sid, "stripped", TrackId::new(4)).is_none());
        assert!(o.lookup_cell(SectionId::new(8), "stripped", tid).is_none());
    }
}
