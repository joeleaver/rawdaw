//! Per-block invariant context passed to every `AudioNode::process`.

use rawdaw_model::MusicalTime;

/// Read-only context for a single `process_block` call. Every node sees the
/// same `ProcessContext` per block.
///
/// `block_size` is the actual number of frames for this block; it may be
/// less than the `max_block_size` that the node was prepared for (e.g., the
/// last block at the end of a render).
#[derive(Debug, Clone, Copy)]
pub struct ProcessContext {
    pub sample_rate: u32,
    pub block_size: usize,

    /// Absolute sample index of this block's first frame in the engine's
    /// clock. Monotonically increasing across blocks while playing.
    pub absolute_time_samples: u64,

    /// Musical-time position at the start of this block.
    pub musical_time: MusicalTime,

    /// Current tempo in beats per minute.
    pub bpm: f64,

    /// `true` if the transport is rolling. `false` during stopped/paused
    /// states (nodes may still receive blocks for tail-out, automation, etc.).
    pub playing: bool,
}

impl ProcessContext {
    /// Convert a sample offset within the current block to its absolute
    /// sample time. Useful when a node needs to know "when in absolute
    /// terms did this event occur."
    pub fn absolute_sample(&self, offset_in_block: u32) -> u64 {
        self.absolute_time_samples + offset_in_block as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_ctx() -> ProcessContext {
        ProcessContext {
            sample_rate: 48_000,
            block_size: 512,
            absolute_time_samples: 1024,
            musical_time: MusicalTime::ZERO,
            bpm: 120.0,
            playing: true,
        }
    }

    #[test]
    fn absolute_sample_adds_offset_to_block_start() {
        let ctx = fake_ctx();
        assert_eq!(ctx.absolute_sample(0), 1024);
        assert_eq!(ctx.absolute_sample(256), 1280);
    }
}
