//! End-to-end smoke test: build a tiny project and check its structure.

mod common;

use rawdaw_model::*;

#[test]
fn build_tiny_project_in_c_major() {
    let project = common::build_tiny_project();

    assert_eq!(project.tracks.len(), 3);
    assert_eq!(project.patterns.len(), 3);
    assert_eq!(project.chord_loops.len(), 1);
    assert_eq!(project.sections.len(), 1);
    assert_eq!(project.arrangement.sections.len(), 3);

    // Every pattern event has a unique, durably-allocated NoteId.
    let all_note_ids: Vec<NoteId> = project
        .patterns
        .values()
        .flat_map(|p| match &p.body {
            PatternBody::Pitched(b) => b
                .variants
                .values()
                .flat_map(|v| v.iter().map(|e| e.note_id))
                .collect::<Vec<_>>(),
            PatternBody::Drum(b) => b
                .variants
                .values()
                .flat_map(|v| v.iter().map(|e| e.note_id))
                .collect::<Vec<_>>(),
        })
        .collect();
    let mut sorted = all_note_ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        all_note_ids.len(),
        "NoteIds must be unique across all patterns"
    );
}
