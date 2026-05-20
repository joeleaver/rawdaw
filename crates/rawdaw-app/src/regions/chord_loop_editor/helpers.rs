//! Pure helpers for the chord-loop editor: musical-time conversions,
//! beat snapping, default-event factory, sorted insert. Pulled out
//! of the component modules so they have unit tests against
//! pure-function contracts.

use rawdaw_model::chord::{ChordEvent, ChordQuality, ChordSpec, ChordSuffix, RomanDegree};
use rawdaw_model::time::{Duration, MusicalTime, PPQ};

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
}
