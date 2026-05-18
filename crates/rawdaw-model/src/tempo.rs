//! Tempo map: musical-time ↔ sample-time conversion.
//!
//! Stub for now. The engine crate will own the high-performance conversion
//! routines and immutable snapshots passed to the RT audio thread. The model
//! layer owns the *editable* tempo map: BPM events (constant or ramped) and
//! time-signature events along the timeline.

use serde::{Deserialize, Serialize};

use crate::time::{MusicalTime, PPQ, SampleTime};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TempoMap {
    pub bpm_events: Vec<BpmEvent>,
    pub time_signature_events: Vec<TimeSignatureEvent>,
}

impl TempoMap {
    pub fn constant(bpm: f64, beats_per_bar: u8, beat_unit: BeatUnit) -> Self {
        Self {
            bpm_events: vec![BpmEvent {
                time: MusicalTime::ZERO,
                bpm,
                ramp: BpmRamp::Constant,
            }],
            time_signature_events: vec![TimeSignatureEvent {
                time: MusicalTime::ZERO,
                beats_per_bar,
                beat_unit,
            }],
        }
    }
}

impl Default for TempoMap {
    fn default() -> Self {
        Self::constant(120.0, 4, BeatUnit::Quarter)
    }
}

impl TempoMap {
    /// Convert musical time to sample time at the given sample rate.
    ///
    /// **v1 limitation:** assumes a single constant BPM (the first `BpmEvent`
    /// in `bpm_events`). Ramped BPM events and mid-arrangement BPM changes
    /// are not yet honored. Time-signature events also aren't relevant to
    /// this conversion (they affect bar counting, not tick-to-sample math).
    pub fn musical_to_sample(&self, time: MusicalTime, sample_rate: u32) -> SampleTime {
        let bpm = self.first_bpm();
        // ticks / PPQ = quarter-notes
        // quarter-notes * 60 / BPM = seconds
        // seconds * sample_rate = samples
        let quarter_notes = time.as_ticks() as f64 / PPQ as f64;
        let seconds = quarter_notes * 60.0 / bpm;
        let samples = (seconds * sample_rate as f64).round() as u64;
        SampleTime::samples(samples)
    }

    /// Convert sample time to musical time at the given sample rate.
    ///
    /// Inverse of [`Self::musical_to_sample`] under the same constant-BPM
    /// assumption. Useful for mapping the engine's transport position
    /// (`SampleTime`) back to bars / beats for UI display.
    ///
    /// **v1 limitation:** matches [`Self::musical_to_sample`] — single
    /// constant BPM, no ramps. Round-trip
    /// `sample_to_musical(musical_to_sample(t))` is exact at tick
    /// boundaries that align with integer-sample positions and within
    /// one tick otherwise (both ends round once).
    pub fn sample_to_musical(&self, time: SampleTime, sample_rate: u32) -> MusicalTime {
        if sample_rate == 0 {
            return MusicalTime::ZERO;
        }
        let bpm = self.first_bpm();
        // samples / sample_rate = seconds
        // seconds * BPM / 60 = quarter-notes
        // quarter-notes * PPQ = ticks
        let seconds = time.as_samples() as f64 / sample_rate as f64;
        let quarter_notes = seconds * bpm / 60.0;
        let ticks = (quarter_notes * PPQ as f64).round() as i64;
        MusicalTime::ticks(ticks)
    }

    fn first_bpm(&self) -> f64 {
        self.bpm_events.first().map(|e| e.bpm).unwrap_or(120.0)
    }

    /// Find the beats-per-bar in effect at the given musical time.
    /// Falls back to 4 if no time-signature events are present.
    pub fn beats_per_bar_at(&self, time: MusicalTime) -> u32 {
        self.time_signature_events
            .iter()
            .rev()
            .find(|e| e.time <= time)
            .map(|e| e.beats_per_bar as u32)
            .unwrap_or(4)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BpmEvent {
    pub time: MusicalTime,
    pub bpm: f64,
    pub ramp: BpmRamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BpmRamp {
    /// Hold this BPM until the next event.
    #[default]
    Constant,
    /// Linearly ramp to the next event's BPM.
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeSignatureEvent {
    pub time: MusicalTime,
    pub beats_per_bar: u8,
    pub beat_unit: BeatUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BeatUnit {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Duration;

    const SAMPLE_RATE: u32 = 48_000;

    fn map() -> TempoMap {
        TempoMap::constant(120.0, 4, BeatUnit::Quarter)
    }

    #[test]
    fn musical_to_sample_round_trips_at_beat_boundaries() {
        let tm = map();
        // At 120 BPM, one quarter note = 0.5s = 24_000 samples.
        for beats in 0..16 {
            let mt = MusicalTime::beats(beats);
            let st = tm.musical_to_sample(mt, SAMPLE_RATE);
            assert_eq!(st.as_samples(), beats as u64 * 24_000);
            let back = tm.sample_to_musical(st, SAMPLE_RATE);
            assert_eq!(back, mt, "round-trip exact at beat {beats}");
        }
    }

    #[test]
    fn sample_to_musical_zero_is_zero() {
        let tm = map();
        assert_eq!(
            tm.sample_to_musical(SampleTime::ZERO, SAMPLE_RATE),
            MusicalTime::ZERO,
        );
    }

    #[test]
    fn sample_to_musical_handles_zero_sample_rate() {
        // Defensive: avoid divide-by-zero. Falls back to MusicalTime::ZERO.
        let tm = map();
        assert_eq!(
            tm.sample_to_musical(SampleTime::samples(48_000), 0),
            MusicalTime::ZERO,
        );
    }

    #[test]
    fn sample_to_musical_midway_through_beat() {
        // 12_000 samples at 48 kHz = 0.25s = half a quarter at 120 BPM.
        let tm = map();
        let mt = tm.sample_to_musical(SampleTime::samples(12_000), SAMPLE_RATE);
        assert_eq!(mt, MusicalTime::ticks(PPQ / 2));
    }

    #[test]
    fn sample_to_musical_bars_against_default_time_signature() {
        // One bar at 4/4 120 BPM = 4 beats = 2s = 96_000 samples.
        let tm = map();
        let mt = tm.sample_to_musical(SampleTime::samples(96_000), SAMPLE_RATE);
        let bpb = tm.beats_per_bar_at(MusicalTime::ZERO);
        let bars = mt.as_beats_f64() / bpb as f64;
        assert!((bars - 1.0).abs() < 1e-9, "expected exactly 1 bar, got {bars}");
        // Type-system sanity: Duration::bars expresses the same length.
        assert_eq!(MusicalTime::ZERO + Duration::bars(1, bpb), mt);
    }
}
