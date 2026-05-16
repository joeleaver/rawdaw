//! Musical and sample time.
//!
//! - [`MusicalTime`] is integer ticks at a fixed PPQ (parts per quarter note),
//!   for sample-accurate timing of MIDI events without floating-point drift.
//! - [`SampleTime`] is sample frames in the engine's clock.
//! - [`Duration`] wraps `MusicalTime` to distinguish "a length of time" from
//!   "a position in time" at the type level.
//! - [`BarRange`] is an inclusive-start, exclusive-end bar range used by
//!   section-level scheduling (variant ranges, chord-loop ranges).

use serde::{Deserialize, Serialize};

/// Pulses per quarter note. 960 gives sub-tick resolution at common subdivisions
/// (3, 4, 5, 6, 8, 12, 16, 32) and is the de-facto standard in pro DAWs.
pub const PPQ: i64 = 960;

/// Musical time in PPQ ticks. Signed so it can represent relative offsets.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct MusicalTime(pub i64);

impl MusicalTime {
    pub const ZERO: Self = Self(0);

    pub const fn ticks(t: i64) -> Self {
        Self(t)
    }

    pub const fn beats(b: i64) -> Self {
        Self(b * PPQ)
    }

    pub const fn quarters(q: i64) -> Self {
        Self(q * PPQ)
    }

    pub const fn bars(bars: i64, beats_per_bar: u32) -> Self {
        Self(bars * beats_per_bar as i64 * PPQ)
    }

    pub const fn as_ticks(self) -> i64 {
        self.0
    }

    pub const fn as_beats_f64(self) -> f64 {
        self.0 as f64 / PPQ as f64
    }
}

impl core::ops::Add for MusicalTime {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl core::ops::Sub for MusicalTime {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl core::ops::Add<Duration> for MusicalTime {
    type Output = Self;
    fn add(self, rhs: Duration) -> Self {
        Self(self.0 + rhs.0.0)
    }
}

/// A length of musical time. Newtype over [`MusicalTime`] to distinguish
/// "duration" from "position" at the type level.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Duration(pub MusicalTime);

impl Duration {
    pub const ZERO: Self = Self(MusicalTime::ZERO);

    pub const fn ticks(t: i64) -> Self {
        Self(MusicalTime::ticks(t))
    }

    pub const fn beats(b: i64) -> Self {
        Self(MusicalTime::beats(b))
    }

    pub const fn bars(bars: i64, beats_per_bar: u32) -> Self {
        Self(MusicalTime::beats(bars * beats_per_bar as i64))
    }

    pub const fn as_ticks(self) -> i64 {
        self.0.0
    }
}

/// Absolute time in audio sample frames. `u64` is more than enough at any
/// realistic sample rate for any realistic playback duration.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct SampleTime(pub u64);

impl SampleTime {
    pub const ZERO: Self = Self(0);

    pub const fn samples(s: u64) -> Self {
        Self(s)
    }

    pub const fn as_samples(self) -> u64 {
        self.0
    }
}

/// Half-open bar range `[start, end)`. Used for section-level scheduling
/// (variant schedules, chord-loop placement).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BarRange {
    pub start: u32,
    pub end: u32,
}

impl BarRange {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }

    pub const fn contains(self, bar: u32) -> bool {
        bar >= self.start && bar < self.end
    }
}
