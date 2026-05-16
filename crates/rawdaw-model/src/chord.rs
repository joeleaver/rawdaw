//! Chord vocabulary, events, and loops.
//!
//! See `docs/design/chord-loops.md` for the full design. Key points:
//!
//! - Functional (Roman-numeral) representation is canonical; `Absolute` is the
//!   escape hatch for non-functional chords.
//! - `in_key` on a `Functional` event handles secondary dominants, modal
//!   interchange, and brief tonicization with one mechanism.
//! - Voicing is NOT stored on chord events — it belongs to realization
//!   (see [`crate::activation::VoicingStrategy`]).
//! - Bass is specified independently of chord (slash chords/inversions).

use serde::{Deserialize, Serialize};

use crate::id::ChordLoopId;
use crate::pitch::{Accidental, PitchClass};
use crate::scale::{Scale, ScaleDegree};
use crate::time::{Duration, MusicalTime};

// ---------- Roman degrees ----------

/// Roman-numeral degree with optional accidental displacement.
///
/// Case convention (I vs. i) is render-only; the `ChordQuality` on the event
/// determines the actual quality, so we don't encode case here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RomanDegree {
    I,
    II,
    III,
    IV,
    V,
    VI,
    VII,
    FlatII,
    FlatIII,
    FlatV,
    FlatVI,
    FlatVII,
    SharpI,
    SharpII,
    SharpIV,
    SharpV,
    SharpVI,
}

// ---------- Chord quality / extensions / alterations ----------

/// Base chord quality. `Custom` accepts arbitrary intervals from the root.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChordQuality {
    Major,
    Minor,
    Diminished,
    Augmented,

    Major7,
    Minor7,
    Dominant7,
    Diminished7,
    HalfDiminished7, // m7♭5
    MinorMajor7,
    AugmentedDom7,

    Major6,
    Minor6,

    Sus2,
    Sus4,
    Sus7, // 7sus4 typically
    Sus9,

    Power, // root + 5

    /// Semitones above the root for each chord tone, ascending and unique.
    /// Root (0) is implicit and need not be listed.
    Custom { intervals: Vec<u8> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Extension {
    /// Major 9th added without forcing a 7th (e.g. Cadd9).
    Add9,
    /// 11th added without a 7th.
    Add11,
    /// 13th added without a 7th.
    Add13,
    /// 9th, implies the 7th and the 9th.
    Ninth,
    /// 11th, implies the 7th, 9th, and 11th.
    Eleventh,
    /// 13th, implies the 7th, 9th, 11th, and 13th.
    Thirteenth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Alteration {
    Flat5,
    Sharp5,
    Flat9,
    Sharp9,
    Sharp11,
    Flat13,
    NoFifth,
    NoThird,
}

/// Shared upper-structure description for both functional and absolute chords.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChordSuffix {
    pub quality: ChordQuality,
    pub extensions: Vec<Extension>,
    pub alterations: Vec<Alteration>,
}

impl ChordSuffix {
    pub fn new(quality: ChordQuality) -> Self {
        Self {
            quality,
            extensions: Vec::new(),
            alterations: Vec::new(),
        }
    }
}

// ---------- Chord spec ----------

/// A chord — either functional in a (possibly overridden) key, or absolute.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChordSpec {
    Functional {
        roman: RomanDegree,
        suffix: ChordSuffix,
        /// If `Some`, interpret `roman` in this scale instead of the section's
        /// scale. Handles secondary dominants, modal interchange, tonicization,
        /// and chord-level modulation with one mechanism.
        in_key: Option<Scale>,
    },
    Absolute {
        root: PitchClass,
        suffix: ChordSuffix,
    },
}

// ---------- Chord degree (used in pitched pattern events) ----------

/// Position within a chord, plus an accidental override.
///
/// `accidental: Natural` means "use the chord's intrinsic interval at this
/// step." `Flat`/`Sharp` force a chromatic alteration relative to that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChordDegree {
    pub step: ChordStep,
    pub accidental: Accidental,
}

impl ChordDegree {
    pub fn new(step: ChordStep) -> Self {
        Self {
            step,
            accidental: Accidental::Natural,
        }
    }

    pub fn with_accidental(step: ChordStep, accidental: Accidental) -> Self {
        Self { step, accidental }
    }
}

/// Chord-tone positions. Even-numbered steps (Second, Fourth, Sixth) are for
/// sus / 6 chords; the realization treats them as chord tones if the quality
/// includes them, otherwise as scale tones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChordStep {
    Root,
    Second,
    Third,
    Fourth,
    Fifth,
    Sixth,
    Seventh,
    Ninth,
    Eleventh,
    Thirteenth,
}

// ---------- Bass spec ----------

/// Bass note for a chord event. If `None` on a `ChordEvent`, bass is the
/// chord's root in root position.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BassSpec {
    /// Standard inversion (1st = 3rd in bass, 2nd = 5th in bass, ...).
    Inversion(u8),
    /// A chord tone in the bass (root, 3rd, 5th, 7th, ...).
    ChordDegree(ChordDegree),
    /// A scale tone in the bass that is not in the chord (e.g. Cmaj/D).
    ScaleDegree(ScaleDegree),
    /// An explicit pitch class (e.g. Cmaj/D♯).
    Absolute(PitchClass),
}

// ---------- Cadence / annotation ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CadenceTag {
    PerfectAuthentic,
    ImperfectAuthentic,
    Half,
    Plagal,
    Deceptive,
    Phrygian,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Annotation {
    pub cadence: Option<CadenceTag>,
    pub comment: Option<String>,
}

// ---------- Chord event and loop ----------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChordEvent {
    pub time: MusicalTime,
    pub duration: Duration,
    pub chord: ChordSpec,
    pub bass: Option<BassSpec>,
    pub annotation: Option<Annotation>,
}

/// A named, reusable harmonic sequence.
///
/// `key` is usually `None` — the loop floats with the section's scale.
/// Set it only when the loop is rigidly tied to a specific tonal center.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChordLoop {
    pub id: ChordLoopId,
    pub name: String,
    pub length: Duration,
    pub key: Option<Scale>,
    pub events: Vec<ChordEvent>,
}
