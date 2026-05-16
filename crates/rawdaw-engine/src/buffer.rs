//! Audio buffer types and channel layout.
//!
//! Buffers are 32-bit float, **planar** (one channel contiguous after another
//! in memory). The underlying storage uses `stride` samples per channel (set
//! at preparation time = `max_block_size`); the current block uses `frames`
//! samples per channel (`frames <= stride`).
//!
//! ```text
//!   storage: [ch0 sample 0..stride) ‖ [ch1 sample 0..stride)
//!   per-block view of ch:  storage[ch*stride .. ch*stride + frames]
//! ```
//!
//! When `frames == stride` the view is dense; when `frames < stride` (a
//! partial block, common at end-of-render and during cpal's variable-block
//! callbacks) the samples between `frames` and `stride` are leftover state
//! from prior blocks and must not be read.
//!
//! Channel order for stereo: 0 = L, 1 = R.

/// Channel layout for a single port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelCount {
    Mono,
    Stereo,
}

impl ChannelCount {
    pub const fn count(self) -> usize {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
        }
    }
}

/// Immutable view of one port's planar audio for one block.
///
/// Invariants:
/// - `samples.len() == channels * stride`
/// - `frames <= stride`
///
/// `frames` is the number of active samples per channel for this block;
/// `stride` is the storage stride between channels.
pub struct BufferRef<'a> {
    samples: &'a [f32],
    channels: usize,
    frames: usize,
    stride: usize,
}

impl<'a> BufferRef<'a> {
    pub fn new(samples: &'a [f32], channels: usize, frames: usize, stride: usize) -> Self {
        debug_assert!(stride >= frames, "stride ({stride}) < frames ({frames})");
        debug_assert_eq!(
            samples.len(),
            channels * stride,
            "samples.len ({}) != channels * stride ({})",
            samples.len(),
            channels * stride
        );
        Self {
            samples,
            channels,
            frames,
            stride,
        }
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Active-frames slice for one channel.
    pub fn channel(&self, ch: usize) -> &[f32] {
        let start = ch * self.stride;
        &self.samples[start..start + self.frames]
    }

    pub fn mono(&self) -> &[f32] {
        self.channel(0)
    }

    pub fn stereo(&self) -> (&[f32], &[f32]) {
        (self.channel(0), self.channel(1))
    }
}

/// Mutable view of one port's planar audio for one block. Same invariants
/// as [`BufferRef`].
pub struct BufferMut<'a> {
    samples: &'a mut [f32],
    channels: usize,
    frames: usize,
    stride: usize,
}

impl<'a> BufferMut<'a> {
    pub fn new(samples: &'a mut [f32], channels: usize, frames: usize, stride: usize) -> Self {
        debug_assert!(stride >= frames, "stride ({stride}) < frames ({frames})");
        debug_assert_eq!(
            samples.len(),
            channels * stride,
            "samples.len ({}) != channels * stride ({})",
            samples.len(),
            channels * stride
        );
        Self {
            samples,
            channels,
            frames,
            stride,
        }
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn frames(&self) -> usize {
        self.frames
    }

    pub fn channel_mut(&mut self, ch: usize) -> &mut [f32] {
        let start = ch * self.stride;
        &mut self.samples[start..start + self.frames]
    }

    pub fn mono_mut(&mut self) -> &mut [f32] {
        self.channel_mut(0)
    }

    /// Get mutable slices for the two active-frame channels simultaneously.
    /// Panics if `channels < 2`.
    pub fn stereo_mut(&mut self) -> (&mut [f32], &mut [f32]) {
        assert!(
            self.channels >= 2,
            "stereo_mut on a {}-channel buffer",
            self.channels
        );
        let stride = self.stride;
        let frames = self.frames;
        // Channel 0: [0..frames]. Channel 1: [stride..stride+frames].
        // We use split_at_mut at the channel-1 boundary.
        let (head, tail) = self.samples.split_at_mut(stride);
        (&mut head[..frames], &mut tail[..frames])
    }

    /// Zero the active frames in every channel. Storage beyond `frames` per
    /// channel is left untouched.
    pub fn clear(&mut self) {
        let frames = self.frames;
        let stride = self.stride;
        for ch in 0..self.channels {
            let start = ch * stride;
            for s in self.samples[start..start + frames].iter_mut() {
                *s = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_count_returns_one_for_mono_two_for_stereo() {
        assert_eq!(ChannelCount::Mono.count(), 1);
        assert_eq!(ChannelCount::Stereo.count(), 2);
    }

    #[test]
    fn buffer_ref_dense_stride_indexes_channels() {
        // Stereo, 3 frames, stride == frames (dense).
        let flat = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
        let buf = BufferRef::new(&flat, 2, 3, 3);
        assert_eq!(buf.channels(), 2);
        assert_eq!(buf.frames(), 3);
        assert_eq!(buf.channel(0), &[0.1, 0.2, 0.3]);
        assert_eq!(buf.channel(1), &[0.4, 0.5, 0.6]);
    }

    #[test]
    fn buffer_ref_padded_stride_skips_inactive_samples() {
        // Stereo, 2 active frames, stride 4 (padded). Storage:
        //   ch0: [0.1, 0.2, X, X] | ch1: [0.4, 0.5, X, X]
        // 'X' is stale leftover; the buffer only exposes the first 2 per channel.
        let flat = [0.1, 0.2, 9.9, 9.9, 0.4, 0.5, 9.9, 9.9];
        let buf = BufferRef::new(&flat, 2, 2, 4);
        assert_eq!(buf.channel(0), &[0.1, 0.2]);
        assert_eq!(buf.channel(1), &[0.4, 0.5]);
    }

    #[test]
    fn buffer_mut_clear_zeroes_active_frames_only() {
        // Write a sentinel in the stale region, then clear, and confirm only
        // active frames went to zero.
        let mut flat = [1.0, 1.0, 9.9, 9.9, 2.0, 2.0, 9.9, 9.9];
        {
            let mut buf = BufferMut::new(&mut flat, 2, 2, 4);
            buf.clear();
        }
        assert_eq!(flat, [0.0, 0.0, 9.9, 9.9, 0.0, 0.0, 9.9, 9.9]);
    }

    #[test]
    fn buffer_mut_stereo_mut_yields_disjoint_active_slices() {
        let mut flat = [0.0; 8];
        let mut buf = BufferMut::new(&mut flat, 2, 4, 4);
        let (l, r) = buf.stereo_mut();
        l[0] = 1.0;
        r[0] = 2.0;
        assert_eq!(flat[0], 1.0);
        assert_eq!(flat[4], 2.0);
    }
}
