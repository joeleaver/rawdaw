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

use rawdaw_model::id::{PatternId, SectionId, TrackId, VariantId};
use rawdaw_model::pattern::PatternBody;
use rawdaw_model::project::Project;
use rawdaw_model::section::{ActivationOverride as ModelActivationOverride, Section};
use rawdaw_model::track::{Role, TrackKind as ModelTrackKind};

use crate::overlay::{ActivationState, ProjectOverlay, Realization, ScheduleEntry, TrackKindTag};
use crate::state::{AppState, EditorMode};
use crate::theme;

/// Sentinel `VariantId` value the round-2 model uses to mean "this
/// sub-range is silent." Surfaced here as a constant rather than a
/// literal so subsequent C2 work can replace the sentinel with a
/// typed `Option<VariantId>` on `ActivationEntry.variant_schedule`.
const SILENT_VARIANT_SENTINEL: &str = "__silent__";

mod activation_cell;
mod cell_inherit;
mod editable_schedule_cell;
mod identity_column;
mod pattern_select;
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
                    key: slot.track_id.get(),
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
    pub section_id: SectionId,
    pub track_id: TrackId,
    pub track_name: String,
    pub track_kind: TrackKindTag,
    pub track_role: String,
    /// Section duration in bars — copied onto each slot because all
    /// cells in a render share the same section, and the schedule
    /// column needs it to draw the timeline. Cheaper than threading a
    /// parent prop down through CellRow / ActivationCell.
    pub total_bars: u32,
    /// Currently-bound pattern id (raw u64), or 0 when no pattern is
    /// bound. The pattern picker uses this as its `value` prop so
    /// project edits re-bind the Select.
    pub bound_pattern_value: u64,
    pub variant: ResolvedVariant,
}

// `Active` carries a fully-resolved set of UI strings plus the
// realization + schedule the cell components consume; substantially
// bigger than `Inherit`. ResolvedVariant lives briefly inside per-
// render `Vec<CellSlot>` — at most one entry per project track per
// cell render — so boxing to equalize variant sizes would trade a
// meaningful allocation for negligible memory savings. Suppress the
// warning rather than add the indirection.
#[allow(clippy::large_enum_variant)]
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
        /// Realization decorations — humanization / voicing / octave —
        /// resolved from overlay.lookup_cell. `None` when the overlay
        /// has no cell entry for this `(section, variant, track)`
        /// triple, in which case the realization column falls back to
        /// role defaults.
        realization: Option<Realization>,
        /// Variant-schedule sub-range pins, converted from the model's
        /// `(BarRange, VariantId)` entries. The sentinel
        /// `SILENT_VARIANT_SENTINEL` collapses to `variant: None`
        /// (silenced sub-range); any other id becomes
        /// `variant: Some(name)`.
        schedule: Vec<ScheduleEntry>,
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
    let app = use_store::<AppState>();
    let project = app.project.get();
    let overlay = app.overlay.get();
    let Some(section) = project
        .sections
        .values()
        .find(|s| s.name == section_key)
        .cloned()
    else {
        return Vec::new();
    };

    let total_bars = section.base.duration_bars;
    let section_id = section.id;
    let variant_id = VariantId::from(variant.as_str());
    project
        .tracks
        .iter()
        .map(|track| {
            let resolved = resolve_one(&project, &overlay, &section, &variant, track.id);
            let bound_pattern_value = effective_pattern_id(&section, track.id, &variant_id)
                .map(PatternId::get)
                .unwrap_or(pattern_select::NO_PATTERN_SENTINEL);
            CellSlot {
                section_id,
                track_id: track.id,
                track_name: track.name.clone(),
                track_kind: TrackKindTag::from_model(&track.kind),
                track_role: track_role_label(&track.kind),
                total_bars,
                bound_pattern_value,
                variant: resolved,
            }
        })
        .collect()
}

/// Effective pattern id at `(section, track)` for the currently-active
/// variant tab. Walks the override chain so the picker shows what the
/// user is actually editing in this variant: variant `Replace` →
/// `Silent` → fall back to `section.base.activations[track].pattern_ref`.
///
/// The base-tab path skips the override lookup (no override should ever
/// be keyed by `section.default_variant`, but we shortcut anyway).
fn effective_pattern_id(
    section: &Section,
    track_id: TrackId,
    variant_id: &VariantId,
) -> Option<PatternId> {
    if section.default_variant != *variant_id
        && let Some(over) = section.variants.get(variant_id)
    {
        match over.activations.get(&track_id) {
            Some(ModelActivationOverride::Replace(entry)) => return entry.pattern_ref,
            // `Silent` overrides base entirely — show "(no pattern)"
            // in the picker so picking a pattern re-Replaces.
            Some(ModelActivationOverride::Silent) => return None,
            None => {}
        }
    }
    section
        .base
        .activations
        .get(&track_id)
        .and_then(|e| e.pattern_ref)
}

fn track_role_label(kind: &ModelTrackKind) -> String {
    match kind {
        ModelTrackKind::Drum { .. } => String::new(),
        ModelTrackKind::Pitched { role } => match role {
            Role::Bass => "bass",
            Role::Voicing => "voicing",
            Role::Arp => "arp",
            Role::Melodic => "melodic",
            Role::Pad => "pad",
            Role::Countermelody => "countermel",
            Role::Other => "other",
        }
        .to_string(),
    }
}

fn resolve_one(
    project: &Project,
    overlay: &ProjectOverlay,
    section: &Section,
    variant_id: &str,
    track_id: TrackId,
) -> ResolvedVariant {
    let variant = VariantId::from(variant_id);

    // Variant override takes precedence over base. SectionVariantOverride.activations
    // is sparse — absence means "inherit base."
    let override_entry = section
        .variants
        .get(&variant)
        .and_then(|v| v.activations.get(&track_id));
    let base = section.base.activations.get(&track_id);

    match (override_entry, base) {
        (None, None) => ResolvedVariant::Inherit {
            reason: "no entry in base",
        },
        (None, Some(entry)) => entry_to_variant(
            project, overlay, section, variant_id, track_id, entry, false, None,
        ),
        (Some(ModelActivationOverride::Silent), Some(base)) => {
            // Silent keeps base's pattern + realization but flips state.
            // We re-use entry_to_variant on the base entry then patch
            // the state to Silent + flag overridden.
            let mut resolved = entry_to_variant(
                project,
                overlay,
                section,
                variant_id,
                track_id,
                base,
                true,
                Some("silenced in this variant".to_string()),
            );
            if let ResolvedVariant::Active { state, .. } = &mut resolved {
                *state = ActivationState::Silent;
            }
            resolved
        }
        (Some(ModelActivationOverride::Silent), None) => ResolvedVariant::Inherit {
            reason: "silent override without base",
        },
        (Some(ModelActivationOverride::Replace(entry)), _) => entry_to_variant(
            project,
            overlay,
            section,
            variant_id,
            track_id,
            entry,
            true,
            Some("replaced in this variant".to_string()),
        ),
    }
}

#[allow(clippy::too_many_arguments)] // intermediate helper inside resolve_one;
                                     // each param is load-bearing and pulling
                                     // them into a struct here would just rename
                                     // the noise.
fn entry_to_variant(
    project: &Project,
    overlay: &ProjectOverlay,
    section: &Section,
    variant_id: &str,
    track_id: TrackId,
    entry: &rawdaw_model::activation::ActivationEntry,
    overridden_by_variant: bool,
    source_label: Option<String>,
) -> ResolvedVariant {
    let pat = entry.pattern_ref.and_then(|pid| project.patterns.get(&pid));
    let pattern_default_variant = pat
        .map(|p| p.default_variant.as_str().to_string())
        .unwrap_or_default();
    let pattern_name = pat.map(|p| p.name.clone()).unwrap_or_default();
    // Pattern color must be a `#RRGGBB` hex literal because the
    // downstream `parts::rgba(hex, alpha)` helper (used by
    // `schedule_column` + the editable schedule cells) `debug_assert!`s
    // that shape. `theme::ACCENT` is a real hex; `theme::TEXT2` is an
    // already-baked rgba string and would crash the debug build when
    // an unbound activation flows through this path.
    let pattern_color = pat
        .and_then(|p| overlay.pattern_color.get(&p.id).cloned())
        .unwrap_or_else(|| theme::ACCENT.to_string());
    let pattern_kind = pat
        .map(|p| match &p.body {
            PatternBody::Pitched(_) => "Pitched".to_string(),
            PatternBody::Drum(_) => "Drum".to_string(),
        })
        .unwrap_or_default();
    let state = if entry.pattern_ref.is_some() {
        ActivationState::Active
    } else {
        ActivationState::Silent
    };
    let realization = overlay
        .lookup_cell(section.id, variant_id, track_id)
        .map(|c| c.realization);
    let schedule = entry
        .variant_schedule
        .iter()
        .map(|(range, vid)| ScheduleEntry {
            start_bar: range.start,
            end_bar: range.end,
            variant: if vid.as_str() == SILENT_VARIANT_SENTINEL {
                None
            } else {
                Some(vid.as_str().to_string())
            },
        })
        .collect();

    ResolvedVariant::Active {
        pattern_name,
        pattern_color,
        pattern_kind,
        pattern_default_variant,
        state,
        overridden_by_variant,
        source_label,
        realization,
        schedule,
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
            realization,
            schedule,
        } => {
            let total_bars = slot.total_bars;
            rsx! {
                ActivationCell {
                    section_id: slot.section_id,
                    track_id: slot.track_id,
                    bound_pattern_value: slot.bound_pattern_value,
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
                section_id: slot.section_id,
                track_id: slot.track_id,
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
    use rawdaw_model::fixtures::build_round1_project;

    fn round1_setup() -> (Project, ProjectOverlay, Section, TrackId, TrackId, TrackId) {
        let (project, keys) = build_round1_project();
        let overlay = crate::initial_project::overlay::build_round1_overlay(&keys);
        let section = project
            .sections
            .get(&keys.sections.verse)
            .cloned()
            .expect("verse exists");
        (
            project,
            overlay,
            section,
            keys.tracks.pad,
            keys.tracks.bass,
            keys.tracks.lead,
        )
    }

    #[test]
    fn pad_in_verse_base_resolves_to_inherit() {
        // Per round-2 decision 14: verse@base has no pad entry.
        let (project, overlay, section, pad, _bass, _lead) = round1_setup();
        let resolved = resolve_one(&project, &overlay, &section, "base", pad);
        match resolved {
            ResolvedVariant::Inherit { reason } => {
                assert_eq!(reason, "no entry in base");
            }
            _ => panic!("expected Inherit, got Active"),
        }
    }

    #[test]
    fn bass_in_verse_stripped_resolves_to_silent_variant_override() {
        // Per round-2 decision 20: verse-stripped overrides bass with Silent.
        let (project, overlay, section, _pad, bass, _lead) = round1_setup();
        let resolved = resolve_one(&project, &overlay, &section, "stripped", bass);
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
        let (project, overlay, section, _pad, _bass, lead) = round1_setup();
        let resolved = resolve_one(&project, &overlay, &section, "stripped", lead);
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
        let (project, overlay, section, _pad, _bass, _lead) = round1_setup();
        let drums = project
            .tracks
            .iter()
            .find(|t| t.name == "drums")
            .expect("drums track")
            .id;
        let resolved = resolve_one(&project, &overlay, &section, "base", drums);
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
