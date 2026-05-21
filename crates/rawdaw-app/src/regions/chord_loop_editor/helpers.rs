//! Pure helpers for the chord-loop editor: musical-time conversions,
//! beat snapping, default-event factory, sorted insert, drag-target
//! clamping. Pulled out of the component modules so they have unit
//! tests against pure-function contracts.

use rawdaw_model::chord::{ChordEvent, ChordQuality, ChordSpec, ChordSuffix, RomanDegree};
use rawdaw_model::time::{Duration, MusicalTime, PPQ};

/// Minimum duration a chord event can shrink to during a resize drag.
/// One beat keeps each block visible + clickable even at extreme
/// shortening.
pub const MIN_EVENT_TICKS: i64 = PPQ;

/// Round a tick count down to whole bars given the time signature.
/// Floors at 0 so a tiny `Duration` doesn't underflow to a negative
/// bar count.
pub fn ticks_to_bars(ticks: i64, beats_per_bar: u32) -> u32 {
    let ticks_per_bar = PPQ * beats_per_bar.max(1) as i64;
    (ticks / ticks_per_bar).max(0) as u32
}

/// Snap a tick value down to the nearest beat. Used by
/// `click-empty-bar to insert` to land the new event on a beat
/// boundary rather than mid-tick.
pub fn snap_ticks_to_beat(ticks: i64) -> i64 {
    if ticks <= 0 {
        return 0;
    }
    (ticks / PPQ) * PPQ
}

/// Default chord event used when the timeline's `+ chord` action
/// fires or the user clicks an empty bar. `Functional { I major }`
/// matches the design doc's "90% UX target: in normal use, the
/// user picks a global project key once, then composes entirely
/// in Roman numerals."
pub fn default_chord_event(time: MusicalTime, duration: Duration) -> ChordEvent {
    ChordEvent {
        time,
        duration,
        chord: ChordSpec::Functional {
            roman: RomanDegree::I,
            suffix: ChordSuffix::new(ChordQuality::Major),
            in_key: None,
        },
        bass: None,
        annotation: None,
    }
}

/// Clamp the move target so the event at `idx` doesn't overlap a
/// neighbour and stays inside `[0, loop_length_ticks)`. Returns the
/// snapped, clamped new start time (in ticks) for the event.
///
/// The event's *current* duration is preserved — the move is purely
/// translational. Snapping is to the nearest beat (floor for moves;
/// distinct from set-on-click which snaps the target start). If the
/// clamped range collapses to a single point, returns that point even
/// if it equals the event's current time (callers can no-op via an
/// equality check).
pub fn clamped_move(
    events: &[ChordEvent],
    idx: usize,
    delta_ticks: i64,
    loop_length_ticks: i64,
) -> i64 {
    let Some(ev) = events.get(idx) else { return 0 };
    let original = ev.time.as_ticks();
    let duration = ev.duration.as_ticks();
    let raw = original + delta_ticks;
    // Snap to beat. round_down so the block doesn't fly past the
    // cursor at fast drag speeds.
    let snapped = snap_ticks_to_beat(raw);
    // Clamp left: must not overlap previous event's end.
    let left_bound = events
        .get(idx.wrapping_sub(1))
        .filter(|_| idx > 0)
        .map(|e| e.time.as_ticks() + e.duration.as_ticks())
        .unwrap_or(0);
    // Clamp right: must not overlap next event's start, and must
    // leave room for the moved event's duration before loop_length.
    let right_bound = events
        .get(idx + 1)
        .map(|e| e.time.as_ticks())
        .unwrap_or(loop_length_ticks)
        .saturating_sub(duration);
    snapped.clamp(left_bound, right_bound.max(left_bound))
}

/// Clamp the resize target so the event at `idx` doesn't extend past
/// its right neighbour (or loop_length) and doesn't shrink below
/// [`MIN_EVENT_TICKS`]. Returns the snapped, clamped new duration in
/// ticks.
pub fn clamped_resize(
    events: &[ChordEvent],
    idx: usize,
    delta_ticks: i64,
    loop_length_ticks: i64,
) -> i64 {
    let Some(ev) = events.get(idx) else { return 0 };
    let original = ev.duration.as_ticks();
    let start = ev.time.as_ticks();
    let raw_end = start + original + delta_ticks;
    let snapped_end = snap_ticks_to_beat(raw_end);
    let right_bound = events
        .get(idx + 1)
        .map(|e| e.time.as_ticks())
        .unwrap_or(loop_length_ticks);
    let new_end = snapped_end.min(right_bound).max(start + MIN_EVENT_TICKS);
    new_end - start
}

/// Insert `event` into `events`, preserving ascending order by
/// `time`. Returns the insertion index so the editor can focus
/// the new event immediately.
///
/// If two events share the exact same `time`, the new event is
/// inserted after the existing entries at that time — matches the
/// "newest wins for ties" UX expectation of click-to-insert.
pub fn insert_chord_event_sorted(events: &mut Vec<ChordEvent>, event: ChordEvent) -> usize {
    let idx = events
        .iter()
        .position(|e| e.time > event.time)
        .unwrap_or(events.len());
    events.insert(idx, event);
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_to_bars_rounds_down() {
        // 4/4 → 4 beats = 4 * PPQ ticks per bar.
        let one_bar = PPQ * 4;
        assert_eq!(ticks_to_bars(one_bar, 4), 1);
        assert_eq!(ticks_to_bars(one_bar - 1, 4), 0);
        assert_eq!(ticks_to_bars(one_bar * 4, 4), 4);
        // Defensive: beats_per_bar=0 must not divide by zero. The
        // `max(1)` clamp inside `ticks_to_bars` covers the edge.
        assert_eq!(ticks_to_bars(one_bar, 0), 4);
    }

    #[test]
    fn snap_ticks_to_beat_floors_to_beat_boundary() {
        assert_eq!(snap_ticks_to_beat(0), 0);
        assert_eq!(snap_ticks_to_beat(PPQ), PPQ);
        assert_eq!(snap_ticks_to_beat(PPQ + 1), PPQ);
        assert_eq!(snap_ticks_to_beat(PPQ * 2 - 1), PPQ);
        // Negative ticks (a UI bug, in principle) snap to 0
        // rather than producing a meaningless negative event time.
        assert_eq!(snap_ticks_to_beat(-100), 0);
    }

    #[test]
    fn default_chord_event_is_one_major_in_section_key() {
        let ev = default_chord_event(MusicalTime::ZERO, Duration::beats(4));
        match ev.chord {
            ChordSpec::Functional { roman, suffix, in_key } => {
                assert_eq!(roman, RomanDegree::I);
                assert_eq!(suffix.quality, ChordQuality::Major);
                assert!(in_key.is_none());
            }
            _ => panic!("default must be functional"),
        }
        assert_eq!(ev.bass, None);
        assert_eq!(ev.annotation, None);
    }

    #[test]
    fn insert_chord_event_sorted_preserves_ascending_time() {
        let mut events = vec![
            default_chord_event(MusicalTime::ZERO, Duration::beats(2)),
            default_chord_event(MusicalTime::beats(4), Duration::beats(2)),
        ];
        let idx = insert_chord_event_sorted(
            &mut events,
            default_chord_event(MusicalTime::beats(2), Duration::beats(2)),
        );
        assert_eq!(idx, 1);
        let times: Vec<i64> = events.iter().map(|e| e.time.as_ticks()).collect();
        assert!(times.windows(2).all(|w| w[0] <= w[1]), "times must be ascending");
    }

    #[test]
    fn insert_chord_event_sorted_appends_when_latest() {
        let mut events = vec![default_chord_event(MusicalTime::ZERO, Duration::beats(2))];
        let idx = insert_chord_event_sorted(
            &mut events,
            default_chord_event(MusicalTime::beats(8), Duration::beats(2)),
        );
        assert_eq!(idx, 1);
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn insert_chord_event_sorted_ties_go_after() {
        // Newest wins for ties so click-to-insert at an existing
        // time places the new event just after the original.
        let mut events = vec![default_chord_event(MusicalTime::beats(4), Duration::beats(2))];
        let idx = insert_chord_event_sorted(
            &mut events,
            default_chord_event(MusicalTime::beats(4), Duration::beats(1)),
        );
        assert_eq!(idx, 1);
        assert_eq!(events[0].duration, Duration::beats(2));
        assert_eq!(events[1].duration, Duration::beats(1));
    }

    // ---------- clamped_move / clamped_resize (CL2.x) ----------

    fn loop_4_bars_4_4() -> i64 {
        // 4 bars × 4 beats × PPQ ticks
        4 * 4 * PPQ
    }

    #[test]
    fn clamped_move_no_neighbours_clamps_to_loop_bounds() {
        // Single event 0..4 beats; loop is 16 beats long. Move +3 beats
        // → new start at beat 3.
        let events = vec![default_chord_event(MusicalTime::beats(0), Duration::beats(4))];
        let new_start = clamped_move(&events, 0, 3 * PPQ, loop_4_bars_4_4());
        assert_eq!(new_start, 3 * PPQ);
    }

    #[test]
    fn clamped_move_clamps_into_left_neighbour() {
        // Two events: 0..4, 4..8. Move idx=1 by -100 beats → clamps to
        // left_bound = 4 (where neighbour 0 ends).
        let events = vec![
            default_chord_event(MusicalTime::beats(0), Duration::beats(4)),
            default_chord_event(MusicalTime::beats(4), Duration::beats(4)),
        ];
        let new_start = clamped_move(&events, 1, -100 * PPQ, loop_4_bars_4_4());
        assert_eq!(new_start, 4 * PPQ);
    }

    #[test]
    fn clamped_move_clamps_into_right_neighbour() {
        // 0..4, 8..12. Move idx=0 by +100 beats → must stop before
        // neighbour 1's start minus its own duration (= 8 - 4 = 4).
        let events = vec![
            default_chord_event(MusicalTime::beats(0), Duration::beats(4)),
            default_chord_event(MusicalTime::beats(8), Duration::beats(4)),
        ];
        let new_start = clamped_move(&events, 0, 100 * PPQ, loop_4_bars_4_4());
        assert_eq!(new_start, 4 * PPQ);
    }

    #[test]
    fn clamped_move_clamps_at_loop_end() {
        // Single event 0..4 beats; loop is 16 beats. Move by +100 beats
        // → clamps at loop_length - duration = 12.
        let events = vec![default_chord_event(MusicalTime::beats(0), Duration::beats(4))];
        let new_start = clamped_move(&events, 0, 100 * PPQ, loop_4_bars_4_4());
        assert_eq!(new_start, 12 * PPQ);
    }

    #[test]
    fn clamped_move_snaps_sub_beat_deltas_down() {
        let events = vec![default_chord_event(MusicalTime::beats(0), Duration::beats(4))];
        // +1.5 beats → snaps to +1 beat (floor).
        let new_start = clamped_move(&events, 0, PPQ + PPQ / 2, loop_4_bars_4_4());
        assert_eq!(new_start, PPQ);
    }

    #[test]
    fn clamped_resize_clamps_into_right_neighbour() {
        let events = vec![
            default_chord_event(MusicalTime::beats(0), Duration::beats(2)),
            default_chord_event(MusicalTime::beats(4), Duration::beats(2)),
        ];
        // Resize idx=0 by +100 beats → clamps to right_bound = 4.
        // new_duration = 4 - 0 = 4.
        let new_duration = clamped_resize(&events, 0, 100 * PPQ, loop_4_bars_4_4());
        assert_eq!(new_duration, 4 * PPQ);
    }

    #[test]
    fn clamped_resize_clamps_at_loop_end() {
        let events = vec![default_chord_event(MusicalTime::beats(12), Duration::beats(2))];
        // Resize last event by +100 → loop_length = 16, start=12, max
        // new_end = 16. new_duration = 16 - 12 = 4.
        let new_duration = clamped_resize(&events, 0, 100 * PPQ, loop_4_bars_4_4());
        assert_eq!(new_duration, 4 * PPQ);
    }

    #[test]
    fn clamped_resize_min_duration_is_one_beat() {
        let events = vec![default_chord_event(MusicalTime::beats(0), Duration::beats(4))];
        // Resize by -100 beats → clamps to MIN_EVENT_TICKS (= PPQ).
        let new_duration = clamped_resize(&events, 0, -100 * PPQ, loop_4_bars_4_4());
        assert_eq!(new_duration, MIN_EVENT_TICKS);
    }

    #[test]
    fn clamped_resize_snaps_sub_beat_deltas_down() {
        let events = vec![default_chord_event(MusicalTime::beats(0), Duration::beats(2))];
        // Resize by +1.5 beats → new_end = 3.5 → snaps to 3.
        // new_duration = 3 - 0 = 3 beats.
        let new_duration = clamped_resize(&events, 0, PPQ + PPQ / 2, loop_4_bars_4_4());
        assert_eq!(new_duration, 3 * PPQ);
    }

    #[test]
    fn clamped_move_missing_idx_is_zero() {
        let events: Vec<ChordEvent> = Vec::new();
        assert_eq!(clamped_move(&events, 0, 100, 16 * PPQ), 0);
    }

    #[test]
    fn clamped_resize_missing_idx_is_zero() {
        let events: Vec<ChordEvent> = Vec::new();
        assert_eq!(clamped_resize(&events, 0, 100, 16 * PPQ), 0);
    }
}
