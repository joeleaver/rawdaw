//! Voice-list mutations on a drum pattern.
//!
//! Drum voices live in `DrumPatternMetadata.voices` (declaration
//! order; first voice is the bottom row of the step grid). Adding a
//! voice appends to the list; removing one purges every `DrumEvent`
//! referencing it across every variant (the alternative — keeping
//! stranded events around — leaves the realization side trying to
//! address a voice the kit no longer supports).
//!
//! Rename is a structural rewrite: every event whose voice matches
//! `old` swaps to `new`. Refused (`VoiceEditError::NameTaken`) if
//! `new` already appears in the voice list — the UI shouldn't
//! collapse two voices into one through a rename. The user wanting
//! that should explicitly remove one and re-add.

use rawdaw_model::id::PatternId;
use rawdaw_model::pattern::{DrumPatternBody, DrumVoice, PatternBody};
use rawdaw_model::project::Project;

/// Append `voice` to the pattern's voice list. Returns the index of
/// the newly-added voice, or `None` if the pattern doesn't exist /
/// isn't a drum body, or the voice is already present.
pub fn add_drum_voice(
    project: &mut Project,
    pattern_id: PatternId,
    voice: DrumVoice,
) -> Option<usize> {
    let body = drum_body_mut(project, pattern_id)?;
    if body.metadata.voices.contains(&voice) {
        return None;
    }
    body.metadata.voices.push(voice);
    Some(body.metadata.voices.len() - 1)
}

/// Remove `voice` from the pattern's voice list AND purge every
/// `DrumEvent` referencing it across every variant. Returns `true` if
/// the voice was present and removed, `false` otherwise.
///
/// Refuses to remove the last voice — a drum pattern with zero voices
/// has no rows to render and no targets to dispatch to, which the UI
/// can't represent meaningfully. Caller surfaces this as a
/// "can't-remove-last-voice" message.
pub fn remove_drum_voice(
    project: &mut Project,
    pattern_id: PatternId,
    voice: &DrumVoice,
) -> Result<(), VoiceEditError> {
    let body = drum_body_mut(project, pattern_id).ok_or(VoiceEditError::NotFound)?;
    if body.metadata.voices.len() <= 1 {
        return Err(VoiceEditError::LastVoice);
    }
    let Some(idx) = body.metadata.voices.iter().position(|v| v == voice) else {
        return Err(VoiceEditError::NotFound);
    };
    body.metadata.voices.remove(idx);
    for events in body.variants.values_mut() {
        events.retain(|e| &e.voice != voice);
    }
    Ok(())
}

/// Rename `old` to `new` within the pattern's voice list AND rewrite
/// every event referencing `old`. Refuses if `new` already exists in
/// the voice list (would collapse two rows) or if `old` doesn't
/// exist.
// Used by the per-voice rename affordance in the editor header,
// which lands in a polish pass. Kept now so the P3 model surface is
// complete; allow(dead_code) until the UI consumer lands.
#[allow(dead_code)]
pub fn rename_drum_voice(
    project: &mut Project,
    pattern_id: PatternId,
    old: &DrumVoice,
    new: DrumVoice,
) -> Result<(), VoiceEditError> {
    let body = drum_body_mut(project, pattern_id).ok_or(VoiceEditError::NotFound)?;
    if old == &new {
        return Ok(());
    }
    if body.metadata.voices.contains(&new) {
        return Err(VoiceEditError::NameTaken);
    }
    let Some(idx) = body.metadata.voices.iter().position(|v| v == old) else {
        return Err(VoiceEditError::NotFound);
    };
    body.metadata.voices[idx] = new.clone();
    for events in body.variants.values_mut() {
        for ev in events.iter_mut() {
            if &ev.voice == old {
                ev.voice = new.clone();
            }
        }
    }
    Ok(())
}

/// Mutation outcomes for the voice-CRUD surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceEditError {
    /// The target pattern or source voice didn't exist.
    NotFound,
    /// Removing would leave the pattern with zero voices.
    LastVoice,
    /// Renaming would collapse onto an existing voice.
    NameTaken,
}

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
    use super::super::events::insert_drum_event;
    use super::super::test_support::{drum_event, project_with_empty_drum_pattern};
    use rawdaw_model::id::VariantId;

    #[test]
    fn add_voice_appends_and_returns_index() {
        let (mut project, pid) = project_with_empty_drum_pattern();
        // Default-created drum pattern has 4 voices (Kick/Snare/CH/OH).
        let idx = add_drum_voice(&mut project, pid, DrumVoice::Clap).expect("add ok");
        assert_eq!(idx, 4);
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Drum(body) => {
                assert_eq!(body.metadata.voices.len(), 5);
                assert_eq!(body.metadata.voices[4], DrumVoice::Clap);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn add_voice_no_op_for_existing() {
        let (mut project, pid) = project_with_empty_drum_pattern();
        // Kick is in the default voice list.
        assert!(add_drum_voice(&mut project, pid, DrumVoice::Kick).is_none());
    }

    #[test]
    fn add_voice_returns_none_for_pitched_pattern() {
        use super::super::create_pitched_pattern;
        let (mut project, _) = project_with_empty_drum_pattern();
        let pitched = create_pitched_pattern(&mut project);
        assert!(add_drum_voice(&mut project, pitched, DrumVoice::Clap).is_none());
    }

    #[test]
    fn remove_voice_purges_events_referencing_it() {
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
        remove_drum_voice(&mut project, pid, &DrumVoice::Snare).expect("remove ok");
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Drum(body) => {
                assert!(!body.metadata.voices.contains(&DrumVoice::Snare));
                let events = body.variants.get(&VariantId::main()).unwrap();
                assert_eq!(events.len(), 1);
                assert_eq!(events[0].note_id, a);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn remove_voice_refuses_when_last() {
        let (mut project, pid) = project_with_empty_drum_pattern();
        // Remove three of the four default voices, leaving Kick.
        remove_drum_voice(&mut project, pid, &DrumVoice::Snare).unwrap();
        remove_drum_voice(&mut project, pid, &DrumVoice::ClosedHat).unwrap();
        remove_drum_voice(&mut project, pid, &DrumVoice::OpenHat).unwrap();
        let err = remove_drum_voice(&mut project, pid, &DrumVoice::Kick).unwrap_err();
        assert_eq!(err, VoiceEditError::LastVoice);
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Drum(body) => {
                assert_eq!(body.metadata.voices, vec![DrumVoice::Kick]);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn remove_voice_not_found_returns_not_found() {
        let (mut project, pid) = project_with_empty_drum_pattern();
        let err = remove_drum_voice(&mut project, pid, &DrumVoice::Clap).unwrap_err();
        assert_eq!(err, VoiceEditError::NotFound);
    }

    #[test]
    fn rename_voice_rewrites_events_and_voice_list() {
        let (mut project, pid) = project_with_empty_drum_pattern();
        let a = project.id_allocators.alloc_note();
        let _ = insert_drum_event(
            &mut project,
            pid,
            &VariantId::main(),
            drum_event(a, 0, DrumVoice::Kick),
        );
        rename_drum_voice(
            &mut project,
            pid,
            &DrumVoice::Kick,
            DrumVoice::extra("808.kick"),
        )
        .expect("rename ok");
        match &project.patterns.get(&pid).unwrap().body {
            PatternBody::Drum(body) => {
                assert!(body
                    .metadata
                    .voices
                    .contains(&DrumVoice::extra("808.kick")));
                assert!(!body.metadata.voices.contains(&DrumVoice::Kick));
                let events = body.variants.get(&VariantId::main()).unwrap();
                assert_eq!(events[0].voice, DrumVoice::extra("808.kick"));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn rename_voice_refuses_collision() {
        let (mut project, pid) = project_with_empty_drum_pattern();
        let err = rename_drum_voice(&mut project, pid, &DrumVoice::Kick, DrumVoice::Snare)
            .unwrap_err();
        assert_eq!(err, VoiceEditError::NameTaken);
    }
}
