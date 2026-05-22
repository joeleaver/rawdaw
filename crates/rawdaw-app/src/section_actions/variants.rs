//! Variant CRUD on a section.
//!
//! **Phase state.** Add / remove / rename / set-default all ship in
//! S1 but have no UI consumer until S3's variant tab strip work.
//! The `#![allow(dead_code)]` comes off when S3 wires the tab
//! affordances.
#![allow(dead_code)]

//!
//! Variants are sparse overrides on top of the base body (see
//! `rawdaw_model::section::SectionVariantOverride`). Each section has
//! a `BTreeMap<VariantId, SectionVariantOverride>` and a
//! `default_variant: VariantId` that points either at base (the
//! canonical `VariantId::base()`) or at one of the override entries.
//!
//! S3's variant tab strip drives these mutators. Errors flow back
//! as typed enums so the UI can surface useful messages — the toast
//! primitive lands later; for now the tab handlers `eprintln!` the
//! error.

use rawdaw_model::id::{SectionId, VariantId};
use rawdaw_model::project::Project;
use rawdaw_model::section::SectionVariantOverride;

/// Add a new (empty) variant override to `id`. The new override has
/// every field set to `None` / empty, so realization inherits from
/// the base body until the user edits it. Returns `Ok` with the
/// chosen `VariantId` (which may be suffix-bumped if `name` collided)
/// or `VariantConflict::NotFound` if the section id doesn't exist.
///
/// Calling with `name == VariantId::base()` or `name` equal to any
/// existing override silently suffix-bumps (matches the
/// `unique_pattern_name` precedent in
/// `pattern_actions::create_variant`). The Library / section editor
/// is expected to inline-rename right after creation, so the suffix
/// only persists if the user accepts it.
pub fn add_section_variant(
    project: &mut Project,
    id: SectionId,
    name: &str,
) -> Result<VariantId, VariantConflict> {
    let section = project.sections.get_mut(&id).ok_or(VariantConflict::NotFound)?;
    let chosen = unique_variant_name(section, name);
    section.variants.insert(chosen.clone(), SectionVariantOverride::default());
    Ok(chosen)
}

/// Remove a variant override from `id`. Refuses
/// ([`RemoveVariantError::DefaultVariant`]) when `variant` is the
/// section's current default — the user must change the default
/// first. Refuses
/// ([`RemoveVariantError::ReferencedBy`]) when any arrangement step
/// targets this variant on this section — same shape as
/// `delete_section`'s `DeleteRefused::ReferencedBy`.
///
/// Returns `RemoveVariantError::NotFound` if the section id or
/// variant entry doesn't exist.
pub fn remove_section_variant(
    project: &mut Project,
    id: SectionId,
    variant: &VariantId,
) -> Result<(), RemoveVariantError> {
    let section = project
        .sections
        .get(&id)
        .ok_or(RemoveVariantError::NotFound)?;
    if &section.default_variant == variant {
        return Err(RemoveVariantError::DefaultVariant);
    }
    if !section.variants.contains_key(variant) {
        return Err(RemoveVariantError::NotFound);
    }
    let referenced = arrangement_steps_for_variant(project, id, variant);
    if !referenced.is_empty() {
        return Err(RemoveVariantError::ReferencedBy(referenced));
    }
    // Re-borrow mutably now that the read-only walk is done.
    let section = project
        .sections
        .get_mut(&id)
        .ok_or(RemoveVariantError::NotFound)?;
    section.variants.remove(variant);
    Ok(())
}

/// Rename a section variant. Atomically rewrites:
/// - the `BTreeMap` key in `section.variants`,
/// - `section.default_variant` if it pointed at `old`,
/// - every `SectionRef.variant` in `project.arrangement.sections`
///   that targeted this section under the old name.
///
/// The atomic rewrite of arrangement steps is critical — without it,
/// a rename would orphan the existing step references and the
/// realization step would fall back to the base body (silent data
/// loss). S0 decision 7 of the plan locks this contract in.
///
/// Refuses with `VariantConflict::NameTaken` on collision with an
/// existing variant (or the default variant if it's not stored in
/// `variants`). `VariantConflict::NotFound` if section / variant
/// doesn't exist.
#[allow(dead_code)] // S3 wires the meta-bar UI consumer.
pub fn rename_section_variant(
    project: &mut Project,
    id: SectionId,
    old: &VariantId,
    new: VariantId,
) -> Result<(), VariantConflict> {
    let section = project
        .sections
        .get_mut(&id)
        .ok_or(VariantConflict::NotFound)?;
    if old == &new {
        return Ok(());
    }
    if section.variants.contains_key(&new) || section.default_variant == new {
        return Err(VariantConflict::NameTaken);
    }
    let Some(payload) = section.variants.remove(old) else {
        return Err(VariantConflict::NotFound);
    };
    section.variants.insert(new.clone(), payload);
    if &section.default_variant == old {
        section.default_variant = new.clone();
    }
    // Rewrite arrangement-step references.
    for step in project.arrangement.sections.iter_mut() {
        if step.section == id && &step.variant == old {
            step.variant = new.clone();
        }
    }
    Ok(())
}

/// Set `id`'s default variant. The new default must be either
/// `VariantId::base()` or a key that already exists in
/// `section.variants`. Returns `VariantConflict::NotFound` if the
/// section doesn't exist or the target variant isn't valid.
pub fn set_default_variant(
    project: &mut Project,
    id: SectionId,
    variant: &VariantId,
) -> Result<(), VariantConflict> {
    let section = project
        .sections
        .get_mut(&id)
        .ok_or(VariantConflict::NotFound)?;
    if variant != &VariantId::base() && !section.variants.contains_key(variant) {
        return Err(VariantConflict::NotFound);
    }
    section.default_variant = variant.clone();
    Ok(())
}

/// Mutation outcomes for [`add_section_variant`] /
/// [`rename_section_variant`] / [`set_default_variant`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantConflict {
    /// The section or source variant didn't exist.
    NotFound,
    /// Renaming would collide with an existing variant (or with the
    /// default variant when the default is `VariantId::base()`).
    NameTaken,
}

/// Mutation outcome for [`remove_section_variant`]. Wider than
/// [`VariantConflict`] because removing also has to defend against
/// default-variant and arrangement-step references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveVariantError {
    /// Section or variant entry didn't exist.
    NotFound,
    /// The variant is `section.default_variant`; UI must change the
    /// default before removing.
    DefaultVariant,
    /// One or more arrangement steps target this section under this
    /// variant. Sorted, deduplicated step indices.
    ReferencedBy(Vec<usize>),
}

fn arrangement_steps_for_variant(
    project: &Project,
    id: SectionId,
    variant: &VariantId,
) -> Vec<usize> {
    project
        .arrangement
        .sections
        .iter()
        .enumerate()
        .filter_map(|(idx, step)| (step.section == id && &step.variant == variant).then_some(idx))
        .collect()
}

fn unique_variant_name(
    section: &rawdaw_model::section::Section,
    seed: &str,
) -> VariantId {
    let candidate_taken = |name: &str| -> bool {
        if section.default_variant == VariantId::new(name) {
            return true;
        }
        section.variants.contains_key(&VariantId::new(name))
    };
    if !candidate_taken(seed) {
        return VariantId::new(seed.to_string());
    }
    for n in 2u32..u32::MAX {
        let candidate = format!("{seed}-{n}");
        if !candidate_taken(&candidate) {
            return VariantId::new(candidate);
        }
    }
    VariantId::new(format!("{seed}-overflow"))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{empty_project, project_with_arrangement_step};
    use super::super::create_section;
    use super::*;

    fn project_with_section() -> (rawdaw_model::project::Project, SectionId) {
        let mut p = empty_project();
        let sid = create_section(&mut p);
        (p, sid)
    }

    // ---------- add_section_variant ----------

    #[test]
    fn add_variant_inserts_empty_override() {
        let (mut p, sid) = project_with_section();
        let v = add_section_variant(&mut p, sid, "fill").unwrap();
        assert_eq!(v, VariantId::new("fill"));
        let section = p.sections.get(&sid).unwrap();
        let o = section.variants.get(&v).unwrap();
        assert_eq!(o, &SectionVariantOverride::default());
        // Default variant unchanged.
        assert_eq!(section.default_variant, VariantId::base());
    }

    #[test]
    fn add_variant_suffixes_on_collision() {
        let (mut p, sid) = project_with_section();
        let _ = add_section_variant(&mut p, sid, "fill").unwrap();
        let v2 = add_section_variant(&mut p, sid, "fill").unwrap();
        assert_eq!(v2, VariantId::new("fill-2"));
    }

    #[test]
    fn add_variant_suffixes_when_colliding_with_default() {
        // `default_variant` may be `VariantId::base()` (which isn't a
        // key in `variants` for a fresh section) — the suffix logic
        // must still treat it as taken.
        let (mut p, sid) = project_with_section();
        let v = add_section_variant(&mut p, sid, "base").unwrap();
        assert_eq!(v, VariantId::new("base-2"));
    }

    #[test]
    fn add_variant_missing_section_returns_not_found() {
        let mut p = empty_project();
        assert_eq!(
            add_section_variant(&mut p, SectionId::new(0), "fill"),
            Err(VariantConflict::NotFound),
        );
    }

    // ---------- remove_section_variant ----------

    #[test]
    fn remove_variant_drops_entry() {
        let (mut p, sid) = project_with_section();
        let v = add_section_variant(&mut p, sid, "fill").unwrap();
        remove_section_variant(&mut p, sid, &v).unwrap();
        assert!(p.sections.get(&sid).unwrap().variants.is_empty());
    }

    #[test]
    fn remove_variant_refuses_default() {
        let (mut p, sid) = project_with_section();
        // The fresh section's default is `base`, which isn't in
        // `variants`. Promote a real variant to default first so we
        // can hit the DefaultVariant branch unambiguously.
        let v = add_section_variant(&mut p, sid, "fill").unwrap();
        set_default_variant(&mut p, sid, &v).unwrap();
        let err = remove_section_variant(&mut p, sid, &v).unwrap_err();
        assert_eq!(err, RemoveVariantError::DefaultVariant);
        // Variant still present after the refusal.
        assert!(p.sections.get(&sid).unwrap().variants.contains_key(&v));
    }

    #[test]
    fn remove_variant_refuses_when_arrangement_references_it() {
        let (mut p, sid) = project_with_section();
        let v = add_section_variant(&mut p, sid, "fill").unwrap();
        // Place a step targeting this section under the default
        // variant, then re-target it to the new variant directly so
        // we don't need the S4 actions module yet.
        project_with_arrangement_step(&mut p, sid);
        p.arrangement.sections[0].variant = v.clone();

        let err = remove_section_variant(&mut p, sid, &v).unwrap_err();
        assert_eq!(err, RemoveVariantError::ReferencedBy(vec![0]));
        assert!(p.sections.get(&sid).unwrap().variants.contains_key(&v));
    }

    #[test]
    fn remove_missing_variant_returns_not_found() {
        let (mut p, sid) = project_with_section();
        let err = remove_section_variant(&mut p, sid, &VariantId::new("ghost")).unwrap_err();
        assert_eq!(err, RemoveVariantError::NotFound);
    }

    #[test]
    fn remove_missing_section_returns_not_found() {
        let mut p = empty_project();
        let err = remove_section_variant(&mut p, SectionId::new(0), &VariantId::new("ghost"))
            .unwrap_err();
        assert_eq!(err, RemoveVariantError::NotFound);
    }

    #[test]
    fn remove_variant_does_not_walk_other_sections_arrangement_steps() {
        // Defends against a too-broad walker: a step referencing
        // section A under variant `v` must NOT count when removing
        // variant `v` from section B.
        let (mut p, sid_a) = project_with_section();
        let sid_b = create_section(&mut p);
        let v = add_section_variant(&mut p, sid_b, "fill").unwrap();
        // Add a step on section A under variant "fill" (same VariantId
        // string, different section). Removing it from B should
        // succeed.
        project_with_arrangement_step(&mut p, sid_a);
        p.arrangement.sections[0].variant = v.clone();

        remove_section_variant(&mut p, sid_b, &v).unwrap();
        assert!(p.sections.get(&sid_b).unwrap().variants.is_empty());
        assert!(!p.arrangement.sections.is_empty()); // step still present
    }

    // ---------- rename_section_variant ----------

    #[test]
    fn rename_variant_moves_entry_and_rewrites_arrangement() {
        let (mut p, sid) = project_with_section();
        let v = add_section_variant(&mut p, sid, "fill").unwrap();
        project_with_arrangement_step(&mut p, sid);
        p.arrangement.sections[0].variant = v.clone();

        rename_section_variant(&mut p, sid, &v, VariantId::new("buildup")).unwrap();
        let section = p.sections.get(&sid).unwrap();
        assert!(!section.variants.contains_key(&v));
        assert!(section.variants.contains_key(&VariantId::new("buildup")));
        // The arrangement step's variant must have followed the
        // rename — otherwise the rename would silently orphan it.
        assert_eq!(p.arrangement.sections[0].variant, VariantId::new("buildup"));
    }

    #[test]
    fn rename_variant_rewrites_default_when_it_pointed_at_old() {
        let (mut p, sid) = project_with_section();
        let v = add_section_variant(&mut p, sid, "fill").unwrap();
        set_default_variant(&mut p, sid, &v).unwrap();
        rename_section_variant(&mut p, sid, &v, VariantId::new("renamed")).unwrap();
        assert_eq!(
            p.sections.get(&sid).unwrap().default_variant,
            VariantId::new("renamed"),
        );
    }

    #[test]
    fn rename_variant_refuses_collision() {
        let (mut p, sid) = project_with_section();
        let _ = add_section_variant(&mut p, sid, "fill").unwrap();
        let other = add_section_variant(&mut p, sid, "build").unwrap();
        let err =
            rename_section_variant(&mut p, sid, &other, VariantId::new("fill")).unwrap_err();
        assert_eq!(err, VariantConflict::NameTaken);
    }

    #[test]
    fn rename_variant_refuses_collision_with_default_base() {
        // The default `VariantId::base()` isn't a key in `variants`;
        // a rename targeting "base" must still be refused.
        let (mut p, sid) = project_with_section();
        let v = add_section_variant(&mut p, sid, "fill").unwrap();
        let err = rename_section_variant(&mut p, sid, &v, VariantId::base()).unwrap_err();
        assert_eq!(err, VariantConflict::NameTaken);
    }

    #[test]
    fn rename_variant_to_same_name_is_noop_ok() {
        let (mut p, sid) = project_with_section();
        let v = add_section_variant(&mut p, sid, "fill").unwrap();
        rename_section_variant(&mut p, sid, &v, v.clone()).unwrap();
        assert!(p.sections.get(&sid).unwrap().variants.contains_key(&v));
    }

    #[test]
    fn rename_missing_variant_returns_not_found() {
        let (mut p, sid) = project_with_section();
        let err = rename_section_variant(
            &mut p,
            sid,
            &VariantId::new("ghost"),
            VariantId::new("buildup"),
        )
        .unwrap_err();
        assert_eq!(err, VariantConflict::NotFound);
    }

    // ---------- set_default_variant ----------

    #[test]
    fn set_default_accepts_existing_variant() {
        let (mut p, sid) = project_with_section();
        let v = add_section_variant(&mut p, sid, "fill").unwrap();
        set_default_variant(&mut p, sid, &v).unwrap();
        assert_eq!(p.sections.get(&sid).unwrap().default_variant, v);
    }

    #[test]
    fn set_default_accepts_base_even_without_variants() {
        // `VariantId::base()` is always valid; it points at the base
        // body which doesn't require an entry in `variants`.
        let (mut p, sid) = project_with_section();
        // Move default away from base, then back.
        let v = add_section_variant(&mut p, sid, "fill").unwrap();
        set_default_variant(&mut p, sid, &v).unwrap();
        set_default_variant(&mut p, sid, &VariantId::base()).unwrap();
        assert_eq!(p.sections.get(&sid).unwrap().default_variant, VariantId::base());
    }

    #[test]
    fn set_default_refuses_unknown_variant() {
        let (mut p, sid) = project_with_section();
        let err = set_default_variant(&mut p, sid, &VariantId::new("ghost")).unwrap_err();
        assert_eq!(err, VariantConflict::NotFound);
    }

    #[test]
    fn set_default_missing_section_returns_not_found() {
        let mut p = empty_project();
        let err =
            set_default_variant(&mut p, SectionId::new(0), &VariantId::base()).unwrap_err();
        assert_eq!(err, VariantConflict::NotFound);
    }
}
