//! Patterns: abstract part definitions referenced by activations.
//!
//! A `Pattern` is sum-typed by kind (`Pitched` or `Drum`) and owns a flat set
//! of named variants. Each variant body is independent (variants do not
//! inherit), but shared metadata (length, voice list) lives at the parent.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::chord::ChordDegree;
use crate::id::{NoteId, PatternId, VariantId};
use crate::pitch::{Octave, PitchClass, U7};
use crate::scale::ScaleDegree;
use crate::time::{Duration, MusicalTime};

// ---------- Pattern wrapper ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub id: PatternId,
    pub name: String,
    pub default_variant: VariantId,
    pub body: PatternBody,
}

/// A pattern's kind plus its variants. The metadata varies by kind, hence
/// the sum at this level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PatternBody {
    Pitched(PitchedPatternBody),
    Drum(DrumPatternBody),
}

// ---------- Pitched patterns ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PitchedPatternBody {
    pub metadata: PitchedPatternMetadata,
    pub variants: BTreeMap<VariantId, Vec<PitchedEvent>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PitchedPatternMetadata {
    pub length: Duration,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PitchedEvent {
    pub note_id: NoteId,
    pub time: MusicalTime,
    pub duration: Duration,
    pub velocity: U7,
    pub articulation: Option<ArticulationTag>,
    pub humanization: EventHumanization,
    pub spec: PitchSpec,
}

/// How a pitched event's pitch is specified.
///
/// `Scale` and `Chord` derive their pitch from the section's scale or current
/// chord. `Absolute` is a fixed pitch class + octave (e.g. for written melodies
/// where octave is structural). `Chromatic` is relative to the previous event
/// in the variant body (e.g. passing tones, ornaments). `Rest` is a positional
/// gap that participates in the pattern's grid but emits no MIDI.
///
/// The variant names omit "Degree" because the inner field already carries it
/// (`degree: ScaleDegree`, `degree: ChordDegree`). Earlier names like
/// `PitchSpec::ChordDegree { degree: ChordDegree, ... }` were confusing to
/// read in serialized output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PitchSpec {
    Scale {
        degree: ScaleDegree,
        octave: OctaveSpec,
    },
    Chord {
        degree: ChordDegree,
        octave: OctaveSpec,
    },
    Absolute {
        pitch_class: PitchClass,
        octave: Octave,
    },
    Chromatic {
        semitones_from_prev: i8,
    },
    Rest,
}

/// How to pick an octave for a degree-based event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum OctaveSpec {
    /// Voice-lead minimal-motion from the previous event (the common default).
    #[default]
    Nearest,
    /// Pin to a specific octave.
    Anchored(Octave),
    /// Force the nearest pitch *above* the previous event.
    UpFromPrev,
    /// Force the nearest pitch *below* the previous event.
    DownFromPrev,
    /// Use the track role's default register (resets octave bias).
    RelativeToRole,
}

/// Open-string articulation tag, mapped to realization details by the
/// instrument or drum kit. Common values: "ghost", "accent", "flam", "roll",
/// "open", "closed", "rim", "bell", "edge", "bow", "staccato", "legato",
/// "tenuto". Free-form to keep the data model out of the way of new tags.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArticulationTag(pub String);

impl ArticulationTag {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Per-event humanization deltas baked into the event. Pattern-wide and
/// activation-wide humanization (random jitter from a seed, swing, accent
/// curves) live elsewhere; these are user-pinned overrides for one event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct EventHumanization {
    /// Pinned timing offset in ticks, applied at realization.
    pub timing_offset_ticks: i32,
    /// Pinned velocity adjustment in U7 units, signed.
    pub velocity_offset: i16,
}

// ---------- Drum patterns ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrumPatternBody {
    pub metadata: DrumPatternMetadata,
    pub variants: BTreeMap<VariantId, Vec<DrumEvent>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrumPatternMetadata {
    pub length: Duration,
    /// Voices addressed by this pattern. All variants share this voice list;
    /// removing a voice drops events referencing it across every variant.
    pub voices: Vec<DrumVoice>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrumEvent {
    pub note_id: NoteId,
    pub time: MusicalTime,
    pub duration: Duration,
    pub voice: DrumVoice,
    pub velocity: U7,
    pub articulation: Option<ArticulationTag>,
    pub humanization: EventHumanization,
}

/// A symbolic drum voice. Hybrid vocabulary: a fixed enum for the common
/// pieces (so step-editor lanes have stable identity and icons), plus an open
/// `Extra` namespace for kit-specific extras.
///
/// `Default` resolves to `Kick` — the most universally-present voice in a
/// kit. The default lets `DrumVoice` participate as a component-prop type
/// in the rawdaw-app pattern editor (Rinch's `#[component]` macro requires
/// every prop to be `Default`); it has no semantic significance outside that.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DrumVoice {
    #[default]
    Kick,
    Snare,
    SnareRim,
    ClosedHat,
    OpenHat,
    PedalHat,
    TomLow,
    TomMid,
    TomHigh,
    Crash,
    Ride,
    RideBell,
    Clap,
    Cowbell,
    /// Kit-specific voice (e.g. "808.sub", "shaker", "tambourine"). Kits
    /// declare which extras they support; events referencing unsupported
    /// extras realize as silent with a UI warning.
    Extra(String),
}

impl DrumVoice {
    pub fn extra(name: impl Into<String>) -> Self {
        Self::Extra(name.into())
    }
}

// ---------- Helpers ----------

impl Pattern {
    pub fn pitched(
        id: PatternId,
        name: impl Into<String>,
        length: Duration,
        default_variant: VariantId,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            default_variant,
            body: PatternBody::Pitched(PitchedPatternBody {
                metadata: PitchedPatternMetadata { length },
                variants: BTreeMap::new(),
            }),
        }
    }

    pub fn drum(
        id: PatternId,
        name: impl Into<String>,
        length: Duration,
        voices: Vec<DrumVoice>,
        default_variant: VariantId,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            default_variant,
            body: PatternBody::Drum(DrumPatternBody {
                metadata: DrumPatternMetadata { length, voices },
                variants: BTreeMap::new(),
            }),
        }
    }

    pub fn length(&self) -> Duration {
        match &self.body {
            PatternBody::Pitched(p) => p.metadata.length,
            PatternBody::Drum(d) => d.metadata.length,
        }
    }
}

/// Convenience constructor for an absolute pitched event.
impl PitchedEvent {
    pub fn absolute(
        note_id: NoteId,
        time: MusicalTime,
        duration: Duration,
        velocity: U7,
        pitch_class: PitchClass,
        octave: Octave,
    ) -> Self {
        Self {
            note_id,
            time,
            duration,
            velocity,
            articulation: None,
            humanization: EventHumanization::default(),
            spec: PitchSpec::Absolute {
                pitch_class,
                octave,
            },
        }
    }

}
