//! Per-note override application.
//!
//! A pattern event's resolution (pitch, velocity, time, duration) can be
//! overridden by entries in the activation's `per_note_overrides`. The
//! realization walker builds a `RealizedEvent` from the pattern event's
//! defaults and then calls `apply_override` to mutate it in place.
//!
//! See `docs/design/realization.md` for the override design.

use crate::activation::{NoteOverride, OverrideTransform};
use crate::id::{NoteId, NoteOverrideId};
use crate::pattern::{DrumEvent, PitchedEvent};
use crate::pitch::{MidiNote, U7};
use crate::time::{Duration, MusicalTime};

/// Mutable working copy of a single event being realized. Built from the
/// pattern event's defaults, then possibly mutated by a matching override.
pub(super) struct RealizedEvent {
    pub note: MidiNote,
    pub velocity: U7,
    pub time: MusicalTime,
    pub duration: Duration,
    pub muted: bool,
    pub override_id: Option<NoteOverrideId>,
}

impl RealizedEvent {
    pub(super) fn from_pitched(note: MidiNote, e: &PitchedEvent, time: MusicalTime) -> Self {
        Self {
            note,
            velocity: e.velocity,
            time,
            duration: e.duration,
            muted: false,
            override_id: None,
        }
    }

    pub(super) fn from_drum(note: MidiNote, e: &DrumEvent, time: MusicalTime) -> Self {
        Self {
            note,
            velocity: e.velocity,
            time,
            duration: e.duration,
            muted: false,
            override_id: None,
        }
    }
}

/// Apply the first matching override for `note_id` to `realized`. Multiple
/// overrides on the same `NoteId` is undefined at the model level; realization
/// takes the first match and ignores the rest.
///
/// Voice-leading state updates are the caller's responsibility — `Mute`
/// suppresses *emission* but should not suppress voice-leading bookkeeping,
/// so the resolved pitch is still "what would have played."
pub(super) fn apply_override(
    overrides: &[NoteOverride],
    note_id: NoteId,
    realized: &mut RealizedEvent,
) {
    let Some(ov) = overrides.iter().find(|o| o.target == note_id) else {
        return;
    };
    realized.override_id = Some(ov.id);
    match &ov.transform {
        OverrideTransform::PinPitch(n) => realized.note = *n,
        OverrideTransform::PinVelocity(v) => realized.velocity = *v,
        OverrideTransform::PinTiming(t) => {
            realized.time = realized.time + MusicalTime::ticks(*t as i64);
        }
        OverrideTransform::PinDuration(d) => realized.duration = *d,
        OverrideTransform::Mute => realized.muted = true,
    }
}
