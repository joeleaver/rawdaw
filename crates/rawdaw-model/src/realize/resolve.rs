//! Pitch / chord / octave resolution and the GM drum-map fallback.
//!
//! All functions in here are pure: same inputs → same outputs. They don't
//! touch any I/O or mutable state. The realization walker calls them.

use crate::chord::{
    Alteration, ChordDegree, ChordQuality, ChordSpec, ChordStep, ChordSuffix, RomanDegree,
};
use crate::pattern::{DrumVoice, OctaveSpec};
use crate::pitch::{Accidental, MidiNote, Octave, PitchClass};
use crate::scale::{Scale, ScaleDegree};
use crate::track::Role;

// ---------- Scale degree → pitch class ----------

/// Resolve a `ScaleDegree` against a `Scale` to an absolute `PitchClass`.
///
/// `degree.degree` is 1-indexed and wraps modulo the scale's degree count.
pub fn resolve_scale_degree(degree: ScaleDegree, scale: &Scale) -> PitchClass {
    let intervals = scale.mode.intervals();
    let zero_indexed = degree.degree.saturating_sub(1) as usize;
    let interval = intervals[zero_indexed % intervals.len()] as i32;
    let tonic = scale.tonic.semitones_from_c() as i32;
    PitchClass::from_semitones_mod12(tonic + interval + degree.accidental.semitone_offset() as i32)
}

// ---------- Roman degree → chord root ----------

/// Given a Roman degree and the key it's interpreted in, return the
/// chord's root pitch class.
pub fn resolve_chord_root(roman: RomanDegree, in_key: &Scale) -> PitchClass {
    let (step, accidental) = match roman {
        RomanDegree::I => (1, Accidental::Natural),
        RomanDegree::II => (2, Accidental::Natural),
        RomanDegree::III => (3, Accidental::Natural),
        RomanDegree::IV => (4, Accidental::Natural),
        RomanDegree::V => (5, Accidental::Natural),
        RomanDegree::VI => (6, Accidental::Natural),
        RomanDegree::VII => (7, Accidental::Natural),
        RomanDegree::FlatII => (2, Accidental::Flat),
        RomanDegree::FlatIII => (3, Accidental::Flat),
        RomanDegree::FlatV => (5, Accidental::Flat),
        RomanDegree::FlatVI => (6, Accidental::Flat),
        RomanDegree::FlatVII => (7, Accidental::Flat),
        RomanDegree::SharpI => (1, Accidental::Sharp),
        RomanDegree::SharpII => (2, Accidental::Sharp),
        RomanDegree::SharpIV => (4, Accidental::Sharp),
        RomanDegree::SharpV => (5, Accidental::Sharp),
        RomanDegree::SharpVI => (6, Accidental::Sharp),
    };
    resolve_scale_degree(
        ScaleDegree {
            degree: step,
            accidental,
        },
        in_key,
    )
}

/// Resolve a `ChordSpec` against a fallback scale to its root pitch class.
/// `Functional` chords use `in_key` when set, else the section scale.
pub fn resolve_chord_spec_root(spec: &ChordSpec, section_scale: &Scale) -> PitchClass {
    match spec {
        ChordSpec::Functional { roman, in_key, .. } => {
            let scale = in_key.as_ref().unwrap_or(section_scale);
            resolve_chord_root(*roman, scale)
        }
        ChordSpec::Absolute { root, .. } => *root,
    }
}

// ---------- Chord step → interval ----------

/// Return the semitone interval from the chord's root for the given step,
/// taking into account the chord's quality, extensions, and alterations.
///
/// Returns `None` if the step isn't represented in this chord (e.g. asking
/// for the third of a power chord, or the fifth of a chord with `NoFifth`).
pub fn chord_step_to_interval(step: ChordStep, suffix: &ChordSuffix) -> Option<i32> {
    let base = base_step_interval(step, &suffix.quality)?;
    Some(apply_alterations(step, base, &suffix.alterations))
}

fn base_step_interval(step: ChordStep, quality: &ChordQuality) -> Option<i32> {
    use ChordQuality::*;
    use ChordStep::*;
    let v: i32 = match (step, quality) {
        // Root is always 0.
        (Root, _) => 0,

        // Second (sus2 / add2 territory).
        (Second, Sus2) => 2,
        (Second, _) => return None,

        // Third — quality determines major (4) vs minor (3); sus suspends it.
        (
            Third,
            Major | Major7 | Dominant7 | AugmentedDom7 | Major6 | Augmented,
        ) => 4,
        (Third, Minor | Minor7 | Minor6 | MinorMajor7 | HalfDiminished7 | Diminished | Diminished7) => 3,
        (Third, Sus2 | Sus4 | Sus7 | Sus9 | Power) => return None,
        (Third, Custom { .. }) => return None,

        // Fourth (sus4 / add4).
        (Fourth, Sus4 | Sus7 | Sus9) => 5,
        (Fourth, _) => return None,

        // Fifth — quality determines perfect (7) vs diminished (6) vs augmented (8).
        (Fifth, Diminished | Diminished7 | HalfDiminished7) => 6,
        (Fifth, Augmented | AugmentedDom7) => 8,
        (Fifth, _) => 7,

        // Sixth (major6 / minor6 chords).
        (Sixth, Major6 | Minor6) => 9,
        (Sixth, _) => return None,

        // Seventh — quality determines major (11) vs minor (10) vs diminished (9).
        (Seventh, Major7 | MinorMajor7) => 11,
        (Seventh, Minor7 | Dominant7 | AugmentedDom7 | Sus7 | Sus9 | HalfDiminished7) => 10,
        (Seventh, Diminished7) => 9,
        (Seventh, _) => return None,

        // Extensions (9th, 11th, 13th). Realization treats these as available on
        // any chord; the user putting them in a pattern means they want them,
        // regardless of whether the chord's quality "natively" includes them.
        (Ninth, _) => 14,
        (Eleventh, _) => 17,
        (Thirteenth, _) => 21,
    };
    Some(v)
}

fn apply_alterations(step: ChordStep, base: i32, alterations: &[Alteration]) -> i32 {
    let mut out = base;
    for &alt in alterations {
        match (step, alt) {
            (ChordStep::Fifth, Alteration::Flat5) => out -= 1,
            (ChordStep::Fifth, Alteration::Sharp5) => out += 1,
            (ChordStep::Ninth, Alteration::Flat9) => out -= 1,
            (ChordStep::Ninth, Alteration::Sharp9) => out += 1,
            (ChordStep::Eleventh, Alteration::Sharp11) => out += 1,
            (ChordStep::Thirteenth, Alteration::Flat13) => out -= 1,
            _ => {}
        }
    }
    out
}

/// Apply the `accidental` on a [`ChordDegree`] after the quality+alteration
/// base. `Natural` is a no-op (use the chord's intrinsic value); `Flat`/`Sharp`
/// force a chromatic shift.
pub fn apply_chord_degree_accidental(interval: i32, accidental: Accidental) -> i32 {
    interval + accidental.semitone_offset() as i32
}

/// Resolve a [`ChordDegree`] against a resolved chord root and suffix to an
/// absolute `PitchClass`. Returns `None` if the step doesn't exist in this
/// chord shape.
pub fn resolve_chord_degree(
    degree: ChordDegree,
    root: PitchClass,
    suffix: &ChordSuffix,
) -> Option<PitchClass> {
    let interval = chord_step_to_interval(degree.step, suffix)?;
    let with_accidental = apply_chord_degree_accidental(interval, degree.accidental);
    let root_semis = root.semitones_from_c() as i32;
    Some(PitchClass::from_semitones_mod12(root_semis + with_accidental))
}

// ---------- Octave selection ----------

/// Pick a concrete `MidiNote` for a given target pitch class and `OctaveSpec`,
/// given the previous-pitch state for the track.
pub fn pick_octave(
    target: PitchClass,
    spec: OctaveSpec,
    last_pitch: Option<MidiNote>,
    role_register: MidiNote,
) -> Option<MidiNote> {
    match spec {
        OctaveSpec::Nearest => Some(nearest_to(target, last_pitch.unwrap_or(role_register))),
        OctaveSpec::Anchored(o) => MidiNote::from_pitch_octave(target, o),
        OctaveSpec::UpFromPrev => up_from(target, last_pitch.unwrap_or(role_register)),
        OctaveSpec::DownFromPrev => down_from(target, last_pitch.unwrap_or(role_register)),
        OctaveSpec::RelativeToRole => Some(nearest_to(target, role_register)),
    }
}

/// Closest MIDI note with the given pitch class to `anchor`.
pub fn nearest_to(target: PitchClass, anchor: MidiNote) -> MidiNote {
    let anchor_n = anchor.get() as i32;
    let anchor_pc = anchor.pitch_class().semitones_from_c() as i32;
    let target_pc = target.semitones_from_c() as i32;
    // Smallest signed difference in -6..=6.
    let mut diff = (target_pc - anchor_pc).rem_euclid(12);
    if diff > 6 {
        diff -= 12;
    }
    let n = (anchor_n + diff).clamp(0, 127) as u8;
    MidiNote::new(n).expect("clamped to 0..=127")
}

/// Smallest MIDI note strictly above `anchor` with the given pitch class.
pub fn up_from(target: PitchClass, anchor: MidiNote) -> Option<MidiNote> {
    let anchor_n = anchor.get() as i32;
    let anchor_pc = anchor.pitch_class().semitones_from_c() as i32;
    let target_pc = target.semitones_from_c() as i32;
    let mut diff = (target_pc - anchor_pc).rem_euclid(12);
    if diff == 0 {
        diff = 12;
    }
    let n = anchor_n + diff;
    if n <= 127 {
        MidiNote::new(n as u8)
    } else {
        None
    }
}

/// Largest MIDI note strictly below `anchor` with the given pitch class.
pub fn down_from(target: PitchClass, anchor: MidiNote) -> Option<MidiNote> {
    let anchor_n = anchor.get() as i32;
    let anchor_pc = anchor.pitch_class().semitones_from_c() as i32;
    let target_pc = target.semitones_from_c() as i32;
    let mut diff = (anchor_pc - target_pc).rem_euclid(12);
    if diff == 0 {
        diff = 12;
    }
    let n = anchor_n - diff;
    if n >= 0 {
        MidiNote::new(n as u8)
    } else {
        None
    }
}

// ---------- Role registers ----------

/// Default starting MIDI note for a role. Used when there's no prior pitch
/// (start of section / activation) and on `OctaveSpec::RelativeToRole`.
pub fn role_register(role: Role) -> MidiNote {
    let n = match role {
        Role::Bass => 40,          // E2
        Role::Voicing => 60,       // C4
        Role::Arp => 60,           // C4
        Role::Melodic => 67,       // G4
        Role::Pad => 55,           // G3
        Role::Countermelody => 64, // E4
        Role::Other => 60,         // C4
    };
    MidiNote::new(n).expect("role register fits in u7")
}

// ---------- GM drum-map fallback ----------

/// Map a `DrumVoice` to a General MIDI percussion note. Used until kits with
/// their own voice→note mappings ship in `rawdaw-drumkits`.
///
/// Returns `None` for `Extra(_)` voices and any future variants the GM map
/// doesn't cover; the realization treats those as silent with a UI warning.
pub fn gm_drum_note(voice: &DrumVoice) -> Option<MidiNote> {
    let n: u8 = match voice {
        DrumVoice::Kick => 36,
        DrumVoice::Snare => 38,
        DrumVoice::SnareRim => 37,
        DrumVoice::ClosedHat => 42,
        DrumVoice::OpenHat => 46,
        DrumVoice::PedalHat => 44,
        DrumVoice::TomLow => 41,
        DrumVoice::TomMid => 47,
        DrumVoice::TomHigh => 50,
        DrumVoice::Crash => 49,
        DrumVoice::Ride => 51,
        DrumVoice::RideBell => 53,
        DrumVoice::Clap => 39,
        DrumVoice::Cowbell => 56,
        DrumVoice::Extra(_) => return None,
    };
    MidiNote::new(n)
}

// ---------- Octave applied to an absolute pitch ----------

/// Compute the `MidiNote` for an `Absolute` pitch event.
pub fn absolute_pitch(pitch_class: PitchClass, octave: Octave) -> Option<MidiNote> {
    MidiNote::from_pitch_octave(pitch_class, octave)
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn c_major() -> Scale {
        Scale::major(PitchClass::C)
    }

    #[test]
    fn scale_degree_resolves_in_c_major() {
        let s = c_major();
        assert_eq!(resolve_scale_degree(ScaleDegree::new(1), &s), PitchClass::C);
        assert_eq!(resolve_scale_degree(ScaleDegree::new(2), &s), PitchClass::D);
        assert_eq!(resolve_scale_degree(ScaleDegree::new(3), &s), PitchClass::E);
        assert_eq!(resolve_scale_degree(ScaleDegree::new(4), &s), PitchClass::F);
        assert_eq!(resolve_scale_degree(ScaleDegree::new(5), &s), PitchClass::G);
        assert_eq!(resolve_scale_degree(ScaleDegree::new(7), &s), PitchClass::B);
    }

    #[test]
    fn flat_three_in_c_major_is_e_flat() {
        let s = c_major();
        let d = ScaleDegree::with_accidental(3, Accidental::Flat);
        assert_eq!(resolve_scale_degree(d, &s), PitchClass::DSharp); // = E♭
    }

    #[test]
    fn roman_v_in_c_major_is_g() {
        let s = c_major();
        assert_eq!(resolve_chord_root(RomanDegree::V, &s), PitchClass::G);
        assert_eq!(resolve_chord_root(RomanDegree::VI, &s), PitchClass::A);
        assert_eq!(resolve_chord_root(RomanDegree::FlatVII, &s), PitchClass::ASharp); // = B♭
    }

    #[test]
    fn secondary_dominant_via_in_key_resolves_correctly() {
        // V/V in C major = D (because V of G is D).
        let g_major = Scale::major(PitchClass::G);
        let spec = ChordSpec::Functional {
            roman: RomanDegree::V,
            suffix: ChordSuffix::new(ChordQuality::Dominant7),
            in_key: Some(g_major),
        };
        assert_eq!(
            resolve_chord_spec_root(&spec, &c_major()),
            PitchClass::D
        );
    }

    #[test]
    fn chord_step_third_depends_on_quality() {
        let major = ChordSuffix::new(ChordQuality::Major);
        let minor = ChordSuffix::new(ChordQuality::Minor);
        let dim = ChordSuffix::new(ChordQuality::Diminished);
        assert_eq!(chord_step_to_interval(ChordStep::Third, &major), Some(4));
        assert_eq!(chord_step_to_interval(ChordStep::Third, &minor), Some(3));
        assert_eq!(chord_step_to_interval(ChordStep::Third, &dim), Some(3));
    }

    #[test]
    fn chord_step_fifth_handles_diminished_and_augmented() {
        let dim = ChordSuffix::new(ChordQuality::Diminished);
        let aug = ChordSuffix::new(ChordQuality::Augmented);
        let major = ChordSuffix::new(ChordQuality::Major);
        assert_eq!(chord_step_to_interval(ChordStep::Fifth, &dim), Some(6));
        assert_eq!(chord_step_to_interval(ChordStep::Fifth, &aug), Some(8));
        assert_eq!(chord_step_to_interval(ChordStep::Fifth, &major), Some(7));
    }

    #[test]
    fn flat5_alteration_lowers_the_fifth() {
        let mut suffix = ChordSuffix::new(ChordQuality::Dominant7);
        suffix.alterations.push(Alteration::Flat5);
        assert_eq!(chord_step_to_interval(ChordStep::Fifth, &suffix), Some(6));
    }

    #[test]
    fn power_chord_has_no_third() {
        let power = ChordSuffix::new(ChordQuality::Power);
        assert_eq!(chord_step_to_interval(ChordStep::Third, &power), None);
        assert_eq!(chord_step_to_interval(ChordStep::Fifth, &power), Some(7));
    }

    #[test]
    fn nearest_to_picks_closest_octave() {
        let anchor = MidiNote::new(60).unwrap(); // C4
        // Nearest D to C4: D4 (62) is closer than D3 (50).
        assert_eq!(nearest_to(PitchClass::D, anchor).get(), 62);
        // Nearest A to C4: A3 (57) is closer than A4 (69).
        assert_eq!(nearest_to(PitchClass::A, anchor).get(), 57);
    }

    #[test]
    fn up_from_forces_strictly_above() {
        let anchor = MidiNote::new(60).unwrap(); // C4
        // Up from C4 to C: C5 (72), not C4 itself.
        assert_eq!(up_from(PitchClass::C, anchor).map(|n| n.get()), Some(72));
        // Up from C4 to D: D4 (62).
        assert_eq!(up_from(PitchClass::D, anchor).map(|n| n.get()), Some(62));
    }

    #[test]
    fn down_from_forces_strictly_below() {
        let anchor = MidiNote::new(60).unwrap();
        assert_eq!(down_from(PitchClass::C, anchor).map(|n| n.get()), Some(48));
        assert_eq!(down_from(PitchClass::B, anchor).map(|n| n.get()), Some(59));
    }

    #[test]
    fn gm_drum_kick_is_36() {
        assert_eq!(gm_drum_note(&DrumVoice::Kick).map(|n| n.get()), Some(36));
        assert_eq!(gm_drum_note(&DrumVoice::Snare).map(|n| n.get()), Some(38));
        assert_eq!(
            gm_drum_note(&DrumVoice::ClosedHat).map(|n| n.get()),
            Some(42)
        );
        assert_eq!(gm_drum_note(&DrumVoice::Extra("foo".into())), None);
    }
}
