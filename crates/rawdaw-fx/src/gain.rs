//! Linear gain — stereo passthrough multiplied by a scalar.
//!
//! Used as the engine's master attenuator in `rawdaw-app` so the
//! summed output of the mixer has headroom before the cpal device.
//! v0 takes the gain at construction; parameter automation lands
//! with the engine-wide parameter queue.

use rawdaw_engine::buffer::ChannelCount;
use rawdaw_engine::context::ProcessContext;
use rawdaw_engine::event::EventBlock;
use rawdaw_engine::node::{AudioNode, InputDescriptor, OutputDescriptor, PortAccess};

/// Linear stereo gain node.
///
/// Single stereo input port → single stereo output port. The gain
/// is applied per-sample with no smoothing — fine for static
/// master attenuation; an LFO-driven gain would need a one-pole
/// smoother to avoid zipper noise (v1 growth).
pub struct GainNode {
    gain: f32,
}

impl GainNode {
    /// Construct with a linear gain factor. `1.0` is unity;
    /// `0.5` is roughly -6 dB; `0.25` is roughly -12 dB.
    pub fn new(gain: f32) -> Self {
        Self { gain }
    }

    /// Replace the gain factor. Discontinuous changes can zipper
    /// for amplitude moves; only safe between blocks where the host
    /// has muted output (parameter automation will route through a
    /// smoother).
    #[allow(dead_code)]
    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }
}

impl Default for GainNode {
    fn default() -> Self {
        // Unity gain default — explicitly opt into attenuation.
        Self::new(1.0)
    }
}

impl AudioNode for GainNode {
    fn process(
        &mut self,
        ports: &mut PortAccess<'_>,
        _events: &EventBlock<'_>,
        ctx: &ProcessContext,
    ) {
        if ports.outputs.count() == 0 || ports.inputs.count() == 0 {
            return;
        }
        let block_size = ctx.block_size;
        // The borrow checker needs the input read to complete before
        // the output borrow. Snapshot input samples into the output
        // first, then apply gain in place.
        let input = ports.inputs.get(0);
        let in_l = input.channel(0);
        let in_r = if input.channels() > 1 {
            input.channel(1)
        } else {
            input.channel(0)
        };

        let mut out = ports.outputs.get_mut(0);
        let (l, r) = out.stereo_mut();
        for i in 0..block_size.min(l.len()).min(in_l.len()) {
            l[i] = in_l[i] * self.gain;
        }
        for i in 0..block_size.min(r.len()).min(in_r.len()) {
            r[i] = in_r[i] * self.gain;
        }
    }

    fn input_descriptors(&self) -> &[InputDescriptor] {
        const DESCRIPTORS: &[InputDescriptor] = &[InputDescriptor {
            name: "main",
            channels: ChannelCount::Stereo,
        }];
        DESCRIPTORS
    }

    fn output_descriptors(&self) -> &[OutputDescriptor] {
        const DESCRIPTORS: &[OutputDescriptor] = &[OutputDescriptor {
            name: "main",
            channels: ChannelCount::Stereo,
        }];
        DESCRIPTORS
    }

    fn prepare(&mut self, _sample_rate: u32, _max_block_size: usize) {
        // Stateless beyond `gain`; nothing to allocate.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_model::MusicalTime;

    const BLOCK: usize = 64;
    const STRIDE: usize = 64;

    fn ctx() -> ProcessContext {
        ProcessContext {
            sample_rate: 48_000,
            block_size: BLOCK,
            absolute_time_samples: 0,
            musical_time: MusicalTime::ZERO,
            bpm: 120.0,
            playing: true,
        }
    }

    fn run_node(node: &mut GainNode, input: &[f32]) -> Vec<f32> {
        // Single stereo input port + single stereo output port.
        assert_eq!(input.len(), 2 * STRIDE);
        let input_buffers: [Vec<f32>; 1] = [input.to_vec()];
        let input_channel_counts: [u8; 1] = [2];
        let mut output_buffers: [Vec<f32>; 1] = [vec![0.0; 2 * STRIDE]];
        let output_channel_counts: [u8; 1] = [2];
        let mut ports = PortAccess::new(
            &input_buffers,
            &input_channel_counts,
            &mut output_buffers,
            &output_channel_counts,
            BLOCK,
            STRIDE,
        );
        let events = EventBlock::new(&[]);
        node.process(&mut ports, &events, &ctx());
        output_buffers.into_iter().next().unwrap()
    }

    fn input_signal(value: f32) -> Vec<f32> {
        let mut buf = vec![0.0; 2 * STRIDE];
        for s in buf.iter_mut().take(BLOCK) {
            *s = value;
        }
        for s in buf.iter_mut().skip(STRIDE).take(BLOCK) {
            *s = value;
        }
        buf
    }

    #[test]
    fn unity_gain_is_identity() {
        let mut node = GainNode::new(1.0);
        node.prepare(48_000, BLOCK);
        let input = input_signal(0.4);
        let output = run_node(&mut node, &input);
        for i in 0..BLOCK {
            assert!((output[i] - 0.4).abs() < 1e-6);
            assert!((output[i + STRIDE] - 0.4).abs() < 1e-6);
        }
    }

    #[test]
    fn half_gain_attenuates_by_six_db() {
        let mut node = GainNode::new(0.5);
        node.prepare(48_000, BLOCK);
        let input = input_signal(1.0);
        let output = run_node(&mut node, &input);
        for i in 0..BLOCK {
            assert!((output[i] - 0.5).abs() < 1e-6);
            assert!((output[i + STRIDE] - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn zero_gain_is_silence() {
        let mut node = GainNode::new(0.0);
        node.prepare(48_000, BLOCK);
        let input = input_signal(1.0);
        let output = run_node(&mut node, &input);
        assert!(output.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn descriptors_advertise_stereo_io() {
        let node = GainNode::new(1.0);
        let inputs = node.input_descriptors();
        let outputs = node.output_descriptors();
        assert_eq!(inputs.len(), 1);
        assert_eq!(outputs.len(), 1);
        assert_eq!(inputs[0].channels, ChannelCount::Stereo);
        assert_eq!(outputs[0].channels, ChannelCount::Stereo);
    }

}
