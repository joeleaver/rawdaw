//! Activations: the per-track-per-section configuration that places a pattern
//! reference into a section. Also holds realization params (humanization,
//! velocity curves, voicing) and per-note overrides.

use serde::{Deserialize, Serialize};

use crate::id::{ActivationEntryId, NoteId, NoteOverrideId, PatternId, VariantId};
use crate::pitch::{MidiNote, U7};
use crate::time::{BarRange, Duration};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivationEntry {
    pub id: ActivationEntryId,
    /// `None` means the track is placed but plays no pattern in this section.
    pub pattern_ref: Option<PatternId>,
    /// Per-bar-range variant pin. Uncovered ranges fall back to the
    /// referenced pattern's `default_variant`.
    pub variant_schedule: Vec<(BarRange, VariantId)>,
    pub realization: RealizationParams,
    pub per_note_overrides: Vec<NoteOverride>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RealizationParams {
    pub voicing: Option<VoicingStrategy>,
    pub velocity_curve: Option<VelocityCurve>,
    pub humanization: Humanization,
    /// Seed for deterministic humanization. New seed = re-roll.
    pub seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VoicingStrategy {
    TriadClose,
    TriadOpen,
    FourWayClose,
    Drop2,
    Drop3,
    Shell,
    Rootless,
    Power,
}

/// A velocity shaping curve applied across the activation. Kept simple for
/// v1; richer shapes (multi-point envelopes) can be added later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VelocityCurve {
    /// Linear scale from `start` to `end` U7 values across the activation.
    Linear { start: U7, end: U7 },
    /// Accent every Nth event at the given velocity.
    AccentEveryN { n: u32, accent_velocity: U7 },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Humanization {
    /// Velocity jitter range, ±value U7 units.
    pub velocity_jitter: u8,
    /// Timing jitter range, ±value ticks.
    pub timing_jitter_ticks: u32,
    /// Swing amount in 0.0..=1.0. 0.0 = straight, 0.5 = full triplet swing.
    pub swing: f32,
}

// ---------- Per-note overrides ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteOverride {
    pub id: NoteOverrideId,
    /// Target pattern event by stable `NoteId`. If the event is deleted, this
    /// override becomes an orphan: realization ignores it and the UI warns.
    pub target: NoteId,
    pub transform: OverrideTransform,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OverrideTransform {
    /// Pin to a specific MIDI note number (bypass degree resolution).
    PinPitch(MidiNote),
    /// Pin velocity.
    PinVelocity(U7),
    /// Override timing by tick offset relative to the event's pattern position.
    PinTiming(i32),
    /// Override duration.
    PinDuration(Duration),
    /// Skip this event entirely.
    Mute,
}
