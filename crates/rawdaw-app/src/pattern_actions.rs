//! Pattern CRUD actions called from the Library panel.
//!
//! Free-functions that mutate a [`Project`] (and, for color, a
//! [`ProjectOverlay`]) — the calling component wraps them in
//! [`AppState::apply_project_edit`] / direct overlay `Signal::set`.
//! Lives outside `regions/library/patterns.rs` so the future pattern
//! editor (P2+ of `docs/pattern-editor-plan.md`) can share the same
//! surface without an inter-region import. Mirrors
//! [`crate::chord_loop_actions`] from CL1.
//!
//! Composition-writability gives us the edit pump for free
//! ([[project-next-session-pickup]]); these helpers only need to
//! describe the mutation, not coordinate audio.

use std::collections::BTreeMap;

use rawdaw_model::id::{NoteId, PatternId, VariantId};
use rawdaw_model::pattern::{
    DrumPatternBody, DrumPatternMetadata, DrumVoice, Pattern, PatternBody, PitchedEvent,
    PitchedPatternBody, PitchedPatternMetadata,
};
use rawdaw_model::project::Project;
use rawdaw_model::section::ActivationOverride;
use rawdaw_model::time::Duration;

use crate::regions::pattern_editor::pitched::helpers::{
    delete_pitched_event_by_id, insert_pitched_event_sorted, update_pitched_event_by_id,
};

use crate::overlay::ProjectOverlay;

/// Default length for a freshly-created pattern. Four bars in 4/4 —
/// matches the round-1 pattern fixtures (`bass-main`, `lead-main`,
/// etc.). The user can edit this in the pattern editor (P2/P3). `i64`
/// because [`Duration::bars`] takes the bar count as `i64`.
pub const DEFAULT_PATTERN_BARS: i64 = 4;
pub const DEFAULT_BEATS_PER_BAR: u32 = 4;

/// Default drum-voice list for a freshly-created drum pattern.
/// Mirrors the drum synth's GM classifier (Kick=36, Snare=38,
/// ClosedHat=42, OpenHat=46) so a new pattern audibly routes to the
/// existing voices without a kit change.
fn default_drum_voices() -> Vec<DrumVoice> {
    vec![DrumVoice::Kick, DrumVoice::Snare, DrumVoice::ClosedHat, DrumVoice::OpenHat]
}

/// Create a new empty pitched pattern. Allocates a fresh
/// [`PatternId`], picks a unique `"untitled"` name, seeds the empty
/// `main` variant, and inserts the pattern into `project.patterns`.
/// Returns the new id so the caller can select it immediately.
pub fn create_pitched_pattern(project: &mut Project) -> PatternId {
    let id = project.id_allocators.alloc_pattern();
    let name = unique_pattern_name(&project.patterns, "untitled");
    let default_variant = VariantId::main();
    let mut variants = BTreeMap::new();
    variants.insert(default_variant.clone(), Vec::new());
    let pattern = Pattern {
        id,
        name,
        default_variant,
        body: PatternBody::Pitched(PitchedPatternBody {
            metadata: PitchedPatternMetadata {
                length: Duration::bars(DEFAULT_PATTERN_BARS, DEFAULT_BEATS_PER_BAR),
            },
            variants,
        }),
    };
    project.patterns.insert(id, pattern);
    id
}

/// Create a new empty drum pattern. Seeds the four-voice default
/// (Kick/Snare/ClosedHat/OpenHat) so the pattern audibly routes to
/// the drum-synth's existing voices on first binding.
pub fn create_drum_pattern(project: &mut Project) -> PatternId {
    let id = project.id_allocators.alloc_pattern();
    let name = unique_pattern_name(&project.patterns, "untitled");
    let default_variant = VariantId::main();
    let mut variants = BTreeMap::new();
    variants.insert(default_variant.clone(), Vec::new());
    let pattern = Pattern {
        id,
        name,
        default_variant,
        body: PatternBody::Drum(DrumPatternBody {
            metadata: DrumPatternMetadata {
                length: Duration::bars(DEFAULT_PATTERN_BARS, DEFAULT_BEATS_PER_BAR),
                voices: default_drum_voices(),
            },
            variants,
        }),
    };
    project.patterns.insert(id, pattern);
    id
}

/// Rename a pattern in place. Caller is expected to have trimmed
/// the new name and refused empty values (the [`NameControl`]-style
/// pattern is reused by the Library row's inline editor). Silently
/// no-ops if `id` doesn't exist — UI handlers can't observe a missing
/// id without a race against deletion.
pub fn rename_pattern(project: &mut Project, id: PatternId, name: String) {
    if let Some(p) = project.patterns.get_mut(&id) {
        p.name = name;
    }
}

/// Duplicate the pattern identified by `id`. Returns the new id, or
/// `None` if the source id doesn't exist. The duplicate gets a fresh
/// allocator-issued id and a "copy"-suffixed unique name. **Every
/// note in every variant gets a freshly-allocated [`NoteId`]** —
/// NoteIds are durable per the project-status contract; even a clone
/// must not reuse them.
pub fn duplicate_pattern(project: &mut Project, id: PatternId) -> Option<PatternId> {
    let source = project.patterns.get(&id).cloned()?;
    let new_id = project.id_allocators.alloc_pattern();
    let seed = format!("{} copy", source.name);
    let name = unique_pattern_name(&project.patterns, &seed);

    let body = match source.body {
        PatternBody::Pitched(p) => {
            let mut new_variants = BTreeMap::new();
            for (vid, events) in p.variants {
                let new_events = events
                    .into_iter()
                    .map(|mut e| {
                        e.note_id = project.id_allocators.alloc_note();
                        e
                    })
                    .collect();
                new_variants.insert(vid, new_events);
            }
            PatternBody::Pitched(PitchedPatternBody {
                metadata: p.metadata,
                variants: new_variants,
            })
        }
        PatternBody::Drum(d) => {
            let mut new_variants = BTreeMap::new();
            for (vid, events) in d.variants {
                let new_events = events
                    .into_iter()
                    .map(|mut e| {
                        e.note_id = project.id_allocators.alloc_note();
                        e
                    })
                    .collect();
                new_variants.insert(vid, new_events);
            }
            PatternBody::Drum(DrumPatternBody {
                metadata: d.metadata,
                variants: new_variants,
            })
        }
    };

    let duplicate = Pattern {
        id: new_id,
        name,
        default_variant: source.default_variant,
        body,
    };
    project.patterns.insert(new_id, duplicate);
    Some(new_id)
}

/// Delete a pattern. Refuses if any [`ActivationEntry`] in any
/// section's base or variant override references it — returns
/// [`DeleteRefused::ReferencedBy`] listing the section names so the
/// caller can surface a useful message. P1 stubs the UI alert as an
/// `eprintln!`; the toast/alert primitive lands later. Same shape as
/// [`crate::chord_loop_actions::delete_chord_loop`].
pub fn delete_pattern(project: &mut Project, id: PatternId) -> Result<(), DeleteRefused> {
    let referenced_by = pattern_references(project, id);
    if !referenced_by.is_empty() {
        return Err(DeleteRefused::ReferencedBy(referenced_by));
    }
    if project.patterns.remove(&id).is_none() {
        return Err(DeleteRefused::NotFound);
    }
    Ok(())
}

/// Set the overlay color for a pattern. Color strings are expected
/// to be `#RRGGBB`; no validation here, since the picker supplies
/// palette literals from `theme::PAL_*`. Empty string removes the
/// entry so the row falls back to the default `theme::TEXT2` tint at
/// render time.
pub fn set_pattern_color(overlay: &mut ProjectOverlay, id: PatternId, color: String) {
    if color.is_empty() {
        overlay.pattern_color.remove(&id);
    } else {
        overlay.pattern_color.insert(id, color);
    }
}

/// Result of refusing to delete a pattern. The reference variant
/// carries owned section names so the UI can build a message like
/// "verse, chorus reference this pattern" without re-walking the
/// project.
#[derive(Debug, Clone, PartialEq)]
pub enum DeleteRefused {
    /// One or more sections (`base` or any variant) reference the
    /// pattern through an [`ActivationEntry`]. The `Vec<String>` is
    /// the deduplicated, sorted list of referencing section names.
    ReferencedBy(Vec<String>),
    /// The pattern id didn't exist in the project. UI-side this is a
    /// "shouldn't happen" race; surfaced as an error so the caller
    /// doesn't silently no-op a click the user expected to do
    /// something.
    NotFound,
}

/// Walk the project and collect every section name that references
/// `id` in its base activations or any variant override. Deduplicated +
/// sorted for deterministic error messages. Public so a future
/// pattern editor (or a P4 activation editor) can render "where is
/// this pattern used" UX without duplicating the walk.
pub fn pattern_references(project: &Project, id: PatternId) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for section in project.sections.values() {
        let in_base = section
            .base
            .activations
            .values()
            .any(|a| a.pattern_ref == Some(id));
        let in_variant = section.variants.values().any(|v| {
            v.activations.values().any(|act| match act {
                ActivationOverride::Replace(entry) => entry.pattern_ref == Some(id),
                ActivationOverride::Silent => false,
            })
        });
        if in_base || in_variant {
            names.push(section.name.clone());
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Insert a pitched event into the pattern + variant identified by
/// `(pattern_id, variant)`. Returns `Some(idx)` with the insertion
/// index when both the pattern and variant exist + the pattern is a
/// pitched body; `None` otherwise. Caller is expected to drive this
/// through [`crate::state::AppState::apply_project_edit`].
///
/// Used by the piano-roll's click-to-insert. The new event already
/// carries its durable [`NoteId`] (allocated by the caller through
/// `Project.id_allocators` before constructing the event).
pub fn insert_pitched_event(
    project: &mut Project,
    pattern_id: PatternId,
    variant: &VariantId,
    event: PitchedEvent,
) -> Option<usize> {
    let body = pitched_body_mut(project, pattern_id)?;
    let events = body.variants.get_mut(variant)?;
    Some(insert_pitched_event_sorted(events, event))
}

/// Remove the event identified by `note_id` from the pattern +
/// variant. Returns `true` if an event was removed, `false` if the
/// pattern/variant/note didn't exist. Used by the piano-roll's
/// "delete focused note" action.
pub fn delete_pitched_event(
    project: &mut Project,
    pattern_id: PatternId,
    variant: &VariantId,
    note_id: NoteId,
) -> bool {
    let Some(body) = pitched_body_mut(project, pattern_id) else { return false };
    let Some(events) = body.variants.get_mut(variant) else { return false };
    delete_pitched_event_by_id(events, note_id)
}

/// Apply a closure to the event identified by `note_id`. Returns
/// `true` if the event existed and `f` ran. Used by the per-note
/// inspector so one shared mutation surface can flip articulation,
/// adjust velocity, swap PitchSpec, etc.
pub fn update_pitched_event<F>(
    project: &mut Project,
    pattern_id: PatternId,
    variant: &VariantId,
    note_id: NoteId,
    f: F,
) -> bool
where
    F: FnOnce(&mut PitchedEvent),
{
    let Some(body) = pitched_body_mut(project, pattern_id) else { return false };
    let Some(events) = body.variants.get_mut(variant) else { return false };
    update_pitched_event_by_id(events, note_id, f)
}

/// Set the length of a pitched pattern. Drum patterns are ignored
/// (P3 will get its own helper if drum-length editing needs different
/// behavior; the contract is the same shape). Out-of-range events
/// are *retained* — they stop realizing once the length shrinks past
/// them, but a follow-up length nudge in the other direction restores
/// them losslessly. Matches P2 design decision 11.
pub fn set_pitched_pattern_length(
    project: &mut Project,
    pattern_id: PatternId,
    length: Duration,
) {
    if let Some(body) = pitched_body_mut(project, pattern_id) {
        body.metadata.length = length;
    }
}

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
    Drum(Vec<rawdaw_model::pattern::DrumEvent>),
}

/// Mutable accessor for a pattern's pitched body. Returns `None` if
/// the id doesn't exist *or* the body is a drum body — callers that
/// want to handle both shapes need their own dispatch.
fn pitched_body_mut(
    project: &mut Project,
    pattern_id: PatternId,
) -> Option<&mut PitchedPatternBody> {
    match &mut project.patterns.get_mut(&pattern_id)?.body {
        PatternBody::Pitched(body) => Some(body),
        PatternBody::Drum(_) => None,
    }
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

/// Pick a unique name in the form `"<seed>"`, `"<seed>-2"`,
/// `"<seed>-3"`, … given the existing pattern name set. Used by
/// `create_*_pattern` (with seed `"untitled"`) and `duplicate_pattern`
/// (with seed `"<source> copy"`).
fn unique_pattern_name(patterns: &BTreeMap<PatternId, Pattern>, seed: &str) -> String {
    let used: std::collections::BTreeSet<&str> =
        patterns.values().map(|p| p.name.as_str()).collect();
    if !used.contains(seed) {
        return seed.to_string();
    }
    for n in 2u32..u32::MAX {
        let candidate = format!("{seed}-{n}");
        if !used.contains(candidate.as_str()) {
            return candidate;
        }
    }
    format!("{seed}-overflow")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_model::activation::{ActivationEntry, RealizationParams};
    use rawdaw_model::id::{ActivationEntryId, TrackId};
    use rawdaw_model::pitch::PitchClass;
    use rawdaw_model::scale::Scale;
    use rawdaw_model::section::{Arrangement, Section, SectionBody, SectionVariantOverride};

    fn empty_project() -> Project {
        Project::new(Scale::major(PitchClass::C))
    }

    fn empty_section_with_id(project: &mut Project, name: &str) -> rawdaw_model::id::SectionId {
        let sid = project.id_allocators.alloc_section();
        project.sections.insert(
            sid,
            Section {
                id: sid,
                name: name.into(),
                base: SectionBody {
                    duration_bars: 4,
                    scale_override: None,
                    chord_loops: Vec::new(),
                    activations: BTreeMap::new(),
                },
                variants: BTreeMap::new(),
                default_variant: VariantId::base(),
            },
        );
        project.arrangement = Arrangement::default();
        sid
    }

    fn activation_referencing(pattern: PatternId) -> ActivationEntry {
        ActivationEntry {
            id: ActivationEntryId::new(0),
            pattern_ref: Some(pattern),
            variant_schedule: Vec::new(),
            realization: RealizationParams::default(),
            per_note_overrides: Vec::new(),
        }
    }

    #[test]
    fn create_pitched_pattern_allocates_id_and_unique_name() {
        let mut project = empty_project();
        let a = create_pitched_pattern(&mut project);
        let b = create_pitched_pattern(&mut project);
        assert_ne!(a, b, "two creates must produce distinct ids");
        let names: Vec<&str> = project.patterns.values().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"untitled"));
        assert!(names.contains(&"untitled-2"));
    }

    #[test]
    fn create_pitched_pattern_seeds_main_variant_and_default_length() {
        let mut project = empty_project();
        let id = create_pitched_pattern(&mut project);
        let p = project.patterns.get(&id).unwrap();
        assert_eq!(p.default_variant, VariantId::main());
        match &p.body {
            PatternBody::Pitched(body) => {
                assert_eq!(
                    body.metadata.length,
                    Duration::bars(DEFAULT_PATTERN_BARS, DEFAULT_BEATS_PER_BAR),
                );
                let main_events = body.variants.get(&VariantId::main()).unwrap();
                assert!(main_events.is_empty());
            }
            PatternBody::Drum(_) => panic!("pitched create produced a drum body"),
        }
    }

    #[test]
    fn create_drum_pattern_seeds_voices_and_main_variant() {
        let mut project = empty_project();
        let id = create_drum_pattern(&mut project);
        let p = project.patterns.get(&id).unwrap();
        match &p.body {
            PatternBody::Drum(body) => {
                assert_eq!(body.metadata.voices, default_drum_voices());
                let main_events = body.variants.get(&VariantId::main()).unwrap();
                assert!(main_events.is_empty());
            }
            PatternBody::Pitched(_) => panic!("drum create produced a pitched body"),
        }
    }

    #[test]
    fn rename_pattern_writes_new_name() {
        let mut project = empty_project();
        let id = create_pitched_pattern(&mut project);
        rename_pattern(&mut project, id, "bass-main".into());
        assert_eq!(project.patterns.get(&id).unwrap().name, "bass-main");
    }

    #[test]
    fn rename_pattern_is_a_noop_for_missing_ids() {
        let mut project = empty_project();
        let real = create_pitched_pattern(&mut project);
        let bogus = PatternId::new(9999);
        let before = project.patterns.clone();
        rename_pattern(&mut project, bogus, "ignored".into());
        assert_eq!(project.patterns, before);
        assert_eq!(project.patterns.get(&real).unwrap().name, "untitled");
    }

    #[test]
    fn duplicate_pattern_clones_with_fresh_id_and_suffixed_name() {
        let mut project = empty_project();
        let original = create_pitched_pattern(&mut project);
        rename_pattern(&mut project, original, "lead".into());

        let copy = duplicate_pattern(&mut project, original).unwrap();
        assert_ne!(copy, original);
        assert_eq!(project.patterns.get(&copy).unwrap().name, "lead copy");
        let copy2 = duplicate_pattern(&mut project, original).unwrap();
        assert_eq!(project.patterns.get(&copy2).unwrap().name, "lead copy-2");
    }

    #[test]
    fn duplicate_pattern_allocates_fresh_note_ids() {
        // NoteIds are durable — even a clone must not reuse them, so
        // per-note overrides on the source cannot silently re-bind.
        use rawdaw_model::pattern::{PitchSpec, PitchedEvent};
        use rawdaw_model::pitch::U7;
        use rawdaw_model::time::MusicalTime;

        let mut project = empty_project();
        let id = create_pitched_pattern(&mut project);
        // Inject one event so we can observe the NoteId reassignment.
        let original_note = project.id_allocators.alloc_note();
        match &mut project.patterns.get_mut(&id).unwrap().body {
            PatternBody::Pitched(body) => {
                body.variants.get_mut(&VariantId::main()).unwrap().push(PitchedEvent {
                    note_id: original_note,
                    time: MusicalTime::ZERO,
                    duration: Duration::bars(1, DEFAULT_BEATS_PER_BAR),
                    velocity: U7::HALF,
                    articulation: None,
                    humanization: Default::default(),
                    spec: PitchSpec::Rest,
                });
            }
            PatternBody::Drum(_) => unreachable!(),
        }

        let copy = duplicate_pattern(&mut project, id).unwrap();
        match &project.patterns.get(&copy).unwrap().body {
            PatternBody::Pitched(body) => {
                let copy_note = body.variants.get(&VariantId::main()).unwrap()[0].note_id;
                assert_ne!(
                    copy_note, original_note,
                    "duplicate must allocate a fresh NoteId",
                );
            }
            PatternBody::Drum(_) => unreachable!(),
        }
    }

    #[test]
    fn duplicate_missing_id_returns_none() {
        let mut project = empty_project();
        assert_eq!(duplicate_pattern(&mut project, PatternId::new(0)), None);
    }

    #[test]
    fn delete_unreferenced_pattern_removes_it() {
        let mut project = empty_project();
        let id = create_pitched_pattern(&mut project);
        assert_eq!(delete_pattern(&mut project, id), Ok(()));
        assert!(!project.patterns.contains_key(&id));
    }

    #[test]
    fn delete_referenced_pattern_refuses_and_lists_section() {
        let mut project = empty_project();
        let pid = create_pitched_pattern(&mut project);
        let sid = empty_section_with_id(&mut project, "verse");
        project
            .sections
            .get_mut(&sid)
            .unwrap()
            .base
            .activations
            .insert(TrackId::new(1), activation_referencing(pid));

        let err = delete_pattern(&mut project, pid).unwrap_err();
        match err {
            DeleteRefused::ReferencedBy(names) => assert_eq!(names, vec!["verse".to_string()]),
            other => panic!("expected ReferencedBy(verse), got {other:?}"),
        }
        assert!(project.patterns.contains_key(&pid));
    }

    #[test]
    fn delete_refuses_when_only_variant_references_pattern() {
        // Defends against a "base doesn't reference it; variant does"
        // miss in the walk. Build a section whose base activations
        // don't mention the pattern but whose variant override
        // Replaces a track with an activation that does.
        let mut project = empty_project();
        let pid = create_pitched_pattern(&mut project);
        let sid = empty_section_with_id(&mut project, "outro");

        let mut variant_acts = BTreeMap::new();
        variant_acts.insert(
            TrackId::new(2),
            ActivationOverride::Replace(activation_referencing(pid)),
        );
        let mut variants = BTreeMap::new();
        variants.insert(
            VariantId::from("chorus"),
            SectionVariantOverride {
                activations: variant_acts,
                ..Default::default()
            },
        );
        project.sections.get_mut(&sid).unwrap().variants = variants;

        let err = delete_pattern(&mut project, pid).unwrap_err();
        assert_eq!(err, DeleteRefused::ReferencedBy(vec!["outro".into()]));
    }

    #[test]
    fn delete_ignores_silent_variant_overrides() {
        // `ActivationOverride::Silent` carries no pattern_ref; a
        // section with only Silent overrides for a track should NOT
        // be reported as referencing the pattern.
        let mut project = empty_project();
        let pid = create_pitched_pattern(&mut project);
        let sid = empty_section_with_id(&mut project, "bridge");

        let mut variant_acts = BTreeMap::new();
        variant_acts.insert(TrackId::new(2), ActivationOverride::Silent);
        let mut variants = BTreeMap::new();
        variants.insert(
            VariantId::from("stripped"),
            SectionVariantOverride {
                activations: variant_acts,
                ..Default::default()
            },
        );
        project.sections.get_mut(&sid).unwrap().variants = variants;

        assert_eq!(delete_pattern(&mut project, pid), Ok(()));
        assert!(!project.patterns.contains_key(&pid));
    }

    #[test]
    fn delete_missing_id_returns_not_found() {
        let mut project = empty_project();
        let err = delete_pattern(&mut project, PatternId::new(0)).unwrap_err();
        assert_eq!(err, DeleteRefused::NotFound);
    }

    use rawdaw_model::pattern::{OctaveSpec, PitchSpec, PitchedEvent};
    use rawdaw_model::pitch::{Octave, U7};
    use rawdaw_model::scale::ScaleDegree;
    use rawdaw_model::time::MusicalTime;

    fn pitched_event(id: NoteId, time_ticks: i64, degree: u8) -> PitchedEvent {
        PitchedEvent {
            note_id: id,
            time: MusicalTime::ticks(time_ticks),
            duration: Duration::ticks(240),
            velocity: U7::HALF,
            articulation: None,
            humanization: Default::default(),
            spec: PitchSpec::Scale {
                degree: ScaleDegree::new(degree),
                octave: OctaveSpec::Anchored(Octave(3)),
            },
        }
    }

    fn project_with_empty_pitched_pattern() -> (Project, PatternId) {
        let mut project = empty_project();
        let pid = create_pitched_pattern(&mut project);
        (project, pid)
    }

    #[test]
    fn insert_pitched_event_inserts_and_returns_index() {
        let (mut project, pid) = project_with_empty_pitched_pattern();
        let nid = project.id_allocators.alloc_note();
        let ev = pitched_event(nid, 0, 1);
        let idx =
            insert_pitched_event(&mut project, pid, &VariantId::main(), ev).expect("insert ok");
        assert_eq!(idx, 0);
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Pitched(body) => {
                let events = body.variants.get(&VariantId::main()).unwrap();
                assert_eq!(events.len(), 1);
                assert_eq!(events[0].note_id, nid);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn insert_pitched_event_returns_none_for_drum_body() {
        // The pitched-specific helper refuses to mutate drum bodies —
        // the caller is expected to dispatch on the pattern kind
        // before reaching this surface. The contract is "None means
        // wrong body type or missing id"; either way the UI
        // shouldn't have routed the click here.
        let mut project = empty_project();
        let pid = create_drum_pattern(&mut project);
        let nid = project.id_allocators.alloc_note();
        let ev = pitched_event(nid, 0, 1);
        let res = insert_pitched_event(&mut project, pid, &VariantId::main(), ev);
        assert!(res.is_none());
    }

    #[test]
    fn delete_pitched_event_removes_only_matching_event() {
        let (mut project, pid) = project_with_empty_pitched_pattern();
        let a = project.id_allocators.alloc_note();
        let b = project.id_allocators.alloc_note();
        let _ =
            insert_pitched_event(&mut project, pid, &VariantId::main(), pitched_event(a, 0, 1));
        let _ = insert_pitched_event(
            &mut project,
            pid,
            &VariantId::main(),
            pitched_event(b, 240, 2),
        );
        assert!(delete_pitched_event(&mut project, pid, &VariantId::main(), b));
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Pitched(body) => {
                let events = body.variants.get(&VariantId::main()).unwrap();
                assert_eq!(events.len(), 1);
                assert_eq!(events[0].note_id, a);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn update_pitched_event_applies_mutation() {
        let (mut project, pid) = project_with_empty_pitched_pattern();
        let a = project.id_allocators.alloc_note();
        let _ =
            insert_pitched_event(&mut project, pid, &VariantId::main(), pitched_event(a, 0, 1));
        let ran =
            update_pitched_event(&mut project, pid, &VariantId::main(), a, |ev| {
                ev.velocity = U7::clamp(110);
                ev.duration = Duration::ticks(480);
            });
        assert!(ran);
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Pitched(body) => {
                let event = &body.variants.get(&VariantId::main()).unwrap()[0];
                assert_eq!(event.velocity, U7::clamp(110));
                assert_eq!(event.duration, Duration::ticks(480));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn set_pitched_pattern_length_writes_metadata() {
        let (mut project, pid) = project_with_empty_pitched_pattern();
        set_pitched_pattern_length(&mut project, pid, Duration::bars(8, DEFAULT_BEATS_PER_BAR));
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Pitched(body) => {
                assert_eq!(body.metadata.length, Duration::bars(8, DEFAULT_BEATS_PER_BAR));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn set_pitched_pattern_length_noop_on_drum_body() {
        // The pitched-specific helper must be inert against drum
        // patterns — drum length editing is P3.
        let mut project = empty_project();
        let pid = create_drum_pattern(&mut project);
        set_pitched_pattern_length(&mut project, pid, Duration::bars(99, DEFAULT_BEATS_PER_BAR));
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Drum(body) => {
                assert_eq!(
                    body.metadata.length,
                    Duration::bars(DEFAULT_PATTERN_BARS, DEFAULT_BEATS_PER_BAR),
                );
            }
            _ => unreachable!(),
        }
    }

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

    #[test]
    fn set_pattern_color_writes_and_clears_overlay_entry() {
        let mut overlay = ProjectOverlay::default();
        let id = PatternId::new(1);
        set_pattern_color(&mut overlay, id, "#abcdef".into());
        assert_eq!(overlay.pattern_color.get(&id).map(|s| s.as_str()), Some("#abcdef"));

        set_pattern_color(&mut overlay, id, "".into());
        assert!(!overlay.pattern_color.contains_key(&id));
    }
}
