//! A node that emits a single-sample impulse (value 1.0) on each `NoteOn`
//! event, on a stereo output. Used to verify event delivery and timing.
//!
//! Between events, the output is zero. L and R are identical.

use rawdaw_model::Midi2Message;

use crate::buffer::ChannelCount;
use crate::context::ProcessContext;
use crate::event::EventBlock;
use crate::node::{AudioNode, OutputDescriptor, PortAccess};

pub struct ImpulseNode;

impl ImpulseNode {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ImpulseNode {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioNode for ImpulseNode {
    fn process(
        &mut self,
        ports: &mut PortAccess<'_>,
        events: &EventBlock<'_>,
        ctx: &ProcessContext,
    ) {
        if ports.outputs.count() == 0 {
            return;
        }
        let mut out = ports.outputs.get_mut(0);
        out.clear();
        let (l, r) = out.stereo_mut();
        for ev in events.iter() {
            if !matches!(ev.message, Midi2Message::NoteOn { .. }) {
                continue;
            }
            let offset = ev.offset_in_block as usize;
            if offset >= ctx.block_size {
                continue;
            }
            l[offset] = 1.0;
            r[offset] = 1.0;
        }
    }

    fn output_descriptors(&self) -> &[OutputDescriptor] {
        const DESCRIPTORS: &[OutputDescriptor] = &[OutputDescriptor {
            name: "main",
            channels: ChannelCount::Stereo,
        }];
        DESCRIPTORS
    }

    fn prepare(&mut self, _sample_rate: u32, _max_block_size: usize) {}
}
