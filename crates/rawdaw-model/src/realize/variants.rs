//! Variant-override resolution.
//!
//! Section variants are sparse overrides on top of the section's `base`. These
//! helpers compute the *effective* value of each overridable field for a
//! given (section, variant) pair, falling back to the base — or to the
//! project default — as appropriate.
//!
//! All functions here are pure; the realization walker calls them per
//! `SectionRef` placement.

use std::collections::BTreeMap;

use crate::activation::ActivationEntry;
use crate::id::{ChordLoopId, TrackId, VariantId};
use crate::project::Project;
use crate::scale::Scale;
use crate::section::{ActivationOverride, Section};
use crate::time::BarRange;
use crate::track::{Role, Track, TrackKind};

/// Effective scale for `(section, variant)`. The merge rules from
/// `section-variants.md`:
///
/// - Variant `scale_override == None`: inherit base.
/// - Variant `scale_override == Some(None)`: explicitly clear back to project default.
/// - Variant `scale_override == Some(Some(s))`: use `s`.
/// - Otherwise the base's `scale_override` is consulted, falling back to project default.
pub(super) fn effective_scale<'a>(
    project: &'a Project,
    section: &'a Section,
    variant: &VariantId,
) -> &'a Scale {
    if let Some(v) = section.variants.get(variant) {
        match &v.scale_override {
            Some(Some(s)) => return s,
            Some(None) => return &project.default_key,
            None => {}
        }
    }
    section
        .base
        .scale_override
        .as_ref()
        .unwrap_or(&project.default_key)
}

/// Effective chord-loop schedule for `(section, variant)`. A variant either
/// inherits the base's `chord_loops` (when `None`) or replaces it wholesale.
pub(super) fn effective_chord_loops(
    section: &Section,
    variant: &VariantId,
) -> Vec<(BarRange, ChordLoopId)> {
    if let Some(v) = section.variants.get(variant)
        && let Some(loops) = &v.chord_loops
    {
        return loops.clone();
    }
    section.base.chord_loops.clone()
}

/// Effective duration in bars for `(section, variant)`. Variants may override
/// the base's `duration_bars`.
pub(super) fn effective_duration_bars(section: &Section, variant: &VariantId) -> u32 {
    if let Some(v) = section.variants.get(variant)
        && let Some(d) = v.duration_bars
    {
        return d;
    }
    section.base.duration_bars
}

/// Effective per-track activations for `(section, variant)`. Returns a map
/// keyed by `TrackId` where the value is a borrow of the activation that
/// actually plays in this variant — either the base's, or a variant's
/// `Replace`. Variant `Silent` removes the entry; absent variant keys
/// inherit the base.
pub(super) fn effective_activations<'a>(
    section: &'a Section,
    variant: &'a VariantId,
) -> BTreeMap<TrackId, &'a ActivationEntry> {
    let mut out: BTreeMap<TrackId, &ActivationEntry> = BTreeMap::new();
    for (tid, act) in &section.base.activations {
        out.insert(*tid, act);
    }
    if let Some(v) = section.variants.get(variant) {
        for (tid, ov) in &v.activations {
            match ov {
                ActivationOverride::Silent => {
                    out.remove(tid);
                }
                ActivationOverride::Replace(act) => {
                    out.insert(*tid, act);
                }
            }
        }
    }
    out
}

/// Which pattern variant plays at bar `bar` within an activation. Falls back
/// to the pattern's `default_variant` when the bar is not covered by the
/// activation's `variant_schedule`.
pub(super) fn variant_at_bar<'a>(
    activation: &'a ActivationEntry,
    bar: u32,
    default: &'a VariantId,
) -> &'a VariantId {
    for (range, variant) in &activation.variant_schedule {
        if range.contains(bar) {
            return variant;
        }
    }
    default
}

/// The compositional role of a track, used to anchor `OctaveSpec::RelativeToRole`
/// and `OctaveSpec::Nearest` when no prior pitch exists. Drum tracks return
/// `Role::Other`; their pitch resolution doesn't use role registers.
pub(super) fn pitched_track_role(track: &Track) -> Role {
    match &track.kind {
        TrackKind::Pitched { role } => *role,
        TrackKind::Drum { .. } => Role::Other,
    }
}
