//! Duration + scale-override mutations for a section.
//!
//! The base body owns the canonical values; each variant override
//! either inherits (field is `None`) or replaces (`Some(_)`). The
//! mutators in this module dispatch on `variant == default_variant`:
//! - Base path: mutate `section.base` directly.
//! - Variant path: auto-promote a `SectionVariantOverride` if missing,
//!   then write the field. See S0 decision 3 of
//!   `docs/section-arrangement-editing-plan.md`.
//!
//! Symmetric clear helpers exist so the UI can offer a "match base"
//! / "inherit" affordance per override field without forcing the
//! caller to know about the `None`-vs-`Some(None)` shape of
//! `SectionVariantOverride::scale_override`.

use rawdaw_model::id::{SectionId, VariantId};
use rawdaw_model::project::Project;
use rawdaw_model::scale::Scale;
use rawdaw_model::section::SectionVariantOverride;

/// Set the `duration_bars` field on the section under the given
/// variant. When `variant == section.default_variant`, writes through
/// to `section.base.duration_bars` (the canonical value). Otherwise,
/// writes the variant override's `duration_bars: Some(bars)`, auto-
/// promoting the override entry if it didn't exist yet.
///
/// Silently no-ops if `id` doesn't exist — UI handlers can't safely
/// panic on a missing id (delete/edit race).
pub fn set_section_duration_bars(
    project: &mut Project,
    id: SectionId,
    variant: &VariantId,
    bars: u32,
) {
    let Some(section) = project.sections.get_mut(&id) else {
        return;
    };
    if variant == &section.default_variant {
        section.base.duration_bars = bars;
        return;
    }
    section
        .variants
        .entry(variant.clone())
        .or_default()
        .duration_bars = Some(bars);
}

/// Set the `scale_override` field on the section under the given
/// variant. On the default-variant path, writes `Some(scale)` /
/// `None` directly into `section.base.scale_override`. On the non-
/// default variant path, writes the override field as
/// `Some(Some(scale))` (set) or `Some(None)` (explicitly clear the
/// base's scale override under this variant), with auto-promotion of
/// the override entry. Use [`clear_variant_scale_override`] to
/// inherit instead (sets the override field back to `None`).
///
/// Silently no-ops if `id` doesn't exist.
pub fn set_section_scale_override(
    project: &mut Project,
    id: SectionId,
    variant: &VariantId,
    scale: Option<Scale>,
) {
    let Some(section) = project.sections.get_mut(&id) else {
        return;
    };
    if variant == &section.default_variant {
        section.base.scale_override = scale;
        return;
    }
    section
        .variants
        .entry(variant.clone())
        .or_default()
        .scale_override = Some(scale);
}

/// Clear the `duration_bars` override on a non-default variant so it
/// inherits the base's duration. No-op on the default variant (which
/// stores the canonical value in `section.base.duration_bars` — there
/// is no override to clear). No-op if `id` or the variant entry
/// doesn't exist.
///
/// If the override entry becomes "all-None" after the clear, the entry
/// itself is left in place; pruning empty overrides is a UI/UX
/// decision deferred to S3 (it doesn't change semantics — an empty
/// override inherits the same way an absent one does).
pub fn clear_variant_duration_override(
    project: &mut Project,
    id: SectionId,
    variant: &VariantId,
) {
    let Some(section) = project.sections.get_mut(&id) else {
        return;
    };
    if variant == &section.default_variant {
        return;
    }
    if let Some(o) = section.variants.get_mut(variant) {
        o.duration_bars = None;
    }
}

/// Clear the `scale_override` field on a non-default variant so it
/// inherits the base's setting. No-op on the default variant or
/// missing ids / variants. Symmetric with
/// [`clear_variant_duration_override`].
pub fn clear_variant_scale_override(project: &mut Project, id: SectionId, variant: &VariantId) {
    let Some(section) = project.sections.get_mut(&id) else {
        return;
    };
    if variant == &section.default_variant {
        return;
    }
    if let Some(o) = section.variants.get_mut(variant) {
        o.scale_override = None;
    }
}

// `SectionVariantOverride::default()` is what `or_default` resolves
// to above. Bring it into scope through the dependency so a future
// refactor of the helper surfaces a compile error if the impl moves.
#[allow(dead_code)]
fn _assert_default_impl() {
    let _ = SectionVariantOverride::default();
}

#[cfg(test)]
mod tests {
    use super::super::test_support::empty_project;
    use super::super::{create_section, set_section_color};
    use super::*;

    use rawdaw_model::pitch::PitchClass;
    use rawdaw_model::scale::Scale;
    use rawdaw_model::section::SectionVariantOverride;

    fn project_with_section() -> (rawdaw_model::project::Project, SectionId) {
        let mut p = empty_project();
        let sid = create_section(&mut p);
        (p, sid)
    }

    // ---------- duration ----------

    #[test]
    fn set_duration_on_default_writes_base() {
        let (mut p, sid) = project_with_section();
        set_section_duration_bars(&mut p, sid, &VariantId::base(), 8);
        let section = p.sections.get(&sid).unwrap();
        assert_eq!(section.base.duration_bars, 8);
        assert!(
            section.variants.is_empty(),
            "default-variant edit must not create a variant override",
        );
    }

    #[test]
    fn set_duration_on_non_default_creates_override() {
        let (mut p, sid) = project_with_section();
        let fill = VariantId::new("fill");
        set_section_duration_bars(&mut p, sid, &fill, 12);
        let section = p.sections.get(&sid).unwrap();
        assert_eq!(
            section.base.duration_bars,
            super::super::DEFAULT_SECTION_BARS,
            "base must remain untouched when editing a non-default variant",
        );
        let override_ = section.variants.get(&fill).unwrap();
        assert_eq!(override_.duration_bars, Some(12));
        assert!(override_.scale_override.is_none());
        assert!(override_.chord_loops.is_none());
        assert!(override_.activations.is_empty());
    }

    #[test]
    fn set_duration_on_existing_override_mutates_in_place() {
        let (mut p, sid) = project_with_section();
        let fill = VariantId::new("fill");
        // Pre-seed an override with a scale set so we can confirm the
        // duration edit doesn't clobber sibling fields.
        let scale = Scale::major(PitchClass::G);
        {
            let section = p.sections.get_mut(&sid).unwrap();
            section.variants.insert(
                fill.clone(),
                SectionVariantOverride {
                    duration_bars: Some(2),
                    scale_override: Some(Some(scale.clone())),
                    chord_loops: None,
                    activations: Default::default(),
                },
            );
        }
        set_section_duration_bars(&mut p, sid, &fill, 16);
        let section = p.sections.get(&sid).unwrap();
        let o = section.variants.get(&fill).unwrap();
        assert_eq!(o.duration_bars, Some(16));
        assert_eq!(o.scale_override, Some(Some(scale)));
    }

    #[test]
    fn set_duration_missing_id_is_noop() {
        let mut p = empty_project();
        set_section_duration_bars(&mut p, SectionId::new(9999), &VariantId::base(), 8);
        assert!(p.sections.is_empty());
    }

    // ---------- scale ----------

    #[test]
    fn set_scale_on_default_writes_base() {
        let (mut p, sid) = project_with_section();
        let scale = Scale::major(PitchClass::F);
        set_section_scale_override(&mut p, sid, &VariantId::base(), Some(scale.clone()));
        let section = p.sections.get(&sid).unwrap();
        assert_eq!(section.base.scale_override, Some(scale));
    }

    #[test]
    fn set_scale_some_some_on_non_default_creates_override() {
        let (mut p, sid) = project_with_section();
        let fill = VariantId::new("fill");
        let scale = Scale::major(PitchClass::G);
        set_section_scale_override(&mut p, sid, &fill, Some(scale.clone()));
        let section = p.sections.get(&sid).unwrap();
        let o = section.variants.get(&fill).unwrap();
        assert_eq!(o.scale_override, Some(Some(scale)));
    }

    #[test]
    fn set_scale_none_on_non_default_writes_explicit_clear() {
        // Per `SectionVariantOverride` docs: `Some(None)` explicitly
        // clears the base's scale override under this variant.
        // Distinguished from `None` (inherit).
        let (mut p, sid) = project_with_section();
        let fill = VariantId::new("fill");
        set_section_scale_override(&mut p, sid, &fill, None);
        let section = p.sections.get(&sid).unwrap();
        let o = section.variants.get(&fill).unwrap();
        assert_eq!(o.scale_override, Some(None));
    }

    // ---------- clear ----------

    #[test]
    fn clear_duration_on_non_default_drops_override_field_to_none() {
        let (mut p, sid) = project_with_section();
        let fill = VariantId::new("fill");
        set_section_duration_bars(&mut p, sid, &fill, 12);
        clear_variant_duration_override(&mut p, sid, &fill);
        let section = p.sections.get(&sid).unwrap();
        // Entry still exists; the field is now None (inherits base).
        let o = section.variants.get(&fill).unwrap();
        assert!(o.duration_bars.is_none());
    }

    #[test]
    fn clear_duration_on_default_is_noop() {
        let (mut p, sid) = project_with_section();
        let before = p.sections.get(&sid).unwrap().clone();
        clear_variant_duration_override(&mut p, sid, &VariantId::base());
        assert_eq!(p.sections.get(&sid).unwrap(), &before);
    }

    #[test]
    fn clear_scale_on_non_default_drops_override_field_to_none() {
        let (mut p, sid) = project_with_section();
        let fill = VariantId::new("fill");
        let scale = Scale::major(PitchClass::F);
        set_section_scale_override(&mut p, sid, &fill, Some(scale));
        clear_variant_scale_override(&mut p, sid, &fill);
        let o = p.sections.get(&sid).unwrap().variants.get(&fill).unwrap();
        assert!(o.scale_override.is_none());
    }

    #[test]
    fn clear_scale_on_default_is_noop() {
        let (mut p, sid) = project_with_section();
        let before = p.sections.get(&sid).unwrap().clone();
        clear_variant_scale_override(&mut p, sid, &VariantId::base());
        assert_eq!(p.sections.get(&sid).unwrap(), &before);
    }

    #[test]
    fn clear_missing_variant_entry_is_noop() {
        let (mut p, sid) = project_with_section();
        let fill = VariantId::new("fill");
        let before = p.sections.get(&sid).unwrap().clone();
        clear_variant_duration_override(&mut p, sid, &fill);
        clear_variant_scale_override(&mut p, sid, &fill);
        // No variant entry was created.
        assert_eq!(p.sections.get(&sid).unwrap(), &before);
    }

    #[test]
    fn cross_helpers_compose_cleanly() {
        // Sanity check: a typical UI flow (color → duration → scale →
        // clear) doesn't leave the section in an unexpected state.
        let (mut p, sid) = project_with_section();
        let fill = VariantId::new("fill");
        let mut overlay = crate::overlay::ProjectOverlay::default();
        set_section_color(&mut overlay, sid, "#112233".into());
        set_section_duration_bars(&mut p, sid, &fill, 6);
        let scale = Scale::major(PitchClass::F);
        set_section_scale_override(&mut p, sid, &fill, Some(scale.clone()));
        clear_variant_duration_override(&mut p, sid, &fill);
        let section = p.sections.get(&sid).unwrap();
        let o = section.variants.get(&fill).unwrap();
        assert!(o.duration_bars.is_none());
        assert_eq!(o.scale_override, Some(Some(scale)));
        assert_eq!(overlay.section_color.get(&sid).unwrap(), "#112233");
    }
}
