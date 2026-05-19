//! Output types produced by the realization pass.

use serde::{Deserialize, Serialize};

use crate::id::{NoteId, NoteOverrideId, PatternId, SectionRefId, TrackId, VariantId};
use crate::pitch::{MidiNote, U7};
use crate::time::SampleTime;

/// A single sample-timed event emitted by the realization pass.
///
/// Events are sorted by `time` in the output stream; ordering between
/// events at the same time is currently arbitrary (see realization design
/// doc for the future "NoteOff before NoteOn at same time" rule).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimedEvent {
    pub time: SampleTime,
    pub target: TrackId,
    pub message: Midi2Message,
    pub provenance: Provenance,
}

/// Internal MIDI 2.0 message type. v1 covers Note On / Note Off plus
/// the K4 expressive-control variants (`ControlChange` for
/// CC routing + the standard sustain pedal CC64; `PitchBend` for
/// the pitch wheel). Per-note pressure / poly aftertouch / RPN /
/// MPE remain out-of-scope.
///
/// Realization (`realize::realize`) currently emits only NoteOn /
/// NoteOff; the new variants come from external live input
/// (`rawdaw-app::midi_input`) and pass through `translate_events`
/// unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Midi2Message {
    NoteOn {
        channel: MidiChannel,
        note: MidiNote,
        velocity: U16Velocity,
    },
    NoteOff {
        channel: MidiChannel,
        note: MidiNote,
        velocity: U16Velocity,
    },
    /// MIDI 1.0-style Control Change. v1 stores 7-bit values; the
    /// `Midi2Message` namespace is forward-looking but the wire
    /// format we ingest from `midir` is MIDI 1.0.
    ///
    /// CC 64 = sustain pedal (value ≥ 64 = down, < 64 = up).
    /// Other CCs are routed to mod-matrix sources by K5.
    ControlChange {
        channel: MidiChannel,
        controller: U7,
        value: U7,
    },
    /// Pitch wheel — 14-bit value spread across two MIDI data bytes
    /// (lsb + msb), unsigned 0..16383 with center at 8192. K4
    /// applies the default ±2 semitone range to oscillator
    /// frequency; future RPN handling can change the range
    /// per-channel.
    PitchBend {
        channel: MidiChannel,
        value_14: u16,
    },
}

impl Midi2Message {
    /// Pitch-bend center value (no bend). MIDI standard.
    pub const PITCH_BEND_CENTER: u16 = 8192;
    /// Maximum unsigned 14-bit pitch-bend value (full positive bend).
    pub const PITCH_BEND_MAX: u16 = 16383;
}

/// MIDI channel 0..16. Drums conventionally live on channel 9 (channel 10
/// in 1-indexed lay terms).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct MidiChannel(u8);

impl MidiChannel {
    pub const DRUMS: Self = Self(9);

    pub const fn new(n: u8) -> Option<Self> {
        if n < 16 { Some(Self(n)) } else { None }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// MIDI 2.0 widens velocity to 16 bits. Stored events use [`U7`] for
/// compactness; widening to 16-bit happens at realization time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct U16Velocity(u16);

impl U16Velocity {
    pub const MIN: Self = Self(0);
    pub const MAX: Self = Self(0xFFFF);
    pub const HALF: Self = Self(0x8000);

    pub const fn new(n: u16) -> Self {
        Self(n)
    }

    pub const fn get(self) -> u16 {
        self.0
    }

    /// Widen a 7-bit velocity to 16 bits using the standard MIDI 2.0 scaling
    /// described in the M2 spec: 0 → 0, 127 → 65535, linear in between.
    ///
    /// This is a "good enough" widening for v1; the spec's exact min-center-max
    /// scaling can replace this without changing the API.
    pub fn from_u7(v: U7) -> Self {
        let n = v.get() as u32;
        // Multiplier chosen so 127 maps exactly to 65535 (127 * 516 + 63 = 65535).
        Self(((n * 516) + (n / 2)).min(0xFFFF) as u16)
    }
}

impl Default for U16Velocity {
    fn default() -> Self {
        Self::HALF
    }
}

/// Traceability information attached to every realized event, so the UI can
/// take a derived note and show where it came from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Provenance {
    pub pattern: PatternId,
    pub variant: VariantId,
    pub event_note_id: NoteId,
    pub override_applied: Option<NoteOverrideId>,
    pub section: SectionRefId,
}
