//! Test suite for the wavetable synth.
//!
//! Split into submodules so each file stays under the workspace
//! ~700-line cap:
//!
//! - [`playback`]: basic note-on/off + L/R + offset behaviour.
//! - [`oscillators`]: F3 (multi-osc headroom) + F4 (PM/AM/RM
//!   modulation routing) tests.
//! - [`matrix`]: M3 mod-matrix routing pins + U3 `with_patch` tests.
//!
//! Shared rendering helpers (constants, `make_node`, `note_on`,
//! `render_block`, …) live here in `mod.rs` and are re-exported as
//! `pub(super)` for submodule use.

mod matrix;
mod oscillators;
mod playback;

use rawdaw_engine::event::{BlockEventInBlock, EventBlock};
use rawdaw_engine::node::{AudioNode, PortAccess};
use rawdaw_engine::ProcessContext;
use rawdaw_model::{Midi2Message, MidiChannel, MidiNote, MusicalTime, U16Velocity};

use crate::{BlockMessage, WavetableSynthNode};

pub(super) const SR: u32 = 48_000;
pub(super) const BLOCK: usize = 256;

pub(super) fn make_node() -> WavetableSynthNode {
    let mut node = WavetableSynthNode::new();
    node.prepare(SR, BLOCK);
    node
}

pub(super) fn note_on(n: u8, vel: U16Velocity) -> BlockMessage {
    BlockMessage::Midi(Midi2Message::NoteOn {
        channel: MidiChannel::default(),
        note: MidiNote::new(n).unwrap(),
        velocity: vel,
    })
}

pub(super) fn note_off(n: u8) -> BlockMessage {
    BlockMessage::Midi(Midi2Message::NoteOff {
        channel: MidiChannel::default(),
        note: MidiNote::new(n).unwrap(),
        velocity: U16Velocity::MIN,
    })
}

pub(super) fn ctx() -> ProcessContext {
    ProcessContext {
        sample_rate: SR,
        block_size: BLOCK,
        absolute_time_samples: 0,
        musical_time: MusicalTime::ZERO,
        bpm: 120.0,
        playing: true,
    }
}

pub(super) fn render_block(
    node: &mut WavetableSynthNode,
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

pub(super) fn rms(samples: &[f32]) -> f32 {
    let sumsq: f32 = samples.iter().map(|s| s * s).sum();
    (sumsq / samples.len() as f32).sqrt()
}
