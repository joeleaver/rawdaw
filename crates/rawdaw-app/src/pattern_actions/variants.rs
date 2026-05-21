//! Variant CRUD on a pattern (works for both pitched and drum bodies).
//!
//! Variant tabs are shared at the pattern editor's top per P2 design
//! decision 5; the four mutations here back those tabs (create, rename,
//! delete, duplicate). Errors flow back as [`VariantEditError`] so the
//! UI can surface a useful message (toast primitive lands later — for
//! now the tab handlers `eprintln!` the error).

use rawdaw_model::id::{PatternId, VariantId};
use rawdaw_model::pattern::{DrumEvent, Pattern, PatternBody, PitchedEvent};
use rawdaw_model::project::Project;

/// Create a new variant in `pattern_id`. The new variant's body is
/// empty. Returns the chosen `VariantId` (which may differ from the
/// caller's request if it collided with an existing variant — we
/// suffix `-2`, `-3`, … to keep names unique inside one pattern).
/// Returns `None` if `pattern_id` doesn't exist.
///
/// The mutation works for both pitched and drum bodies (shared
/// variant tabs per P2 design decision 5).
pub fn create_variant(
    project: &mut Project,
    pattern_id: PatternId,
    seed: &str,
) -> Option<VariantId> {
    let pattern = project.patterns.get_mut(&pattern_id)?;
    let chosen = unique_variant_name(pattern, seed);
    match &mut pattern.body {
        PatternBody::Pitched(body) => {
            body.variants.insert(chosen.clone(), Vec::new());
        }
        PatternBody::Drum(body) => {
            body.variants.insert(chosen.clone(), Vec::new());
        }
    }
    Some(chosen)
}

/// Rename `old` to `new` within `pattern_id`. Refuses (returns
/// `Err(VariantEditError::NameTaken)`) if `new` already exists in
/// the pattern; returns `Err(VariantEditError::NotFound)` if either
/// the pattern or `old` doesn't exist. On success, also rewrites
/// `pattern.default_variant` if it pointed at `old`.
///
/// Activation `variant_schedule` entries pointing at `old` are NOT
/// rewritten here — they'd need a project-wide walk and the typical
/// rename in v1 is "create + delete," not in-place. Variant
/// scheduling lands in P4; this helper preserves the contract for
/// the editor side.
// Used by the variant tab strip's rename affordance, which lands
// alongside the in-place tab inline-edit in a polish pass. Kept now
// so the P1+P2 model surface is complete; allow(dead_code) until the
// UI consumer lands.
#[allow(dead_code)]
pub fn rename_variant(
    project: &mut Project,
    pattern_id: PatternId,
    old: &VariantId,
    new: VariantId,
) -> Result<(), VariantEditError> {
    let pattern = project
        .patterns
        .get_mut(&pattern_id)
        .ok_or(VariantEditError::NotFound)?;
    if old == &new {
        return Ok(());
    }
    if variant_exists(pattern, &new) {
        return Err(VariantEditError::NameTaken);
    }
    let renamed = match &mut pattern.body {
        PatternBody::Pitched(body) => match body.variants.remove(old) {
            Some(events) => {
                body.variants.insert(new.clone(), events);
                true
            }
            None => false,
        },
        PatternBody::Drum(body) => match body.variants.remove(old) {
            Some(events) => {
                body.variants.insert(new.clone(), events);
                true
            }
            None => false,
        },
    };
    if !renamed {
        return Err(VariantEditError::NotFound);
    }
    if &pattern.default_variant == old {
        pattern.default_variant = new;
    }
    Ok(())
}

/// Delete `variant` from `pattern_id`. Refuses
/// ([`VariantEditError::LastVariant`]) if it would leave the pattern
/// with zero variants — a pattern must always have at least one
/// variant body so realization has a well-defined fallback.
///
/// If `variant` is the pattern's `default_variant`, the new default
/// is the first remaining variant in sort order.
pub fn delete_variant(
    project: &mut Project,
    pattern_id: PatternId,
    variant: &VariantId,
) -> Result<(), VariantEditError> {
    let pattern = project
        .patterns
        .get_mut(&pattern_id)
        .ok_or(VariantEditError::NotFound)?;
    let variant_count = match &pattern.body {
        PatternBody::Pitched(body) => body.variants.len(),
        PatternBody::Drum(body) => body.variants.len(),
    };
    if variant_count <= 1 {
        return Err(VariantEditError::LastVariant);
    }
    let removed = match &mut pattern.body {
        PatternBody::Pitched(body) => body.variants.remove(variant).is_some(),
        PatternBody::Drum(body) => body.variants.remove(variant).is_some(),
    };
    if !removed {
        return Err(VariantEditError::NotFound);
    }
    if &pattern.default_variant == variant {
        let next = match &pattern.body {
            PatternBody::Pitched(body) => body.variants.keys().next().cloned(),
            PatternBody::Drum(body) => body.variants.keys().next().cloned(),
        };
        // We just checked variant_count > 1, so at least one variant
        // remains and `next` is Some.
        if let Some(next) = next {
            pattern.default_variant = next;
        }
    }
    Ok(())
}

/// Duplicate `source` inside `pattern_id` to a fresh name. Pitched
/// events get freshly-allocated NoteIds (durability contract from
/// [[project-status]]); drum events likewise. Returns the new
/// variant id or an error if the pattern/variant doesn't exist.
pub fn duplicate_variant(
    project: &mut Project,
    pattern_id: PatternId,
    source: &VariantId,
) -> Result<VariantId, VariantEditError> {
    // Clone the source events first so the id allocator's mutable
    // borrow doesn't overlap with the pattern body's.
    let cloned: ClonedVariant = {
        let pattern = project
            .patterns
            .get(&pattern_id)
            .ok_or(VariantEditError::NotFound)?;
        match &pattern.body {
            PatternBody::Pitched(body) => {
                let events = body
                    .variants
                    .get(source)
                    .ok_or(VariantEditError::NotFound)?
                    .clone();
                ClonedVariant::Pitched(events)
            }
            PatternBody::Drum(body) => {
                let events = body
                    .variants
                    .get(source)
                    .ok_or(VariantEditError::NotFound)?
                    .clone();
                ClonedVariant::Drum(events)
            }
        }
    };
    let cloned = match cloned {
        ClonedVariant::Pitched(events) => {
            let reissued: Vec<PitchedEvent> = events
                .into_iter()
                .map(|mut e| {
                    e.note_id = project.id_allocators.alloc_note();
                    e
                })
                .collect();
            ClonedVariant::Pitched(reissued)
        }
        ClonedVariant::Drum(events) => {
            let reissued = events
                .into_iter()
                .map(|mut e| {
                    e.note_id = project.id_allocators.alloc_note();
                    e
                })
                .collect();
            ClonedVariant::Drum(reissued)
        }
    };
    let pattern = project
        .patterns
        .get_mut(&pattern_id)
        .ok_or(VariantEditError::NotFound)?;
    let seed = format!("{} copy", source.as_str());
    let new_id = unique_variant_name(pattern, &seed);
    match (cloned, &mut pattern.body) {
        (ClonedVariant::Pitched(events), PatternBody::Pitched(body)) => {
            body.variants.insert(new_id.clone(), events);
            Ok(new_id)
        }
        (ClonedVariant::Drum(events), PatternBody::Drum(body)) => {
            body.variants.insert(new_id.clone(), events);
            Ok(new_id)
        }
        _ => Err(VariantEditError::NotFound),
    }
}

/// Mutation outcomes for [`rename_variant`] / [`delete_variant`] /
/// [`duplicate_variant`]. UI handlers branch on these to render an
/// error toast (or eprintln stub until the toast primitive lands).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantEditError {
    /// The target pattern or source variant didn't exist.
    NotFound,
    /// Deleting would leave the pattern without any variant body.
    LastVariant,
    /// Renaming would collide with an existing variant in the same
    /// pattern. The UI should reject the rename, not silently
    /// overwrite.
    NameTaken,
}

enum ClonedVariant {
    Pitched(Vec<PitchedEvent>),
    Drum(Vec<DrumEvent>),
}

fn variant_exists(pattern: &Pattern, variant: &VariantId) -> bool {
    match &pattern.body {
        PatternBody::Pitched(body) => body.variants.contains_key(variant),
        PatternBody::Drum(body) => body.variants.contains_key(variant),
    }
}

/// Pick a unique variant name within `pattern`. Mirrors the
/// project-level `unique_pattern_name` shape so the editor's
/// "+ variant" / "duplicate" affordances both flow through one
/// dedupe routine.
fn unique_variant_name(pattern: &Pattern, seed: &str) -> VariantId {
    let used: std::collections::BTreeSet<String> = match &pattern.body {
        PatternBody::Pitched(body) => body.variants.keys().map(|v| v.0.clone()).collect(),
        PatternBody::Drum(body) => body.variants.keys().map(|v| v.0.clone()).collect(),
    };
    if !used.contains(seed) {
        return VariantId::new(seed.to_string());
    }
    for n in 2u32..u32::MAX {
        let candidate = format!("{seed}-{n}");
        if !used.contains(&candidate) {
            return VariantId::new(candidate);
        }
    }
    VariantId::new(format!("{seed}-overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::events::insert_pitched_event;
    use super::super::test_support::{pitched_event, project_with_empty_pitched_pattern};

    #[test]
    fn create_variant_adds_unique_name_and_empty_body() {
        let (mut project, pid) = project_with_empty_pitched_pattern();
        let v = create_variant(&mut project, pid, "fill").expect("create_variant");
        assert_eq!(v, VariantId::new("fill"));
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Pitched(body) => {
                assert!(body.variants.contains_key(&VariantId::new("fill")));
                assert!(body.variants.get(&VariantId::new("fill")).unwrap().is_empty());
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn create_variant_suffixes_on_collision() {
        let (mut project, pid) = project_with_empty_pitched_pattern();
        let _ = create_variant(&mut project, pid, "fill");
        let v2 = create_variant(&mut project, pid, "fill").expect("create_variant 2");
        assert_eq!(v2, VariantId::new("fill-2"));
    }

    #[test]
    fn rename_variant_moves_events_to_new_key_and_rewrites_default() {
        let (mut project, pid) = project_with_empty_pitched_pattern();
        let nid = project.id_allocators.alloc_note();
        let _ =
            insert_pitched_event(&mut project, pid, &VariantId::main(), pitched_event(nid, 0, 1));
        let res = rename_variant(
            &mut project,
            pid,
            &VariantId::main(),
            VariantId::new("verse-line"),
        );
        assert!(res.is_ok());
        let pattern = project.patterns.get(&pid).unwrap();
        assert_eq!(pattern.default_variant, VariantId::new("verse-line"));
        match &pattern.body {
            PatternBody::Pitched(body) => {
                assert!(!body.variants.contains_key(&VariantId::main()));
                let events = body.variants.get(&VariantId::new("verse-line")).unwrap();
                assert_eq!(events[0].note_id, nid);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn rename_variant_refuses_collision() {
        let (mut project, pid) = project_with_empty_pitched_pattern();
        let _ = create_variant(&mut project, pid, "fill");
        let err = rename_variant(&mut project, pid, &VariantId::main(), VariantId::new("fill"))
            .unwrap_err();
        assert_eq!(err, VariantEditError::NameTaken);
        // Pattern state unchanged.
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Pitched(body) => {
                assert!(body.variants.contains_key(&VariantId::main()));
                assert!(body.variants.contains_key(&VariantId::new("fill")));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn rename_variant_to_same_name_is_noop_ok() {
        let (mut project, pid) = project_with_empty_pitched_pattern();
        let res = rename_variant(&mut project, pid, &VariantId::main(), VariantId::main());
        assert!(res.is_ok());
    }

    #[test]
    fn delete_variant_refuses_last_variant() {
        let (mut project, pid) = project_with_empty_pitched_pattern();
        let err = delete_variant(&mut project, pid, &VariantId::main()).unwrap_err();
        assert_eq!(err, VariantEditError::LastVariant);
    }

    #[test]
    fn delete_variant_reassigns_default_to_remaining() {
        let (mut project, pid) = project_with_empty_pitched_pattern();
        let _ = create_variant(&mut project, pid, "fill");
        // Default is "main"; delete it and the default should
        // promote to the remaining "fill".
        delete_variant(&mut project, pid, &VariantId::main()).expect("delete_variant");
        let pattern = project.patterns.get(&pid).unwrap();
        assert_eq!(pattern.default_variant, VariantId::new("fill"));
        match &pattern.body {
            PatternBody::Pitched(body) => {
                assert!(!body.variants.contains_key(&VariantId::main()));
                assert!(body.variants.contains_key(&VariantId::new("fill")));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn duplicate_variant_reissues_note_ids_and_picks_unique_name() {
        let (mut project, pid) = project_with_empty_pitched_pattern();
        let original = project.id_allocators.alloc_note();
        let _ = insert_pitched_event(
            &mut project,
            pid,
            &VariantId::main(),
            pitched_event(original, 0, 1),
        );
        let new_id =
            duplicate_variant(&mut project, pid, &VariantId::main()).expect("duplicate_variant");
        assert_eq!(new_id, VariantId::new("main copy"));
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Pitched(body) => {
                let copy_events = body.variants.get(&new_id).unwrap();
                assert_eq!(copy_events.len(), 1);
                assert_ne!(
                    copy_events[0].note_id, original,
                    "duplicate must allocate a fresh NoteId",
                );
            }
            _ => unreachable!(),
        }
    }
}
