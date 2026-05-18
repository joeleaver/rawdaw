//! A stereo summing mixer: N stereo inputs → 1 stereo output.
//!
//! Built for the round-1 / engine-wiring use case: each project track's
//! instrument node feeds into a project master via the mixer. The
//! output is a simple sum (no gain trim, no panning, no clipping)
//! because the first audible iteration of the engine is plumbing
//! verification — per-input gain, soft clipping, and a proper master
//! bus model land in a later iteration.
//!
//! Inputs are declared at construction time; the descriptor slice is
//! owned by the node so the count is not bounded by `&'static`. Every
//! input is stereo (the only channel layout this codebase supports
//! today). The single output is also stereo.

use crate::buffer::ChannelCount;
use crate::context::ProcessContext;
use crate::event::EventBlock;
use crate::node::{AudioNode, InputDescriptor, OutputDescriptor, PortAccess};

const OUTPUTS: &[OutputDescriptor] = &[OutputDescriptor {
    name: "main",
    channels: ChannelCount::Stereo,
}];

pub struct MixerNode {
    inputs: Vec<InputDescriptor>,
}

impl MixerNode {
    /// Build a mixer with `input_count` stereo inputs. `input_count` of
    /// zero is permitted (the mixer emits silence) but is rarely
    /// useful — it's mostly a defensive non-panicking edge case for
    /// project loaders that haven't installed any track instruments
    /// yet.
    pub fn new(input_count: usize) -> Self {
        let inputs = (0..input_count)
            .map(|_| InputDescriptor {
                name: "in",
                channels: ChannelCount::Stereo,
            })
            .collect();
        Self { inputs }
    }

    pub fn input_count(&self) -> usize {
        self.inputs.len()
    }
}

impl AudioNode for MixerNode {
    fn process(
        &mut self,
        ports: &mut PortAccess<'_>,
        _events: &EventBlock<'_>,
        ctx: &ProcessContext,
    ) {
        if ports.outputs.count() == 0 {
            return;
        }
        let mut out = ports.outputs.get_mut(0);
        out.clear();
        let (out_l, out_r) = out.stereo_mut();
        let block_size = ctx.block_size;
        for i in 0..ports.inputs.count() {
            let inp = ports.inputs.get(i);
            // Skip non-stereo inputs defensively. By construction every
            // declared input is stereo, but a future input-channel-count
            // mismatch shouldn't crash the audio thread.
            if inp.channels() < 2 {
                continue;
            }
            let (in_l, in_r) = inp.stereo();
            for f in 0..block_size {
                out_l[f] += in_l[f];
                out_r[f] += in_r[f];
            }
        }
    }

    fn input_descriptors(&self) -> &[InputDescriptor] {
        &self.inputs
    }

    fn output_descriptors(&self) -> &[OutputDescriptor] {
        OUTPUTS
    }

    fn prepare(&mut self, _sample_rate: u32, _max_block_size: usize) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::PortAccess;
    use rawdaw_model::MusicalTime;

    const SR: u32 = 48_000;
    const BLOCK: usize = 256;

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
        node: &mut MixerNode,
        inputs: &[Vec<f32>],
        input_channel_counts: &[u8],
        out: &mut Vec<f32>,
    ) {
        out.clear();
        out.resize(2 * BLOCK, 0.0);
        let mut ports = PortAccess::new(
            inputs,
            input_channel_counts,
            std::slice::from_mut(out),
            &[2u8],
            BLOCK,
            BLOCK,
        );
        let evblock = EventBlock::new(&[]);
        node.process(&mut ports, &evblock, &ctx());
    }

    fn stereo_block(left: f32, right: f32) -> Vec<f32> {
        let mut buf = Vec::with_capacity(2 * BLOCK);
        buf.extend(std::iter::repeat_n(left, BLOCK));
        buf.extend(std::iter::repeat_n(right, BLOCK));
        buf
    }

    #[test]
    fn zero_inputs_yields_silence() {
        let mut node = MixerNode::new(0);
        let mut out = Vec::new();
        render_block(&mut node, &[], &[], &mut out);
        assert!(out.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn single_input_passes_through() {
        let mut node = MixerNode::new(1);
        let inputs = vec![stereo_block(0.25, 0.50)];
        let mut out = Vec::new();
        render_block(&mut node, &inputs, &[2u8], &mut out);
        let (l, r) = out.split_at(BLOCK);
        assert!(l.iter().all(|s| (*s - 0.25).abs() < 1e-6));
        assert!(r.iter().all(|s| (*s - 0.50).abs() < 1e-6));
    }

    #[test]
    fn multiple_inputs_sum() {
        let mut node = MixerNode::new(3);
        let inputs = vec![
            stereo_block(0.10, 0.20),
            stereo_block(0.30, 0.40),
            stereo_block(0.20, 0.10),
        ];
        let mut out = Vec::new();
        render_block(&mut node, &inputs, &[2u8, 2u8, 2u8], &mut out);
        let (l, r) = out.split_at(BLOCK);
        // L = 0.10 + 0.30 + 0.20 = 0.60.
        assert!(l.iter().all(|s| (*s - 0.60).abs() < 1e-6));
        // R = 0.20 + 0.40 + 0.10 = 0.70.
        assert!(r.iter().all(|s| (*s - 0.70).abs() < 1e-6));
    }

    #[test]
    fn declares_stereo_inputs_and_output() {
        let node = MixerNode::new(4);
        assert_eq!(node.input_descriptors().len(), 4);
        for d in node.input_descriptors() {
            assert_eq!(d.channels, ChannelCount::Stereo);
        }
        assert_eq!(node.output_descriptors().len(), 1);
        assert_eq!(node.output_descriptors()[0].channels, ChannelCount::Stereo);
    }
}
