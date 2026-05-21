//! Per-event mutations on a pitched pattern body.
//!
//! All four functions take `(project, pattern_id, variant)` plus the
//! per-call event/note payload. They return `None` / `false` for any
//! mismatch (missing pattern, missing variant, wrong body type) so the
//! caller can decide whether to surface an error or silently no-op.
//! Drum-body counterparts will land alongside the P3 drum step-grid
//! editor.

use rawdaw_model::id::{NoteId, PatternId, VariantId};
use rawdaw_model::pattern::{PatternBody, PitchedEvent, PitchedPatternBody};
use rawdaw_model::project::Project;
use rawdaw_model::time::Duration;

use crate::regions::pattern_editor::pitched::helpers::{
    delete_pitched_event_by_id, insert_pitched_event_sorted, update_pitched_event_by_id,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::test_support::{
        empty_project, pitched_event, project_with_empty_pitched_pattern, DEFAULT_BEATS_PER_BAR,
        DEFAULT_PATTERN_BARS,
    };
    use super::super::create_drum_pattern;
    use rawdaw_model::pitch::U7;
    use rawdaw_model::time::Duration;

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
}
