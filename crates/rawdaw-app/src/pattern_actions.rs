//! Pattern CRUD actions called from the Library panel and the
//! pattern editor.
//!
//! Free-functions that mutate a [`Project`] (and, for color, a
//! [`ProjectOverlay`]) — the calling component wraps them in
//! [`AppState::apply_project_edit`] / direct overlay `Signal::set`.
//! Lives outside `regions/library/patterns.rs` so the pattern editor
//! (P2+ of `docs/pattern-editor-plan.md`) can share the same surface
//! without an inter-region import. Mirrors [`crate::chord_loop_actions`]
//! from CL1.
//!
//! File layout (split per the next-session-pickup plan to keep each
//! file under the 700-line cap):
//! - `pattern_actions.rs` (this file): pattern-level CRUD (create /
//!   rename / delete / duplicate / set_color), reference walker, the
//!   `unique_pattern_name` helper, and pattern-level tests.
//! - `pattern_actions/events.rs`: per-event mutations on a pitched
//!   body (insert / delete / update / set_length).
//! - `pattern_actions/variants.rs`: variant-level CRUD (create /
//!   rename / delete / duplicate) shared by pitched and drum bodies.
//! - `pattern_actions/test_support.rs`: shared `#[cfg(test)]` fixtures.

mod activations;
mod events;
mod variant_schedule;
mod variants;
mod voices;

#[cfg(test)]
mod test_support;

pub use events::{
    delete_pitched_event, insert_drum_event, insert_pitched_event, set_pitched_pattern_length,
    update_pitched_event,
};
pub use activations::{
    clear_activation_variant_range, merge_activation_variant_left,
    merge_activation_variant_right, set_activation_pattern, set_activation_variant_for_bar,
};
// `remove_activation` is the explicit "drop the entry, not just clear
// the pattern_ref" path. No UI consumer yet — the PatternSelect's
// "(no pattern)" option just nulls pattern_ref. Suppress the unused
// re-export until a delete affordance lands.
#[allow(unused_imports)]
pub use activations::remove_activation;
pub use variants::{create_variant, delete_variant, duplicate_variant, VariantEditError};
pub use events::{delete_drum_event, set_drum_pattern_length, update_drum_event};
pub use voices::{add_drum_voice, remove_drum_voice, VoiceEditError};
// `rename_variant` + `rename_drum_voice` land in polish passes (no UI
// consumers yet) — re-exports suppressed so the unused-import lint
// doesn't trip until the affordances arrive. Mirrors the
// `#[allow(dead_code)]` already on the functions.
#[allow(unused_imports)]
pub use variants::rename_variant;
#[allow(unused_imports)]
pub use voices::rename_drum_voice;

use std::collections::BTreeMap;

use rawdaw_model::id::PatternId;
use rawdaw_model::pattern::{
    DrumPatternBody, DrumPatternMetadata, DrumVoice, Pattern, PatternBody, PitchedPatternBody,
    PitchedPatternMetadata,
};
use rawdaw_model::project::Project;
use rawdaw_model::section::ActivationOverride;
use rawdaw_model::time::Duration;

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
    let default_variant = rawdaw_model::id::VariantId::main();
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
    let default_variant = rawdaw_model::id::VariantId::main();
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
    use super::test_support::empty_project;
    use rawdaw_model::activation::{ActivationEntry, RealizationParams};
    use rawdaw_model::id::{ActivationEntryId, SectionId, TrackId, VariantId};
    use rawdaw_model::section::{Arrangement, Section, SectionBody, SectionVariantOverride};

    fn empty_section_with_id(project: &mut Project, name: &str) -> SectionId {
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
