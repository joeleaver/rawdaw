//! Project-meta display helpers.
//!
//! Format model `Project` fields as the strings the top bar and inspector
//! render. Lives here (not in `rawdaw-model`) because formatting is a
//! presentation concern — model crate stays free of display logic per
//! CLAUDE.md rule 1.

use rawdaw_model::tempo::TempoMap;
use rawdaw_model::time::MusicalTime;

/// Current BPM in effect at the start of the arrangement. Reads the
/// first BPM event in the map; returns the model's documented fallback
/// of `120.0` when no events are present.
///
/// Wraps `TempoMap`'s currently-private `first_bpm` accessor — the model
/// could expose it directly later, but for now mirroring the logic
/// here keeps the model surface conservative.
pub fn current_bpm(tempo_map: &TempoMap) -> f64 {
    tempo_map
        .bpm_events
        .first()
        .map(|e| e.bpm)
        .unwrap_or(120.0)
}

/// `"4/4"`, `"3/4"`, etc. — beats-per-bar at musical-time zero followed
/// by the beat-unit denominator. The model carries beat unit as a
/// `BeatUnit` enum on each time-signature event; round-1's tempo map
/// uses quarter notes so we surface `4` for the denominator until the
/// model grows mid-arrangement beat-unit changes.
pub fn time_signature_label(tempo_map: &TempoMap) -> String {
    let beats = tempo_map.beats_per_bar_at(MusicalTime::ZERO);
    let denom = tempo_map
        .time_signature_events
        .first()
        .map(|e| beat_unit_denominator(e.beat_unit))
        .unwrap_or(4);
    format!("{beats}/{denom}")
}

fn beat_unit_denominator(unit: rawdaw_model::tempo::BeatUnit) -> u8 {
    use rawdaw_model::tempo::BeatUnit;
    match unit {
        BeatUnit::Whole => 1,
        BeatUnit::Half => 2,
        BeatUnit::Quarter => 4,
        BeatUnit::Eighth => 8,
        BeatUnit::Sixteenth => 16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_model::tempo::BeatUnit;

    #[test]
    fn current_bpm_falls_back_to_120() {
        let tm = TempoMap {
            bpm_events: vec![],
            time_signature_events: vec![],
        };
        assert!((current_bpm(&tm) - 120.0).abs() < f64::EPSILON);
    }

    #[test]
    fn time_signature_label_renders_four_four() {
        let tm = TempoMap::constant(120.0, 4, BeatUnit::Quarter);
        assert_eq!(time_signature_label(&tm), "4/4");
    }
}
