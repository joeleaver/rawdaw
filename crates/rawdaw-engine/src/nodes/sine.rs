//! A polyphonic sine oscillator.
//!
//! Each `NoteOn` allocates a voice with phase 0, a precomputed phase
//! increment from a 128-entry table built at `prepare()` time, and a
//! linear amplitude derived from velocity. Each `NoteOff` deactivates
//! the first matching active voice (matched by MIDI note number).
//!
//! When all voices are in use, the oldest active voice is stolen. Voice
//! age is a monotonic counter incremented on every `NoteOn`.
//!
//! Voices and the frequency table are preallocated in `prepare()`; no
//! allocation happens in `process()`. Per-sample event handling honors
//! sub-block event offsets — a `NoteOn` at offset 32 starts producing
//! sound exactly at sample 32 of the current block.
//!
//! v1 is a first real synth, sufficient to verify the full
//! realization → translation → engine → audio pipeline. A subtractive
//! synth with envelopes, filters, and parameter automation is a future
//! `rawdaw-synth` crate, not this node.

use std::f32::consts::TAU;

use rawdaw_model::{Midi2Message, MidiNote, U16Velocity};

use crate::buffer::ChannelCount;
use crate::context::ProcessContext;
use crate::event::EventBlock;
use crate::node::{AudioNode, OutputDescriptor, PortAccess};

/// Maximum simultaneous voices. Chosen for v1; the voice manager design
/// (linear scan, oldest-steal) is fine at this size and gives us enough
/// polyphony for layered chord pads while we're still in the model-driven
/// composition era. Revisit when arrangements regularly exceed this.
const NUM_VOICES: usize = 16;

/// A4 reference pitch in Hz. Standard concert tuning. v1 hardcodes this;
/// the tuning system gets its own configuration surface later, alongside
/// non-12-TET work.
const A4_HZ: f32 = 440.0;

/// MIDI note number of A4 (concert A above middle C).
const A4_NOTE: f32 = 69.0;

/// A single oscillator voice. Inactive voices keep their last phase /
/// increment so retriggering on the same slot is cheap; only the `active`
/// flag matters for summing.
#[derive(Default, Clone, Copy)]
struct Voice {
    note: u8,
    active: bool,
    phase: f32,
    phase_inc: f32,
    amplitude: f32,
    age: u64,
}

/// Polyphonic sine oscillator. Single stereo output port; L and R are
/// identical (no spatialisation yet).
pub struct SineNode {
    voices: Vec<Voice>,
    /// phase_inc_table[n] is the phase advance per sample for MIDI note `n`
    /// at the current sample rate. Filled in `prepare()`; never resized.
    phase_inc_table: Vec<f32>,
    /// Monotonic counter incremented on every NoteOn. Lowest age = oldest
    /// active voice, which is the steal target.
    age_counter: u64,
}

impl SineNode {
    pub fn new() -> Self {
        Self {
            voices: Vec::new(),
            phase_inc_table: Vec::new(),
            age_counter: 0,
        }
    }

    /// Pick the slot for a new voice. Prefers any inactive voice; otherwise
    /// steals the oldest active voice. Voices vec is guaranteed non-empty
    /// because `prepare()` runs at install time (before any `process()`).
    fn allocate_voice(&self) -> usize {
        if let Some((i, _)) = self
            .voices
            .iter()
            .enumerate()
            .find(|(_, v)| !v.active)
        {
            return i;
        }
        self.voices
            .iter()
            .enumerate()
            .min_by_key(|(_, v)| v.age)
            .map(|(i, _)| i)
            .expect("voices vec must be non-empty after prepare()")
    }

    fn note_on(&mut self, note: MidiNote, velocity: U16Velocity) {
        self.age_counter += 1;
        let idx = self.allocate_voice();
        let voice = &mut self.voices[idx];
        voice.note = note.get();
        voice.active = true;
        voice.phase = 0.0;
        voice.phase_inc = self.phase_inc_table[note.get() as usize];
        voice.amplitude = u16_velocity_to_amplitude(velocity);
        voice.age = self.age_counter;
    }

    fn note_off(&mut self, note: MidiNote) {
        if let Some(voice) = self
            .voices
            .iter_mut()
            .find(|v| v.active && v.note == note.get())
        {
            voice.active = false;
        }
    }

    fn apply_event(&mut self, message: &Midi2Message) {
        match message {
            Midi2Message::NoteOn { note, velocity, .. } => self.note_on(*note, *velocity),
            Midi2Message::NoteOff { note, .. } => self.note_off(*note),
        }
    }
}

impl Default for SineNode {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioNode for SineNode {
    fn process(
        &mut self,
        ports: &mut PortAccess<'_>,
        events: &EventBlock<'_>,
        ctx: &ProcessContext,
    ) {
        if ports.outputs.count() == 0 {
            return;
        }
        let block_size = ctx.block_size;
        let mut out = ports.outputs.get_mut(0);
        out.clear();
        let (l, r) = out.stereo_mut();

        let event_slice = events.as_slice();
        let mut next_event = 0;

        for i in 0..block_size {
            // Apply every event whose offset has now been reached. Events
            // are sorted by offset_in_block ascending; multiple events can
            // share an offset.
            while next_event < event_slice.len()
                && (event_slice[next_event].offset_in_block as usize) <= i
            {
                let msg = &event_slice[next_event].message;
                self.apply_event(msg);
                next_event += 1;
            }

            let mut sample = 0.0_f32;
            for v in self.voices.iter_mut() {
                if !v.active {
                    continue;
                }
                sample += v.phase.sin() * v.amplitude;
                v.phase += v.phase_inc;
                if v.phase >= TAU {
                    v.phase -= TAU;
                }
            }
            l[i] = sample;
            r[i] = sample;
        }
    }

    fn output_descriptors(&self) -> &[OutputDescriptor] {
        const DESCRIPTORS: &[OutputDescriptor] = &[OutputDescriptor {
            name: "main",
            channels: ChannelCount::Stereo,
        }];
        DESCRIPTORS
    }

    fn prepare(&mut self, sample_rate: u32, _max_block_size: usize) {
        self.voices = vec![Voice::default(); NUM_VOICES];
        self.phase_inc_table = (0..=127)
            .map(|n| note_to_phase_inc(n, sample_rate))
            .collect();
        self.age_counter = 0;
    }
}

fn note_to_phase_inc(note: u8, sample_rate: u32) -> f32 {
    let hz = A4_HZ * 2.0_f32.powf((note as f32 - A4_NOTE) / 12.0);
    TAU * hz / sample_rate as f32
}

fn u16_velocity_to_amplitude(v: U16Velocity) -> f32 {
    v.get() as f32 / u16::MAX as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{BlockEventInBlock, EventBlock};
    use crate::node::PortAccess;
    use rawdaw_model::{Midi2Message, MidiChannel, MidiNote, MusicalTime, U16Velocity};

    const SR: u32 = 48_000;
    const BLOCK: usize = 256;

    fn make_node() -> SineNode {
        let mut node = SineNode::new();
        node.prepare(SR, BLOCK);
        node
    }

    fn note_on(n: u8, vel: U16Velocity) -> Midi2Message {
        Midi2Message::NoteOn {
            channel: MidiChannel::default(),
            note: MidiNote::new(n).unwrap(),
            velocity: vel,
        }
    }

    fn note_off(n: u8) -> Midi2Message {
        Midi2Message::NoteOff {
            channel: MidiChannel::default(),
            note: MidiNote::new(n).unwrap(),
            velocity: U16Velocity::MIN,
        }
    }

    fn ctx() -> ProcessContext {
        ProcessContext {
            sample_rate: SR,
            block_size: BLOCK,
            absolute_time_samples: 0,
            musical_time: MusicalTime::ZERO,
            bpm: 120.0,
            playing: true,
        }
    }

    fn render_block(
        node: &mut SineNode,
        events: &[BlockEventInBlock],
        out: &mut Vec<f32>,
    ) {
        out.clear();
        out.resize(2 * BLOCK, 0.0);
        let mut ports = PortAccess::new(
            &[],
            &[],
            std::slice::from_mut(out),
            &[2u8],
            BLOCK,
            BLOCK,
        );
        let evblock = EventBlock::new(events);
        node.process(&mut ports, &evblock, &ctx());
    }

    fn rms(samples: &[f32]) -> f32 {
        let sumsq: f32 = samples.iter().map(|s| s * s).sum();
        (sumsq / samples.len() as f32).sqrt()
    }

    #[test]
    fn silent_until_note_on() {
        let mut node = make_node();
        let mut buf = Vec::new();
        render_block(&mut node, &[], &mut buf);
        assert!(buf.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn note_on_produces_nonzero_output() {
        let mut node = make_node();
        let mut buf = Vec::new();
        let events = vec![BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }];
        render_block(&mut node, &events, &mut buf);
        let left = &buf[..BLOCK];
        assert!(rms(left) > 0.01, "expected audible signal, got rms {}", rms(left));
    }

    #[test]
    fn note_off_deactivates_voice() {
        let mut node = make_node();
        // Block 1: NoteOn.
        let mut buf = Vec::new();
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut buf,
        );
        // Block 2: NoteOff at the very start, then silence the rest of the block.
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_off(60),
            }],
            &mut buf,
        );
        assert!(
            buf.iter().all(|s| *s == 0.0),
            "after NoteOff the entire block should be silent",
        );
    }

    #[test]
    fn mid_block_note_on_starts_at_offset() {
        let mut node = make_node();
        let mut buf = Vec::new();
        // NoteOn at offset 100: samples [0..100) silent, [100..BLOCK) audible.
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 100,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut buf,
        );
        let left = &buf[..BLOCK];
        assert!(
            left[..100].iter().all(|s| *s == 0.0),
            "pre-onset samples must be silent",
        );
        assert!(
            rms(&left[100..]) > 0.01,
            "post-onset section must produce signal",
        );
    }

    #[test]
    fn polyphony_sums_voices() {
        let mut node = make_node();
        let mut buf = Vec::new();
        let events = vec![
            BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            },
            BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(64, U16Velocity::HALF),
            },
            BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(67, U16Velocity::HALF),
            },
        ];
        render_block(&mut node, &events, &mut buf);
        let left = &buf[..BLOCK];
        // RMS should exceed a single voice's RMS.
        let single_voice_rms_upper_bound = 0.5_f32 / std::f32::consts::SQRT_2; // ~0.354
        assert!(
            rms(left) > single_voice_rms_upper_bound * 1.1,
            "summed polyphony should exceed single-voice RMS, got {}",
            rms(left),
        );
    }

    #[test]
    fn voice_stealing_takes_oldest() {
        let mut node = make_node();
        // Fill all voices.
        for i in 0..NUM_VOICES {
            node.note_on(MidiNote::new(60 + i as u8).unwrap(), U16Velocity::HALF);
        }
        // Steal: NoteOn 90 should evict note 60 (oldest, age=1).
        node.note_on(MidiNote::new(90).unwrap(), U16Velocity::HALF);
        // Look for note 60 — it should no longer be active.
        let still_active_60 = node
            .voices
            .iter()
            .any(|v| v.active && v.note == 60);
        assert!(!still_active_60, "oldest voice (note 60) should have been stolen");
        // Note 90 should be active somewhere.
        let active_90 = node.voices.iter().any(|v| v.active && v.note == 90);
        assert!(active_90, "newest voice (note 90) should be active after steal");
    }
}
