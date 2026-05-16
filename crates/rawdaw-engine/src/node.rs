//! The `AudioNode` trait and the `PortAccess` API for reading inputs /
//! writing outputs in a `process` call.

use crate::buffer::{BufferMut, BufferRef, ChannelCount};
use crate::context::ProcessContext;
use crate::event::EventBlock;

/// A unit of audio processing in the engine's DAG.
///
/// Implementors run on the audio (RT) thread. `process` is the hot path
/// and MUST be RT-safe: no allocation, no locks, no syscalls, no I/O.
///
/// All preallocation must happen in `prepare`, which is called once per
/// node on the non-RT thread before the node enters the active graph.
pub trait AudioNode: Send {
    /// One block of audio processing.
    ///
    /// `ports.inputs.get(i)` returns a `BufferRef` for input port `i`.
    /// `ports.outputs.get_mut(i)` returns a `BufferMut` for output port `i`.
    /// Both are valid for `ctx.block_size` frames.
    ///
    /// `events` contains only events targeting this node, sorted by
    /// `offset_in_block` ascending. `events.is_empty()` is the common case.
    fn process(
        &mut self,
        ports: &mut PortAccess<'_>,
        events: &EventBlock<'_>,
        ctx: &ProcessContext,
    );

    /// Audio inputs this node accepts. Default: none (a source).
    fn input_descriptors(&self) -> &[InputDescriptor] {
        &[]
    }

    /// Audio outputs this node produces. Must return the same slice on
    /// every call.
    fn output_descriptors(&self) -> &[OutputDescriptor];

    /// Sample latency this node introduces. PDC is a v2 concern.
    fn latency_samples(&self) -> usize {
        0
    }

    /// Called once on the non-RT thread before the node enters the active
    /// graph. Allows the node to size internal state to `sample_rate` and
    /// `max_block_size`.
    fn prepare(&mut self, sample_rate: u32, max_block_size: usize);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputDescriptor {
    pub name: &'static str,
    pub channels: ChannelCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputDescriptor {
    pub name: &'static str,
    pub channels: ChannelCount,
}

// ---------- PortAccess ----------

/// Bundle of input and output access for one `process` call. Constructed
/// fresh by the engine each block — no allocation, just borrow assembly.
///
/// Inputs and outputs are exposed through two sub-structs so a node can
/// borrow them simultaneously (`ports.inputs.get(0)` and
/// `ports.outputs.get_mut(0)` coexist).
pub struct PortAccess<'a> {
    pub inputs: PortInputs<'a>,
    pub outputs: PortOutputs<'a>,
}

impl<'a> PortAccess<'a> {
    /// Construct from engine-owned scratch and the node's own output buffers.
    ///
    /// `frames` is the current block's active sample count per channel;
    /// `stride` is the storage stride (always `>= frames`). The engine
    /// passes `stride = max_block_size`, since both input scratch and the
    /// node's output buffers were preallocated for that.
    pub fn new(
        input_buffers: &'a [Vec<f32>],
        input_channel_counts: &'a [u8],
        output_buffers: &'a mut [Vec<f32>],
        output_channel_counts: &'a [u8],
        frames: usize,
        stride: usize,
    ) -> Self {
        Self {
            inputs: PortInputs {
                buffers: input_buffers,
                channel_counts: input_channel_counts,
                frames,
                stride,
            },
            outputs: PortOutputs {
                buffers: output_buffers,
                channel_counts: output_channel_counts,
                frames,
                stride,
            },
        }
    }
}

pub struct PortInputs<'a> {
    buffers: &'a [Vec<f32>],
    channel_counts: &'a [u8],
    frames: usize,
    stride: usize,
}

impl<'a> PortInputs<'a> {
    pub fn count(&self) -> usize {
        self.buffers.len().min(self.channel_counts.len())
    }

    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// Read-only view of input port `port`.
    pub fn get(&self, port: usize) -> BufferRef<'_> {
        let channels = self.channel_counts[port] as usize;
        BufferRef::new(&self.buffers[port], channels, self.frames, self.stride)
    }
}

pub struct PortOutputs<'a> {
    buffers: &'a mut [Vec<f32>],
    channel_counts: &'a [u8],
    frames: usize,
    stride: usize,
}

impl<'a> PortOutputs<'a> {
    pub fn count(&self) -> usize {
        self.buffers.len().min(self.channel_counts.len())
    }

    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// Mutable view of output port `port`.
    pub fn get_mut(&mut self, port: usize) -> BufferMut<'_> {
        let channels = self.channel_counts[port] as usize;
        BufferMut::new(&mut self.buffers[port], channels, self.frames, self.stride)
    }
}
