//! Per-event mutations on pitched and drum pattern bodies.
//!
//! All `(insert|delete|update)_*_event` functions take `(project,
//! pattern_id, variant)` plus the per-call event/note payload. They
//! return `None` / `false` for any mismatch (missing pattern, missing
//! variant, wrong body type) so the caller can decide whether to
//! surface an error or silently no-op.

use rawdaw_model::id::{NoteId, PatternId, VariantId};
use rawdaw_model::pattern::{
    DrumEvent, DrumPatternBody, PatternBody, PitchedEvent, PitchedPatternBody,
};
use rawdaw_model::project::Project;
use rawdaw_model::time::Duration;

use crate::regions::pattern_editor::drum::helpers::{
    delete_drum_event_by_id, insert_drum_event_sorted, update_drum_event_by_id,
};
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
/// (use [`set_drum_pattern_length`] instead). Out-of-range events
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

// ─── Drum-event mirrors of the pitched API ───────────────────────────────

/// Insert a drum event. Same contract as [`insert_pitched_event`],
/// dispatched onto the drum body.
pub fn insert_drum_event(
    project: &mut Project,
    pattern_id: PatternId,
    variant: &VariantId,
    event: DrumEvent,
) -> Option<usize> {
    let body = drum_body_mut(project, pattern_id)?;
    let events = body.variants.get_mut(variant)?;
    Some(insert_drum_event_sorted(events, event))
}

/// Remove the drum event identified by `note_id`. Returns `true` if an
/// event was removed.
pub fn delete_drum_event(
    project: &mut Project,
    pattern_id: PatternId,
    variant: &VariantId,
    note_id: NoteId,
) -> bool {
    let Some(body) = drum_body_mut(project, pattern_id) else { return false };
    let Some(events) = body.variants.get_mut(variant) else { return false };
    delete_drum_event_by_id(events, note_id)
}

/// Apply a closure to the drum event identified by `note_id`. Returns
/// `true` if the event existed and `f` ran. Used by the drum
/// inspector for velocity / articulation / humanization edits.
pub fn update_drum_event<F>(
    project: &mut Project,
    pattern_id: PatternId,
    variant: &VariantId,
    note_id: NoteId,
    f: F,
) -> bool
where
    F: FnOnce(&mut DrumEvent),
{
    let Some(body) = drum_body_mut(project, pattern_id) else { return false };
    let Some(events) = body.variants.get_mut(variant) else { return false };
    update_drum_event_by_id(events, note_id, f)
}

/// Set the length of a drum pattern. Pitched patterns are ignored.
/// Same retain-out-of-range-events contract as pitched.
pub fn set_drum_pattern_length(
    project: &mut Project,
    pattern_id: PatternId,
    length: Duration,
) {
    if let Some(body) = drum_body_mut(project, pattern_id) {
        body.metadata.length = length;
    }
}

// ─── Body-typed accessors ────────────────────────────────────────────────

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

/// Mutable accessor for a pattern's drum body. Returns `None` if the
/// id doesn't exist *or* the body is a pitched body.
fn drum_body_mut(
    project: &mut Project,
    pattern_id: PatternId,
) -> Option<&mut DrumPatternBody> {
    match &mut project.patterns.get_mut(&pattern_id)?.body {
        PatternBody::Drum(body) => Some(body),
        PatternBody::Pitched(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::test_support::{
        drum_event, empty_project, pitched_event, project_with_empty_drum_pattern,
        project_with_empty_pitched_pattern, DEFAULT_BEATS_PER_BAR, DEFAULT_PATTERN_BARS,
    };
    use super::super::{create_drum_pattern, create_pitched_pattern};
    use rawdaw_model::pattern::DrumVoice;
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
        // patterns — drum patterns route through set_drum_pattern_length.
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

    // ─── Drum-event tests ───────────────────────────────────────────

    #[test]
    fn insert_drum_event_inserts_and_returns_index() {
        let (mut project, pid) = project_with_empty_drum_pattern();
        let nid = project.id_allocators.alloc_note();
        let ev = drum_event(nid, 0, DrumVoice::Kick);
        let idx =
            insert_drum_event(&mut project, pid, &VariantId::main(), ev).expect("insert ok");
        assert_eq!(idx, 0);
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Drum(body) => {
                let events = body.variants.get(&VariantId::main()).unwrap();
                assert_eq!(events.len(), 1);
                assert_eq!(events[0].note_id, nid);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn insert_drum_event_returns_none_for_pitched_body() {
        let mut project = empty_project();
        let pid = create_pitched_pattern(&mut project);
        let nid = project.id_allocators.alloc_note();
        let ev = drum_event(nid, 0, DrumVoice::Kick);
        let res = insert_drum_event(&mut project, pid, &VariantId::main(), ev);
        assert!(res.is_none());
    }

    #[test]
    fn delete_drum_event_removes_only_matching_event() {
        let (mut project, pid) = project_with_empty_drum_pattern();
        let a = project.id_allocators.alloc_note();
        let b = project.id_allocators.alloc_note();
        let _ = insert_drum_event(
            &mut project,
            pid,
            &VariantId::main(),
            drum_event(a, 0, DrumVoice::Kick),
        );
        let _ = insert_drum_event(
            &mut project,
            pid,
            &VariantId::main(),
            drum_event(b, 240, DrumVoice::Snare),
        );
        assert!(delete_drum_event(&mut project, pid, &VariantId::main(), b));
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Drum(body) => {
                let events = body.variants.get(&VariantId::main()).unwrap();
                assert_eq!(events.len(), 1);
                assert_eq!(events[0].note_id, a);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn update_drum_event_applies_mutation() {
        let (mut project, pid) = project_with_empty_drum_pattern();
        let a = project.id_allocators.alloc_note();
        let _ = insert_drum_event(
            &mut project,
            pid,
            &VariantId::main(),
            drum_event(a, 0, DrumVoice::Kick),
        );
        let ran = update_drum_event(&mut project, pid, &VariantId::main(), a, |ev| {
            ev.velocity = U7::clamp(110);
            ev.duration = Duration::ticks(480);
        });
        assert!(ran);
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Drum(body) => {
                let event = &body.variants.get(&VariantId::main()).unwrap()[0];
                assert_eq!(event.velocity, U7::clamp(110));
                assert_eq!(event.duration, Duration::ticks(480));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn set_drum_pattern_length_writes_metadata() {
        let (mut project, pid) = project_with_empty_drum_pattern();
        set_drum_pattern_length(&mut project, pid, Duration::bars(8, DEFAULT_BEATS_PER_BAR));
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Drum(body) => {
                assert_eq!(body.metadata.length, Duration::bars(8, DEFAULT_BEATS_PER_BAR));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn set_drum_pattern_length_noop_on_pitched_body() {
        let mut project = empty_project();
        let pid = create_pitched_pattern(&mut project);
        set_drum_pattern_length(&mut project, pid, Duration::bars(99, DEFAULT_BEATS_PER_BAR));
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Pitched(body) => {
                assert_eq!(
                    body.metadata.length,
                    Duration::bars(DEFAULT_PATTERN_BARS, DEFAULT_BEATS_PER_BAR),
                );
            }
            _ => unreachable!(),
        }
    }
}
