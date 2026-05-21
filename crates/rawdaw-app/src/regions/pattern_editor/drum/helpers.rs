//! Pure helpers for the drum step-grid editor.
//!
//! Mirrors `pitched/helpers.rs` in shape: pure functions, no rinch
//! deps, unit-tested. The grid-snap primitives (`GridSpec`,
//! `snap_time_to_grid`) live one level up in
//! `regions/pattern_editor/grid.rs` and are shared with the pitched
//! editor.
//!
//! Concerns covered here:
//!
//! - **Step ↔ time** — `step_to_time` and `time_to_step` translate
//!   between the editor's integer step index (column on the grid) and
//!   the model's `MusicalTime`. Floored conversion both ways so the
//!   round trip is stable.
//! - **Total step count** — `total_steps_for_length` derives the
//!   number of grid columns from the pattern length + the editor's
//!   grid resolution. Determines the grid's visual width.
//! - **Default event factory** — `default_drum_event` produces the
//!   `(voice, velocity 64, one-grid-cell duration)` shape used by
//!   click-to-insert.
//! - **Sorted insert / delete / update by id** — same contract as the
//!   pitched-side helpers. Events are kept ascending by `time`; ties
//!   go after.

use rawdaw_model::id::NoteId;
use rawdaw_model::pattern::{DrumEvent, DrumVoice, EventHumanization};
use rawdaw_model::pitch::U7;
use rawdaw_model::time::{Duration, MusicalTime};

use super::super::grid::GridSpec;

/// Translate an integer step index into the model's `MusicalTime`.
/// Floored: `step_to_time(0, _) == 0`, `step_to_time(1, 1/16) == 240`.
/// Always returns a non-negative time.
pub fn step_to_time(step: usize, grid: GridSpec) -> MusicalTime {
    MusicalTime::ticks((step as i64) * grid.step_ticks())
}

/// Translate a `MusicalTime` into an integer step index. Floored —
/// times between two grid lines round down. Used by event-rendering to
/// decide which column a stored event occupies.
pub fn time_to_step(time: MusicalTime, grid: GridSpec) -> usize {
    let ticks = time.as_ticks().max(0);
    (ticks / grid.step_ticks()) as usize
}

/// Number of grid columns the step grid renders for a pattern of the
/// given length at the given resolution. Always at least 1 so the
/// grid stays clickable even on a degenerate zero-length pattern.
pub fn total_steps_for_length(length: Duration, grid: GridSpec) -> usize {
    let step = grid.step_ticks();
    let len_ticks = length.as_ticks().max(0);
    let count = (len_ticks / step) as usize;
    count.max(1)
}

/// Build a default `DrumEvent` for click-to-insert. Voice is supplied
/// by the row (each grid row is a fixed voice); duration is one grid
/// cell so velocity-derived rendering has a sensible default width;
/// velocity is `U7::HALF` (≈ 64) to match the pitched-side default.
///
/// `note_id` is the durable id the caller allocated from
/// `Project.id_allocators`. The caller is responsible for the alloc
/// so each editor action lands a fresh id.
pub fn default_drum_event(
    note_id: NoteId,
    time: MusicalTime,
    voice: DrumVoice,
    grid: GridSpec,
) -> DrumEvent {
    DrumEvent {
        note_id,
        time,
        duration: Duration::ticks(grid.step_ticks()),
        voice,
        velocity: U7::HALF,
        articulation: None,
        humanization: EventHumanization::default(),
    }
}

/// Insert `event` into `events`, preserving ascending order by `time`.
/// Returns the insertion index so the caller can focus the new event
/// immediately. Ties go after existing entries at the same time.
pub fn insert_drum_event_sorted(events: &mut Vec<DrumEvent>, event: DrumEvent) -> usize {
    let idx = events
        .iter()
        .position(|e| e.time > event.time)
        .unwrap_or(events.len());
    events.insert(idx, event);
    idx
}

/// Remove the event with `id` from `events`. Returns `true` if an
/// event was removed, `false` if `id` wasn't present.
pub fn delete_drum_event_by_id(events: &mut Vec<DrumEvent>, id: NoteId) -> bool {
    if let Some(pos) = events.iter().position(|e| e.note_id == id) {
        events.remove(pos);
        true
    } else {
        false
    }
}

/// Find the event with `id` and apply a mutation in place. Returns
/// `true` if the event was found and `f` ran, `false` otherwise.
pub fn update_drum_event_by_id<F>(events: &mut [DrumEvent], id: NoteId, f: F) -> bool
where
    F: FnOnce(&mut DrumEvent),
{
    if let Some(ev) = events.iter_mut().find(|e| e.note_id == id) {
        f(ev);
        true
    } else {
        false
    }
}

/// Find the event at exactly `(voice, step)` in `events`. Used by the
/// step-grid surface to decide whether a cell click should insert
/// (None) or focus (Some). "Exact" here means the event's stored time,
/// when floored to the grid, lands on `step`; events between grid
/// lines are *not* considered "on" the cell — they require timing
/// edits in the inspector.
pub fn event_at_step<'a>(
    events: &'a [DrumEvent],
    voice: &DrumVoice,
    step: usize,
    grid: GridSpec,
) -> Option<&'a DrumEvent> {
    events
        .iter()
        .find(|e| &e.voice == voice && time_to_step(e.time, grid) == step)
}

/// Short label for a `DrumVoice` row. The GM voices map to
/// fixed abbreviations (`Kick → "K"`, `Snare → "S"`, …). Extras
/// truncate to their first two characters in upper case, falling back
/// to "X" for an empty string. The full name is in the tooltip; this
/// is just for the inline row label.
pub fn voice_short_label(voice: &DrumVoice) -> String {
    match voice {
        DrumVoice::Kick => "K".into(),
        DrumVoice::Snare => "S".into(),
        DrumVoice::SnareRim => "SR".into(),
        DrumVoice::ClosedHat => "CH".into(),
        DrumVoice::OpenHat => "OH".into(),
        DrumVoice::PedalHat => "PH".into(),
        DrumVoice::TomLow => "TL".into(),
        DrumVoice::TomMid => "TM".into(),
        DrumVoice::TomHigh => "TH".into(),
        DrumVoice::Crash => "Cr".into(),
        DrumVoice::Ride => "Rd".into(),
        DrumVoice::RideBell => "RB".into(),
        DrumVoice::Clap => "Cl".into(),
        DrumVoice::Cowbell => "Co".into(),
        DrumVoice::Extra(name) => {
            let upper: String = name.chars().take(2).collect::<String>().to_uppercase();
            if upper.is_empty() {
                "X".into()
            } else {
                upper
            }
        }
    }
}

/// Full display name for a `DrumVoice`. The fixed voices use their
/// canonical English names; `Extra(name)` is the user-supplied string.
/// Used for tooltips and the inspector's voice selector.
pub fn voice_full_name(voice: &DrumVoice) -> String {
    match voice {
        DrumVoice::Kick => "Kick".into(),
        DrumVoice::Snare => "Snare".into(),
        DrumVoice::SnareRim => "Snare rim".into(),
        DrumVoice::ClosedHat => "Closed hat".into(),
        DrumVoice::OpenHat => "Open hat".into(),
        DrumVoice::PedalHat => "Pedal hat".into(),
        DrumVoice::TomLow => "Tom low".into(),
        DrumVoice::TomMid => "Tom mid".into(),
        DrumVoice::TomHigh => "Tom high".into(),
        DrumVoice::Crash => "Crash".into(),
        DrumVoice::Ride => "Ride".into(),
        DrumVoice::RideBell => "Ride bell".into(),
        DrumVoice::Clap => "Clap".into(),
        DrumVoice::Cowbell => "Cowbell".into(),
        DrumVoice::Extra(name) => name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_model::time::PPQ;

    fn ev(id: u64, time_ticks: i64, voice: DrumVoice) -> DrumEvent {
        DrumEvent {
            note_id: NoteId::new(id),
            time: MusicalTime::ticks(time_ticks),
            duration: Duration::ticks(240),
            voice,
            velocity: U7::HALF,
            articulation: None,
            humanization: EventHumanization::default(),
        }
    }

    #[test]
    fn step_to_time_zero_is_zero() {
        assert_eq!(
            step_to_time(0, GridSpec::STRAIGHT_SIXTEENTH),
            MusicalTime::ZERO,
        );
    }

    #[test]
    fn step_to_time_sixteenth_step_is_240_ticks() {
        assert_eq!(
            step_to_time(1, GridSpec::STRAIGHT_SIXTEENTH),
            MusicalTime::ticks(240),
        );
        assert_eq!(
            step_to_time(16, GridSpec::STRAIGHT_SIXTEENTH),
            MusicalTime::ticks(240 * 16),
        );
    }

    #[test]
    fn time_to_step_floors() {
        let g = GridSpec::STRAIGHT_SIXTEENTH;
        assert_eq!(time_to_step(MusicalTime::ticks(0), g), 0);
        assert_eq!(time_to_step(MusicalTime::ticks(239), g), 0);
        assert_eq!(time_to_step(MusicalTime::ticks(240), g), 1);
        assert_eq!(time_to_step(MusicalTime::ticks(241), g), 1);
        assert_eq!(time_to_step(MusicalTime::ticks(480), g), 2);
    }

    #[test]
    fn time_to_step_handles_negative_as_zero() {
        // Defensive — model events never have negative times, but
        // the floor should still saturate at 0.
        assert_eq!(
            time_to_step(MusicalTime::ticks(-50), GridSpec::STRAIGHT_SIXTEENTH),
            0,
        );
    }

    #[test]
    fn step_then_time_round_trips() {
        let g = GridSpec::STRAIGHT_SIXTEENTH;
        for s in [0usize, 1, 7, 15, 16, 32] {
            assert_eq!(time_to_step(step_to_time(s, g), g), s);
        }
    }

    #[test]
    fn total_steps_for_one_bar_sixteenth_is_sixteen() {
        // 1 bar in 4/4 = PPQ * 4 ticks; at 1/16 each step is PPQ * 4 /
        // 16 = PPQ / 4 ticks. So 16 steps per bar.
        let length = Duration::ticks(PPQ * 4);
        assert_eq!(
            total_steps_for_length(length, GridSpec::STRAIGHT_SIXTEENTH),
            16,
        );
    }

    #[test]
    fn total_steps_clamps_to_one_for_zero_length() {
        let length = Duration::ticks(0);
        assert_eq!(
            total_steps_for_length(length, GridSpec::STRAIGHT_SIXTEENTH),
            1,
        );
    }

    #[test]
    fn default_event_has_velocity_half_and_supplied_voice() {
        let g = GridSpec::STRAIGHT_SIXTEENTH;
        let event = default_drum_event(NoteId::new(7), MusicalTime::ticks(480), DrumVoice::Snare, g);
        assert_eq!(event.note_id, NoteId::new(7));
        assert_eq!(event.time, MusicalTime::ticks(480));
        assert_eq!(event.voice, DrumVoice::Snare);
        assert_eq!(event.velocity, U7::HALF);
        assert_eq!(event.duration, Duration::ticks(g.step_ticks()));
        assert!(event.articulation.is_none());
    }

    #[test]
    fn insert_preserves_time_order_and_returns_index() {
        let mut events = vec![
            ev(1, 0, DrumVoice::Kick),
            ev(2, 480, DrumVoice::Snare),
        ];
        let idx = insert_drum_event_sorted(&mut events, ev(3, 240, DrumVoice::ClosedHat));
        assert_eq!(idx, 1);
        let ticks: Vec<i64> = events.iter().map(|e| e.time.as_ticks()).collect();
        assert!(ticks.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn insert_ties_go_after_existing() {
        let mut events = vec![ev(1, 480, DrumVoice::Kick)];
        let idx = insert_drum_event_sorted(&mut events, ev(2, 480, DrumVoice::Snare));
        assert_eq!(idx, 1);
        assert_eq!(events[0].note_id, NoteId::new(1));
        assert_eq!(events[1].note_id, NoteId::new(2));
    }

    #[test]
    fn delete_by_id_removes_only_matching_event() {
        let mut events = vec![
            ev(10, 0, DrumVoice::Kick),
            ev(11, 240, DrumVoice::Snare),
            ev(12, 480, DrumVoice::ClosedHat),
        ];
        assert!(delete_drum_event_by_id(&mut events, NoteId::new(11)));
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].note_id, NoteId::new(10));
        assert_eq!(events[1].note_id, NoteId::new(12));
    }

    #[test]
    fn delete_by_id_no_op_for_missing_id() {
        let mut events = vec![ev(1, 0, DrumVoice::Kick)];
        assert!(!delete_drum_event_by_id(&mut events, NoteId::new(999)));
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn update_by_id_runs_mutation_and_reports_found() {
        let mut events = vec![ev(1, 0, DrumVoice::Kick), ev(2, 240, DrumVoice::Snare)];
        let ran = update_drum_event_by_id(&mut events, NoteId::new(2), |ev| {
            ev.velocity = U7::clamp(100);
        });
        assert!(ran);
        assert_eq!(events[1].velocity, U7::clamp(100));
    }

    #[test]
    fn update_by_id_returns_false_for_missing_id() {
        let mut events = vec![ev(1, 0, DrumVoice::Kick)];
        let ran = update_drum_event_by_id(&mut events, NoteId::new(999), |ev| {
            ev.velocity = U7::MAX;
        });
        assert!(!ran);
        assert_eq!(events[0].velocity, U7::HALF);
    }

    #[test]
    fn event_at_step_matches_voice_and_floored_time() {
        let g = GridSpec::STRAIGHT_SIXTEENTH;
        let events = vec![
            ev(1, 0, DrumVoice::Kick),
            ev(2, 240, DrumVoice::Snare),
            ev(3, 480, DrumVoice::Kick),
        ];
        assert_eq!(
            event_at_step(&events, &DrumVoice::Kick, 0, g).map(|e| e.note_id),
            Some(NoteId::new(1)),
        );
        assert_eq!(
            event_at_step(&events, &DrumVoice::Snare, 1, g).map(|e| e.note_id),
            Some(NoteId::new(2)),
        );
        assert_eq!(
            event_at_step(&events, &DrumVoice::Kick, 2, g).map(|e| e.note_id),
            Some(NoteId::new(3)),
        );
        // Empty cell: no event for Snare at step 0.
        assert!(event_at_step(&events, &DrumVoice::Snare, 0, g).is_none());
        // Voice mismatch: Kick at step 1 has no event (Snare does).
        assert!(event_at_step(&events, &DrumVoice::Kick, 1, g).is_none());
    }

    #[test]
    fn event_at_step_floors_between_grid_lines() {
        // An event at 241 ticks (240 = grid line, +1 tick off) belongs
        // to step 1 (floored), so a click at step 1 should focus it.
        let g = GridSpec::STRAIGHT_SIXTEENTH;
        let events = vec![ev(1, 241, DrumVoice::Kick)];
        assert_eq!(
            event_at_step(&events, &DrumVoice::Kick, 1, g).map(|e| e.note_id),
            Some(NoteId::new(1)),
        );
        assert!(event_at_step(&events, &DrumVoice::Kick, 0, g).is_none());
    }
}
