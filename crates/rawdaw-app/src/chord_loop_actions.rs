//! Chord-loop CRUD actions called from the Library panel.
//!
//! Free-functions that mutate a [`Project`] (and, for color, a
//! [`ProjectOverlay`]) — the calling component wraps them in
//! [`AppState::apply_project_edit`] / direct overlay `Signal::set`.
//! Lives outside `regions/library.rs` so the chord-loop editor in
//! CL2 can share the same surface without an inter-region import.
//!
//! Composition-writability gives us the edit pump for free
//! ([[project-next-session-pickup]]); these helpers only need to
//! describe the mutation, not coordinate audio.

use std::collections::BTreeMap;

use rawdaw_model::chord::ChordLoop;
use rawdaw_model::id::ChordLoopId;
use rawdaw_model::project::Project;
use rawdaw_model::time::Duration;

use crate::overlay::ProjectOverlay;

/// Default length for a freshly-created chord loop. Four bars in
/// 4/4 — matches the round-1 verse/chorus loops and is the most
/// common starting length for a pop progression. The user can
/// edit this in the chord-loop editor (CL2). `i64` because
/// [`Duration::bars`] takes the bar count as `i64`.
pub const DEFAULT_LOOP_BARS: i64 = 4;
pub const DEFAULT_BEATS_PER_BAR: u32 = 4;

/// Create a new empty chord loop. Allocates a fresh
/// [`ChordLoopId`], picks a unique `"untitled-N"` name, and inserts
/// it into `project.chord_loops`. Returns the new id so the caller
/// (the Library panel's `+` action) can select the new loop
/// immediately.
pub fn create_chord_loop(project: &mut Project) -> ChordLoopId {
    let id = project.id_allocators.alloc_chord_loop();
    let name = unique_loop_name(&project.chord_loops, "untitled");
    let loop_ = ChordLoop {
        id,
        name,
        length: Duration::bars(DEFAULT_LOOP_BARS, DEFAULT_BEATS_PER_BAR),
        key: None,
        events: Vec::new(),
    };
    project.chord_loops.insert(id, loop_);
    id
}

/// Rename a chord loop in place. Caller is expected to have trimmed
/// the new name and refused empty values (the [`NameControl`]-style
/// pattern is reused by the Library row's inline editor). Silently
/// no-ops if `id` doesn't exist — UI handlers can't observe a
/// missing id without a race against deletion, and an `unwrap`-
/// style panic here would be worse.
pub fn rename_chord_loop(project: &mut Project, id: ChordLoopId, name: String) {
    if let Some(loop_) = project.chord_loops.get_mut(&id) {
        loop_.name = name;
    }
}

/// Duplicate the chord loop identified by `id`. Returns the new
/// id, or `None` if the source id doesn't exist. The duplicate
/// gets a fresh allocator-issued id, a "copy"-suffixed unique
/// name, and a deep clone of `events` / `key` / `length`.
pub fn duplicate_chord_loop(project: &mut Project, id: ChordLoopId) -> Option<ChordLoopId> {
    let source = project.chord_loops.get(&id).cloned()?;
    let new_id = project.id_allocators.alloc_chord_loop();
    let seed = format!("{} copy", source.name);
    let name = unique_loop_name(&project.chord_loops, &seed);
    let duplicate = ChordLoop {
        id: new_id,
        name,
        length: source.length,
        key: source.key,
        events: source.events,
    };
    project.chord_loops.insert(new_id, duplicate);
    Some(new_id)
}

/// Delete a chord loop. Refuses if any section's `base` or any
/// variant override references it — returns
/// [`DeleteRefused::ReferencedBy`] listing the section names so
/// the caller can surface a useful message. CL1 stubs the UI alert
/// as an `eprintln!`; the toast/alert primitive lands later.
pub fn delete_chord_loop(project: &mut Project, id: ChordLoopId) -> Result<(), DeleteRefused> {
    let referenced_by = referencing_section_names(project, id);
    if !referenced_by.is_empty() {
        return Err(DeleteRefused::ReferencedBy(referenced_by));
    }
    if project.chord_loops.remove(&id).is_none() {
        return Err(DeleteRefused::NotFound);
    }
    Ok(())
}

/// Set the overlay color for a chord loop. Color strings are
/// expected to be `#RRGGBB`; no validation here, since the picker
/// supplies palette literals from `theme::PAL_*`. Empty string
/// removes the entry so the row falls back to the default
/// `theme::TEXT2` tint at render time.
pub fn set_chord_loop_color(overlay: &mut ProjectOverlay, id: ChordLoopId, color: String) {
    if color.is_empty() {
        overlay.chord_loop_color.remove(&id);
    } else {
        overlay.chord_loop_color.insert(id, color);
    }
}

/// Result of refusing to delete a chord loop. The reference variant
/// carries owned section names so the UI can build a message like
/// "verse, chorus reference this loop" without re-walking the
/// project.
#[derive(Debug, Clone, PartialEq)]
pub enum DeleteRefused {
    /// One or more sections (`base` or any variant) reference the
    /// chord loop. The `Vec<String>` is the deduplicated, sorted
    /// list of referencing section names.
    ReferencedBy(Vec<String>),
    /// The chord loop id didn't exist in the project. UI-side this
    /// is a "shouldn't happen" race; surfaced as an error so the
    /// caller doesn't silently no-op a click the user expected to
    /// do something.
    NotFound,
}

/// Walk the project and collect every section name that references
/// `id` in its base body or any variant override. Deduplicated +
/// sorted for deterministic error messages.
fn referencing_section_names(project: &Project, id: ChordLoopId) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for section in project.sections.values() {
        let in_base = section.base.chord_loops.iter().any(|(_, cid)| *cid == id);
        let in_variant = section.variants.values().any(|v| {
            v.chord_loops
                .as_ref()
                .is_some_and(|chords| chords.iter().any(|(_, cid)| *cid == id))
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
/// `"<seed>-3"`, … given the existing chord-loop name set. Used by
/// `create_chord_loop` (with seed `"untitled"`) and
/// `duplicate_chord_loop` (with seed `"<source> copy"`).
fn unique_loop_name(loops: &BTreeMap<ChordLoopId, ChordLoop>, seed: &str) -> String {
    let used: std::collections::BTreeSet<&str> = loops.values().map(|l| l.name.as_str()).collect();
    if !used.contains(seed) {
        return seed.to_string();
    }
    for n in 2u32..u32::MAX {
        let candidate = format!("{seed}-{n}");
        if !used.contains(candidate.as_str()) {
            return candidate;
        }
    }
    // The 2..u32::MAX range is effectively unreachable — if it ever
    // fires we'd rather return a recognizable sentinel than panic in
    // a UI handler.
    format!("{seed}-overflow")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_model::pitch::PitchClass;
    use rawdaw_model::scale::Scale;
    use rawdaw_model::section::{Arrangement, Section, SectionBody, SectionVariantOverride};
    use rawdaw_model::time::BarRange;

    fn empty_project() -> Project {
        Project::new(Scale::major(PitchClass::C))
    }

    fn project_with_section_referencing(loop_id: ChordLoopId, section_name: &str) -> Project {
        let mut project = empty_project();
        let sid = project.id_allocators.alloc_section();
        project.sections.insert(
            sid,
            Section {
                id: sid,
                name: section_name.into(),
                base: SectionBody {
                    duration_bars: 4,
                    scale_override: None,
                    chord_loops: vec![(BarRange::new(0, 4), loop_id)],
                    activations: BTreeMap::new(),
                },
                variants: BTreeMap::new(),
                default_variant: rawdaw_model::id::VariantId::base(),
            },
        );
        project.arrangement = Arrangement::default();
        project
    }

    #[test]
    fn create_chord_loop_allocates_id_and_unique_name() {
        let mut project = empty_project();
        let a = create_chord_loop(&mut project);
        let b = create_chord_loop(&mut project);
        assert_ne!(a, b, "two creates must produce distinct ids");
        let names: Vec<&str> = project.chord_loops.values().map(|l| l.name.as_str()).collect();
        // First create grabs "untitled", second falls through to
        // "untitled-2" via `unique_loop_name`. Sorted because the
        // BTreeMap iterates in id order.
        assert!(names.contains(&"untitled"));
        assert!(names.contains(&"untitled-2"));
    }

    #[test]
    fn create_chord_loop_defaults_length_and_empty_events() {
        let mut project = empty_project();
        let id = create_chord_loop(&mut project);
        let loop_ = project.chord_loops.get(&id).unwrap();
        assert_eq!(loop_.length, Duration::bars(DEFAULT_LOOP_BARS, DEFAULT_BEATS_PER_BAR));
        assert!(loop_.events.is_empty());
        assert!(loop_.key.is_none());
    }

    #[test]
    fn rename_chord_loop_writes_new_name() {
        let mut project = empty_project();
        let id = create_chord_loop(&mut project);
        rename_chord_loop(&mut project, id, "verse".into());
        assert_eq!(project.chord_loops.get(&id).unwrap().name, "verse");
    }

    #[test]
    fn rename_chord_loop_is_a_noop_for_missing_ids() {
        // UI handlers can't safely panic on missing ids — there's a
        // delete/rename race in principle. Confirm the no-op leaves
        // the rest of the project untouched.
        let mut project = empty_project();
        let real = create_chord_loop(&mut project);
        let bogus = ChordLoopId::new(9999);
        let before = project.chord_loops.clone();
        rename_chord_loop(&mut project, bogus, "ignored".into());
        assert_eq!(project.chord_loops, before);
        assert_eq!(project.chord_loops.get(&real).unwrap().name, "untitled");
    }

    #[test]
    fn duplicate_chord_loop_clones_with_fresh_id_and_suffixed_name() {
        let mut project = empty_project();
        let original = create_chord_loop(&mut project);
        rename_chord_loop(&mut project, original, "verse".into());

        let copy = duplicate_chord_loop(&mut project, original).unwrap();
        assert_ne!(copy, original);
        assert_eq!(project.chord_loops.get(&copy).unwrap().name, "verse copy");
        // Second duplicate of the same source bumps the suffix.
        let copy2 = duplicate_chord_loop(&mut project, original).unwrap();
        assert_eq!(project.chord_loops.get(&copy2).unwrap().name, "verse copy-2");
    }

    #[test]
    fn duplicate_missing_id_returns_none() {
        let mut project = empty_project();
        assert_eq!(duplicate_chord_loop(&mut project, ChordLoopId::new(0)), None);
    }

    #[test]
    fn delete_unreferenced_chord_loop_removes_it() {
        let mut project = empty_project();
        let id = create_chord_loop(&mut project);
        assert_eq!(delete_chord_loop(&mut project, id), Ok(()));
        assert!(!project.chord_loops.contains_key(&id));
    }

    #[test]
    fn delete_referenced_chord_loop_refuses_and_lists_section() {
        let mut project = empty_project();
        let id = create_chord_loop(&mut project);
        // Splice the loop into a section's base body so the walk finds it.
        let project_with_ref = {
            let mut p = project_with_section_referencing(id, "verse");
            // Carry the loop into the new project — `project_with_section_referencing`
            // returns a fresh `Project::new`, so re-insert the loop record.
            p.chord_loops = project.chord_loops.clone();
            p
        };
        let mut p = project_with_ref;
        let err = delete_chord_loop(&mut p, id).unwrap_err();
        match err {
            DeleteRefused::ReferencedBy(names) => assert_eq!(names, vec!["verse".to_string()]),
            other => panic!("expected ReferencedBy(verse), got {other:?}"),
        }
        // The loop should still be present after the refusal.
        assert!(p.chord_loops.contains_key(&id));
    }

    #[test]
    fn delete_refuses_when_only_variant_references_loop() {
        // Defends against a "base doesn't reference it; variant does"
        // miss in the walk. Build a section whose base has no chord
        // loops but whose variant override carries one.
        let mut project = empty_project();
        let id = create_chord_loop(&mut project);
        let sid = project.id_allocators.alloc_section();
        let mut variants = BTreeMap::new();
        variants.insert(
            rawdaw_model::id::VariantId::from("chorus"),
            SectionVariantOverride {
                chord_loops: Some(vec![(BarRange::new(0, 4), id)]),
                ..Default::default()
            },
        );
        project.sections.insert(
            sid,
            Section {
                id: sid,
                name: "outro".into(),
                base: SectionBody {
                    duration_bars: 4,
                    scale_override: None,
                    chord_loops: Vec::new(),
                    activations: BTreeMap::new(),
                },
                variants,
                default_variant: rawdaw_model::id::VariantId::base(),
            },
        );

        let err = delete_chord_loop(&mut project, id).unwrap_err();
        assert_eq!(err, DeleteRefused::ReferencedBy(vec!["outro".into()]));
    }

    #[test]
    fn delete_missing_id_returns_not_found() {
        let mut project = empty_project();
        let err = delete_chord_loop(&mut project, ChordLoopId::new(0)).unwrap_err();
        assert_eq!(err, DeleteRefused::NotFound);
    }

    #[test]
    fn set_chord_loop_color_writes_and_clears_overlay_entry() {
        let mut overlay = ProjectOverlay::default();
        let id = ChordLoopId::new(1);
        set_chord_loop_color(&mut overlay, id, "#abcdef".into());
        assert_eq!(overlay.chord_loop_color.get(&id).map(|s| s.as_str()), Some("#abcdef"));

        // Empty string clears the entry — row falls back to default tint.
        set_chord_loop_color(&mut overlay, id, "".into());
        assert!(!overlay.chord_loop_color.contains_key(&id));
    }
}
