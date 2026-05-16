//! Pitch primitives: pitch classes, MIDI notes, octaves, accidentals,
//! and the 7-bit unsigned values MIDI uses for velocity and CC values.

use serde::{Deserialize, Serialize};

/// The twelve pitch classes. Sharps preferred; render as flats by context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum PitchClass {
    C = 0,
    CSharp = 1,
    D = 2,
    DSharp = 3,
    E = 4,
    F = 5,
    FSharp = 6,
    G = 7,
    GSharp = 8,
    A = 9,
    ASharp = 10,
    B = 11,
}

impl PitchClass {
    pub const fn semitones_from_c(self) -> u8 {
        self as u8
    }

    pub const fn from_semitones_mod12(n: i32) -> Self {
        match n.rem_euclid(12) {
            0 => Self::C,
            1 => Self::CSharp,
            2 => Self::D,
            3 => Self::DSharp,
            4 => Self::E,
            5 => Self::F,
            6 => Self::FSharp,
            7 => Self::G,
            8 => Self::GSharp,
            9 => Self::A,
            10 => Self::ASharp,
            11 => Self::B,
            _ => unreachable!(),
        }
    }
}

/// A MIDI note number 0–127. Middle C is 60 (`octave 4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MidiNote(u8);

impl MidiNote {
    pub const MIN: Self = Self(0);
    pub const MAX: Self = Self(127);
    pub const MIDDLE_C: Self = Self(60);

    /// Returns `None` if `n > 127`.
    pub const fn new(n: u8) -> Option<Self> {
        if n <= 127 { Some(Self(n)) } else { None }
    }

    pub const fn from_pitch_octave(pc: PitchClass, octave: Octave) -> Option<Self> {
        // MIDI octave numbering: C4 = MIDI 60, so MIDI = (octave + 1) * 12 + pc.
        let n = (octave.0 as i32 + 1) * 12 + pc.semitones_from_c() as i32;
        if n >= 0 && n <= 127 {
            Some(Self(n as u8))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub const fn pitch_class(self) -> PitchClass {
        PitchClass::from_semitones_mod12(self.0 as i32)
    }

    pub const fn octave(self) -> Octave {
        Octave((self.0 as i8 / 12) - 1)
    }
}

/// Octave number. Middle C is `Octave(4)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Octave(pub i8);

impl Octave {
    pub const MIDDLE: Self = Self(4);
}

/// Accidental modifier. `Natural` means "no accidental" in the absolute sense,
/// and for [`crate::chord::ChordDegree`] it means "as the chord's quality
/// intrinsically defines this step" — i.e. take the diatonic interval, not
/// literal natural.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Accidental {
    DoubleFlat,
    Flat,
    #[default]
    Natural,
    Sharp,
    DoubleSharp,
}

impl Accidental {
    pub const fn semitone_offset(self) -> i8 {
        match self {
            Self::DoubleFlat => -2,
            Self::Flat => -1,
            Self::Natural => 0,
            Self::Sharp => 1,
            Self::DoubleSharp => 2,
        }
    }
}

/// 7-bit unsigned. Used for MIDI 1.0 velocity and CC values. MIDI 2.0 widens
/// these but we keep `U7` for compactness in stored pattern data; widening to
/// 16-bit precision happens at realization or output time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct U7(u8);

impl U7 {
    pub const MIN: Self = Self(0);
    pub const MAX: Self = Self(127);
    pub const HALF: Self = Self(64);

    pub const fn new(n: u8) -> Option<Self> {
        if n <= 127 { Some(Self(n)) } else { None }
    }

    /// Saturating constructor.
    pub const fn clamp(n: u8) -> Self {
        if n > 127 { Self(127) } else { Self(n) }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

impl Default for U7 {
    fn default() -> Self {
        Self::HALF
    }
}
