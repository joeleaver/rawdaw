//! Pure helpers for the pitched pattern editor.
//!
//! Lives in its own module so it can be unit-tested against
//! pure-function contracts without spinning up the rinch
//! component tree. Same shape as
//! `regions/chord_loop_editor/helpers.rs`.
//!
//! Concerns covered here:
//!
//! - **Grid snapping** — `GridSpec` + `snap_time_to_grid` quantize
//!   click-time to musical subdivisions.
//! - **Default event factory** — `default_pitched_event` produces
//!   the `PitchSpec::Scale { degree: 1, octave: Anchored(3) }`
//!   shape called out in P2 design decision 3.
//! - **Sorted insert / delete by id** — preserves the
//!   "events ascending by `time`, ties go after" contract that
//!   matches the chord-loop editor's CL2 helpers.
//! - **PitchSpec conversions** — `pitch_spec_to_row` /
//!   `row_to_pitch_spec` translate between a vertical lane on
//!   the piano roll (a `degree` for `Scale` / `Chord` specs, a
//!   pitch class for `Absolute`) and the model's `PitchSpec`.

use rawdaw_model::id::NoteId;
use rawdaw_model::pattern::{
    EventHumanization, OctaveSpec, PitchSpec, PitchedEvent,
};
use rawdaw_model::pitch::{Octave, U7};
use rawdaw_model::scale::ScaleDegree;
use rawdaw_model::time::{Duration, MusicalTime};

// `GridSpec` + `snap_time_to_grid` live one level up so the drum
// editor can share them (sibling-to-sibling deps would be wrong).
// Re-exported here so existing callers don't need to change paths.
pub use super::super::grid::{snap_time_to_grid, GridSpec};

/// Default anchored octave for new pitched events. Matches P2 design
/// decision 13: `OctaveSpec::Anchored(3)` is the simplest variant for
/// users to reason about; `Nearest` is the realization-side default
/// for voice-leading but the inspector lets users opt in.
pub const DEFAULT_ANCHOR_OCTAVE: Octave = Octave(3);

/// Number of pitch rows the piano-roll renders by default. Centered
/// on `DEFAULT_ANCHOR_OCTAVE`; covers a comfortable ~2 octaves of
/// degree space for diatonic input. Configurable per pattern in a
/// future polish pass — for v1 it's a constant.
pub const DEFAULT_PITCH_ROWS: usize = 25;

/// Build a default `PitchedEvent` for click-to-insert. Matches
/// P2 design decision 3: `PitchSpec::Scale { degree: 1, octave:
/// Anchored(3) }`, velocity `64`, duration of one grid cell.
///
/// `note_id` is the durable id the caller allocated from
/// `Project.id_allocators`. The caller is responsible for the
/// alloc so each editor action lands a fresh id.
pub fn default_pitched_event(
    note_id: NoteId,
    time: MusicalTime,
    grid: GridSpec,
) -> PitchedEvent {
    PitchedEvent {
        note_id,
        time,
        duration: Duration::ticks(grid.step_ticks()),
        velocity: U7::HALF,
        articulation: None,
        humanization: EventHumanization::default(),
        spec: PitchSpec::Scale {
            degree: ScaleDegree::new(1),
            octave: OctaveSpec::Anchored(DEFAULT_ANCHOR_OCTAVE),
        },
    }
}

/// Insert `event` into `events`, preserving ascending order by
/// `time`. Returns the insertion index so the caller can focus the
/// new event immediately. Ties go after existing entries at the same
/// time — matches the chord-loop editor's `insert_chord_event_sorted`.
pub fn insert_pitched_event_sorted(
    events: &mut Vec<PitchedEvent>,
    event: PitchedEvent,
) -> usize {
    let idx = events
        .iter()
        .position(|e| e.time > event.time)
        .unwrap_or(events.len());
    events.insert(idx, event);
    idx
}

/// Remove the event with `id` from `events`. Returns `true` if an
/// event was removed, `false` if `id` wasn't present. Keying by
/// durable `NoteId` rather than index keeps the contract robust
/// across insertions (see P2 design decision 9).
pub fn delete_pitched_event_by_id(
    events: &mut Vec<PitchedEvent>,
    id: NoteId,
) -> bool {
    if let Some(pos) = events.iter().position(|e| e.note_id == id) {
        events.remove(pos);
        true
    } else {
        false
    }
}

/// Find the event with `id` and apply a mutation in place. Returns
/// `true` if the event was found and `f` ran, `false` otherwise.
///
/// Used by per-note inspector mutations so a single closure can
/// touch any combination of fields without re-walking the project
/// in every caller.
pub fn update_pitched_event_by_id<F>(events: &mut [PitchedEvent], id: NoteId, f: F) -> bool
where
    F: FnOnce(&mut PitchedEvent),
{
    if let Some(ev) = events.iter_mut().find(|e| e.note_id == id) {
        f(ev);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_model::pitch::{Octave, PitchClass};

    #[test]
    fn default_event_has_scale_degree_one_at_anchored_three() {
        let g = GridSpec::STRAIGHT_SIXTEENTH;
        let ev = default_pitched_event(NoteId::new(7), MusicalTime::ZERO, g);
        match ev.spec {
            PitchSpec::Scale { degree, octave } => {
                assert_eq!(degree.degree, 1);
                assert_eq!(octave, OctaveSpec::Anchored(Octave(3)));
            }
            _ => panic!("default must be PitchSpec::Scale"),
        }
        assert_eq!(ev.note_id, NoteId::new(7));
        assert_eq!(ev.time, MusicalTime::ZERO);
        assert_eq!(ev.duration, Duration::ticks(g.step_ticks()));
        assert_eq!(ev.velocity, U7::HALF);
    }

    fn anchored_scale_event(id: u64, time_ticks: i64, degree: u8) -> PitchedEvent {
        PitchedEvent {
            note_id: NoteId::new(id),
            time: MusicalTime::ticks(time_ticks),
            duration: Duration::ticks(240),
            velocity: U7::HALF,
            articulation: None,
            humanization: EventHumanization::default(),
            spec: PitchSpec::Scale {
                degree: ScaleDegree::new(degree),
                octave: OctaveSpec::Anchored(Octave(3)),
            },
        }
    }

    #[test]
    fn insert_preserves_time_order() {
        let mut events = vec![
            anchored_scale_event(1, 0, 1),
            anchored_scale_event(2, 480, 3),
        ];
        let idx = insert_pitched_event_sorted(&mut events, anchored_scale_event(3, 240, 2));
        assert_eq!(idx, 1);
        let ticks: Vec<i64> = events.iter().map(|e| e.time.as_ticks()).collect();
        assert!(ticks.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn insert_ties_go_after() {
        let mut events = vec![anchored_scale_event(1, 480, 1)];
        let idx = insert_pitched_event_sorted(&mut events, anchored_scale_event(2, 480, 5));
        assert_eq!(idx, 1);
        assert_eq!(events[0].note_id, NoteId::new(1));
        assert_eq!(events[1].note_id, NoteId::new(2));
    }

    #[test]
    fn delete_by_id_removes_only_matching_event() {
        let mut events = vec![
            anchored_scale_event(10, 0, 1),
            anchored_scale_event(11, 240, 2),
            anchored_scale_event(12, 480, 3),
        ];
        assert!(delete_pitched_event_by_id(&mut events, NoteId::new(11)));
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].note_id, NoteId::new(10));
        assert_eq!(events[1].note_id, NoteId::new(12));
    }

    #[test]
    fn delete_by_id_no_op_for_missing_id() {
        let mut events = vec![anchored_scale_event(1, 0, 1)];
        assert!(!delete_pitched_event_by_id(&mut events, NoteId::new(999)));
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn update_by_id_runs_mutation_and_reports_found() {
        let mut events = vec![
            anchored_scale_event(1, 0, 1),
            anchored_scale_event(2, 240, 2),
        ];
        let ran = update_pitched_event_by_id(&mut events, NoteId::new(2), |ev| {
            ev.velocity = U7::clamp(100);
        });
        assert!(ran);
        assert_eq!(events[1].velocity, U7::clamp(100));
    }

    #[test]
    fn update_by_id_returns_false_for_missing_id() {
        let mut events = vec![anchored_scale_event(1, 0, 1)];
        let ran = update_pitched_event_by_id(&mut events, NoteId::new(999), |ev| {
            ev.velocity = U7::MAX;
        });
        assert!(!ran);
        assert_eq!(events[0].velocity, U7::HALF);
    }

    #[test]
    fn absolute_pitch_construction_round_trip() {
        // Sanity check that PitchedEvent::absolute (the model ctor)
        // shares the same field layout we rely on; if the constructor
        // ever drifts (e.g. flips Octave to OctaveSpec) the editor's
        // Absolute escape hatch needs the matching update.
        let ev = PitchedEvent::absolute(
            NoteId::new(1),
            MusicalTime::ZERO,
            Duration::beats(1),
            U7::HALF,
            PitchClass::E,
            Octave(4),
        );
        match ev.spec {
            PitchSpec::Absolute { pitch_class, octave } => {
                assert_eq!(pitch_class, PitchClass::E);
                assert_eq!(octave, Octave(4));
            }
            _ => panic!("expected Absolute"),
        }
    }
}
