//! Scales, modes, and scale degrees.

use serde::{Deserialize, Serialize};

use crate::pitch::{Accidental, PitchClass};

/// A tonal scale: a tonic pitch class and a mode.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Scale {
    pub tonic: PitchClass,
    pub mode: Mode,
}

impl Scale {
    pub fn new(tonic: PitchClass, mode: Mode) -> Self {
        Self { tonic, mode }
    }

    pub fn major(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::Ionian)
    }

    pub fn natural_minor(tonic: PitchClass) -> Self {
        Self::new(tonic, Mode::Aeolian)
    }
}

/// Mode / scale type. `Custom` is the escape hatch.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    // Diatonic modes
    Ionian,     // major
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian,    // natural minor
    Locrian,

    // Other common scales
    HarmonicMinor,
    MelodicMinor,
    PhrygianDominant,
    Lydian7,    // a.k.a. acoustic / Lydian dominant
    Altered,    // melodic minor mode 7

    // Symmetric / "color" scales
    MajorPentatonic,
    MinorPentatonic,
    Blues,
    WholeTone,
    Chromatic,

    /// Intervals in semitones from the tonic, ascending. Must be sorted and
    /// strictly increasing within [0, 11].
    Custom { intervals: Vec<u8> },
}

impl Mode {
    /// Semitones from the tonic for each scale degree (1-indexed externally;
    /// returned slice is 0-indexed).
    pub fn intervals(&self) -> &[u8] {
        match self {
            Self::Ionian => &[0, 2, 4, 5, 7, 9, 11],
            Self::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Self::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Self::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            Self::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            Self::Aeolian => &[0, 2, 3, 5, 7, 8, 10],
            Self::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            Self::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            Self::MelodicMinor => &[0, 2, 3, 5, 7, 9, 11],
            Self::PhrygianDominant => &[0, 1, 4, 5, 7, 8, 10],
            Self::Lydian7 => &[0, 2, 4, 6, 7, 9, 10],
            Self::Altered => &[0, 1, 3, 4, 6, 8, 10],
            Self::MajorPentatonic => &[0, 2, 4, 7, 9],
            Self::MinorPentatonic => &[0, 3, 5, 7, 10],
            Self::Blues => &[0, 3, 5, 6, 7, 10],
            Self::WholeTone => &[0, 2, 4, 6, 8, 10],
            Self::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            Self::Custom { intervals } => intervals,
        }
    }

    /// Number of degrees in the scale (e.g. 7 for diatonic, 5 for pentatonic).
    pub fn degree_count(&self) -> usize {
        self.intervals().len()
    }
}

/// A degree within a scale, with optional accidental.
///
/// `degree` is 1-indexed (1..=degree_count). `Natural` accidental means the
/// scale's diatonic interval for this degree; `Flat`/`Sharp` shift it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScaleDegree {
    pub degree: u8,
    pub accidental: Accidental,
}

impl ScaleDegree {
    pub fn new(degree: u8) -> Self {
        Self {
            degree,
            accidental: Accidental::Natural,
        }
    }

    pub fn with_accidental(degree: u8, accidental: Accidental) -> Self {
        Self { degree, accidental }
    }
}
