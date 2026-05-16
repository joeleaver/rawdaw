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
        let bpm = self
            .bpm_events
            .first()
            .map(|e| e.bpm)
            .unwrap_or(120.0);
        // ticks / PPQ = quarter-notes
        // quarter-notes * 60 / BPM = seconds
        // seconds * sample_rate = samples
        let quarter_notes = time.as_ticks() as f64 / PPQ as f64;
        let seconds = quarter_notes * 60.0 / bpm;
        let samples = (seconds * sample_rate as f64).round() as u64;
        SampleTime::samples(samples)
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
