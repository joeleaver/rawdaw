//! A node that emits silence (zeros) on a single stereo output port.
//!
//! Used as the default master output in tests and as a placeholder
//! for not-yet-installed instruments.

use crate::buffer::ChannelCount;
use crate::context::ProcessContext;
use crate::event::EventBlock;
use crate::node::{AudioNode, OutputDescriptor, PortAccess};

pub struct SilenceNode;

impl SilenceNode {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SilenceNode {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioNode for SilenceNode {
    fn process(
        &mut self,
        ports: &mut PortAccess<'_>,
        _events: &EventBlock<'_>,
        _ctx: &ProcessContext,
    ) {
        for i in 0..ports.outputs.count() {
            ports.outputs.get_mut(i).clear();
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
