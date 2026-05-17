//! Activation cells — the rows that fill the body of the section
//! editor. One cell per (section × track), resolved against the
//! currently-active variant.
//!
//! Mirrors `mockups/round-2/components/activation-cell.jsx`. The three
//! cell columns split across submodules so each piece stays small:
//!
//! - `activation_cell` — outer 3-column shell + realization / schedule
//!   placeholders. The realization (col 2) and schedule (col 3)
//!   columns are stubbed in Phase 4 and filled in by Phases 5 and 6.
//! - `identity_column` — col 1 (track row + pattern card + footer).
//! - `cell_inherit` — dashed-border placeholder for tracks that have
//!   no entry in either base or the active variant. Per round-2
//!   README decision 14, these render in place of an active cell so
//!   the user sees every project track.
//!
//! ## Resolution semantics
//!
//! Mirrors the JS mockup's `effective` block (`activation-cell.jsx`
//! lines ~10-30). Variant override wins over base; an override of
//! `Silent` keeps base's pattern + realization but flips state to
//! Silent; an override of `Replace(act)` swaps the entry entirely.
//! Both absent → render `CellInherit`.
//!
//! No phantom default entries; no stored inheritance flags. The
//! computed-not-stored rule from the round-2 README applies here too.

use rinch::prelude::*;

use crate::fixture::{
    self, Activation, ActivationOverride, ActivationState, Section, TrackKind,
};
use crate::state::{AppState, EditorMode};
use crate::theme;

mod activation_cell;
mod cell_inherit;
mod identity_column;
mod realization_column;
mod schedule_column;

use activation_cell::ActivationCell;
use cell_inherit::CellInherit;

/// Renders the list of activation cells below the
/// `ActivationsHeader`. Reads section + variant from the shared
/// `AppState` store. Iterates `r.tracks` in project order — one cell
/// per track, always; tracks with no entry get a dashed `CellInherit`
/// placeholder.
#[component]
pub fn CellList() -> NodeHandle {
    let app = use_store::<AppState>();
    let section_key = match app.editor_mode.get() {
        EditorMode::SectionEditor { section_key, .. } => section_key,
        EditorMode::Arrangement => String::new(),
    };

    let outer_style = "display: flex; flex-direction: column; gap: 10px; \
         padding: 4px 20px 20px;"
        .to_string();
    let _ = theme::BG0; // theme tokens used by leaf cells, not here
    let section_key_for_iter = section_key.clone();

    rsx! {
        div { style: {outer_style.clone()},
            for slot in resolve_cells(section_key_for_iter.clone(), variant_from_store()) {
                CellRow {
                    key: slot.track_id.clone(),
                    slot: slot,
                }
            }
        }
    }
}

/// Reactive scalar reader — grabs the current variant out of the store
/// each time the `for` source re-runs. Pulled out of `CellList` so the
/// rsx for-source closure stays small.
fn variant_from_store() -> String {
    let app = use_store::<AppState>();
    match app.editor_mode.get() {
        EditorMode::SectionEditor { variant, .. } => variant,
        EditorMode::Arrangement => String::new(),
    }
}

/// One resolved cell — either an active/silent activation with a
/// pattern bound, or an inherit placeholder. Owned (String / Copy
/// fields only) so it satisfies the `Clone + PartialEq + 'static`
/// bound on rsx `for` items, and `Default` so it can pass through
/// `#[component]` props (the macro's auto-generated `Default` impl
/// requires every field type to itself implement `Default`).
#[derive(Clone, PartialEq, Default)]
pub struct CellSlot {
    pub track_id: String,
    pub track_name: String,
    pub track_kind: TrackKind,
    pub track_role: String,
    /// Section duration in bars — copied onto each slot because all
    /// cells in a render share the same section, and the schedule
    /// column needs it to draw the timeline. Cheaper than threading a
    /// parent prop down through CellRow / ActivationCell.
    pub total_bars: u32,
    pub variant: ResolvedVariant,
}

#[derive(Clone, PartialEq)]
pub enum ResolvedVariant {
    /// Track has an effective activation in this section/variant.
    Active {
        pattern_name: String,
        pattern_color: String,
        pattern_kind: String,
        /// `Pattern.default_variant` — the variant id that plays for
        /// any bar range not covered by an entry in the
        /// `Activation.variant_schedule`. The Phase 6 schedule
        /// builder uses this at render time.
        pattern_default_variant: String,
        state: ActivationState,
        /// True when the variant override is the source of this entry
        /// (drives the `*` mark on the state pill and the source label
        /// in the cell footer).
        overridden_by_variant: bool,
        /// `"silenced in this variant"` or `"replaced in this variant"`
        /// when the variant override produced this slot; `None` on
        /// pure-base entries.
        source_label: Option<String>,
        /// Carry the resolved `Activation` so future phases (realization,
        /// schedule) can read humanization / overrides without
        /// re-walking the merge.
        activation: Activation,
    },
    /// Track has no entry in either base or the active variant. This
    /// is the natural empty state, so it's the `Default` impl —
    /// keeps `CellSlot::default()` cheap. Manual impl rather than
    /// `#[derive(Default)] + #[default]` because rustc only accepts
    /// the attribute on *unit* enum variants.
    Inherit {
        reason: &'static str,
    },
}

impl Default for ResolvedVariant {
    fn default() -> Self {
        Self::Inherit { reason: "" }
    }
}

fn resolve_cells(section_key: String, variant: String) -> Vec<CellSlot> {
    let r = fixture::round1();
    let Some(section) = fixture::section_by_key(&r, section_key.as_str()) else {
        return Vec::new();
    };

    let total_bars = section.base_duration_bars;
    r.tracks
        .iter()
        .map(|track| {
            let variant = resolve_one(section, variant.as_str(), track.id);
            CellSlot {
                track_id: track.id.to_string(),
                track_name: track.name.to_string(),
                track_kind: track.kind,
                track_role: track.role.to_string(),
                total_bars,
                variant,
            }
        })
        .collect()
}

fn resolve_one(section: &Section, variant_id: &str, track_id: &str) -> ResolvedVariant {
    // Look up the variant-override entry for (variant_id, track_id), if
    // any. `variant_overrides` is sparse — absence means "inherit base."
    let override_entry = section
        .variant_overrides
        .iter()
        .find(|(vid, _)| *vid == variant_id)
        .and_then(|(_, list)| list.iter().find(|(tid, _)| *tid == track_id))
        .map(|(_, ov)| ov);

    let base = section
        .activations
        .iter()
        .find(|(tid, _)| *tid == track_id)
        .map(|(_, a)| *a);

    match (override_entry, base) {
        (None, None) => ResolvedVariant::Inherit {
            reason: "no entry in base",
        },
        (None, Some(base)) => activation_to_variant(base, false, None),
        (Some(ActivationOverride::Silent), Some(base)) => {
            // Silent keeps base's pattern + realization, flips state.
            let mut act = base;
            act.state = ActivationState::Silent;
            act.overridden = true;
            activation_to_variant(act, true, Some("silenced in this variant".to_string()))
        }
        (Some(ActivationOverride::Silent), None) => ResolvedVariant::Inherit {
            // The mockup doesn't render this combination — base has to
            // exist for a Silent override to silence anything. Mirror
            // that: fall back to Inherit with a diagnostic reason.
            reason: "silent override without base",
        },
        (Some(ActivationOverride::Replace(act)), _) => {
            let mut act = *act;
            act.overridden = true;
            activation_to_variant(act, true, Some("replaced in this variant".to_string()))
        }
    }
}

fn activation_to_variant(
    act: Activation,
    overridden_by_variant: bool,
    source_label: Option<String>,
) -> ResolvedVariant {
    let r = fixture::round1();
    let pat = fixture::pattern_by_name(&r, act.pattern);
    let pattern_default_variant = pat
        .map(|p| p.default_variant.to_string())
        .unwrap_or_default();
    let (pattern_color, pattern_kind) = pat
        .map(|p| (p.color.to_string(), p.kind.to_string()))
        .unwrap_or_else(|| (theme::TEXT2.to_string(), String::new()));
    ResolvedVariant::Active {
        pattern_name: act.pattern.to_string(),
        pattern_color,
        pattern_kind,
        pattern_default_variant,
        state: act.state,
        overridden_by_variant,
        source_label,
        activation: act,
    }
}

/// One row of the cell list. Switches between `ActivationCell` and
/// `CellInherit` based on the resolved slot.
#[component]
fn CellRow(slot: CellSlot) -> NodeHandle {
    match slot.variant {
        ResolvedVariant::Active {
            pattern_name,
            pattern_color,
            pattern_kind,
            pattern_default_variant,
            state,
            overridden_by_variant,
            source_label,
            activation,
        } => {
            // The realization parameters travel into the cell so Phase 5's
            // RealizationColumn can compare them against the track role's
            // defaults (`fixture::role_defaults`) and surface
            // `↳ role default` vs `*` overrides per field. Phase 6 reads
            // `activation.variant_schedule` similarly. Converting the
            // sparse `&'static [ScheduleEntry]` into an owned
            // `Vec<ScheduleEntry>` keeps `ScheduleColumn`'s prop owned
            // (the `#[component]` macro requires owned param types).
            let realization = activation.realization;
            let schedule: Vec<crate::fixture::ScheduleEntry> =
                activation.variant_schedule.to_vec();
            let total_bars = slot.total_bars;
            rsx! {
                ActivationCell {
                    track_name: slot.track_name,
                    track_kind: slot.track_kind,
                    track_role: slot.track_role,
                    pattern_name: pattern_name,
                    pattern_color: pattern_color,
                    pattern_kind: pattern_kind,
                    pattern_default_variant: pattern_default_variant,
                    state: state,
                    overridden_by_variant: overridden_by_variant,
                    source_label: source_label.unwrap_or_default(),
                    realization: realization,
                    schedule: schedule,
                    total_bars: total_bars,
                }
            }
        }
        ResolvedVariant::Inherit { reason } => rsx! {
            CellInherit {
                track_name: slot.track_name,
                track_kind: slot.track_kind,
                track_role: slot.track_role,
                reason: reason.to_string(),
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_in_verse_base_resolves_to_inherit() {
        // Per fixture: verse@base has no pad entry (decision 14).
        let r = fixture::round1();
        let section = fixture::section_by_key(&r, "verse").expect("verse exists");
        let resolved = resolve_one(section, "base", "t_pad");
        match resolved {
            ResolvedVariant::Inherit { reason } => {
                assert_eq!(reason, "no entry in base");
            }
            _ => panic!("expected Inherit, got Active"),
        }
    }

    #[test]
    fn bass_in_verse_stripped_resolves_to_silent_variant_override() {
        // Per fixture: verse-stripped overrides bass with Silent.
        let r = fixture::round1();
        let section = fixture::section_by_key(&r, "verse").expect("verse exists");
        let resolved = resolve_one(section, "stripped", "t_bass");
        match resolved {
            ResolvedVariant::Active {
                state,
                overridden_by_variant,
                source_label,
                ..
            } => {
                assert_eq!(state, ActivationState::Silent);
                assert!(overridden_by_variant);
                assert_eq!(source_label.as_deref(), Some("silenced in this variant"));
            }
            _ => panic!("expected Active(Silent), got Inherit"),
        }
    }

    #[test]
    fn lead_in_verse_stripped_resolves_to_replace_override() {
        let r = fixture::round1();
        let section = fixture::section_by_key(&r, "verse").expect("verse exists");
        let resolved = resolve_one(section, "stripped", "t_lead");
        match resolved {
            ResolvedVariant::Active {
                overridden_by_variant,
                source_label,
                ..
            } => {
                assert!(overridden_by_variant);
                assert_eq!(source_label.as_deref(), Some("replaced in this variant"));
            }
            _ => panic!("expected Active(replace), got Inherit"),
        }
    }

    #[test]
    fn drums_in_verse_base_resolves_to_active_no_override() {
        let r = fixture::round1();
        let section = fixture::section_by_key(&r, "verse").expect("verse exists");
        let resolved = resolve_one(section, "base", "t_drums");
        match resolved {
            ResolvedVariant::Active {
                state,
                overridden_by_variant,
                source_label,
                ..
            } => {
                assert_eq!(state, ActivationState::Active);
                assert!(!overridden_by_variant);
                assert!(source_label.is_none());
            }
            _ => panic!("expected Active, got Inherit"),
        }
    }
}
