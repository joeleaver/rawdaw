//! Soft-clipping master limiter.
//!
//! Memoryless tanh-knee shaper applied per-sample to a stereo
//! passthrough port. Replaces the audible `-12 dB` master-gain
//! workaround in `rawdaw-app::audio` (X7 will drop that workaround
//! once the chain is wired end-to-end).
//!
//! Shape: `y = threshold * tanh(x / threshold)`. Unity slope at
//! `x = 0` so signals well below `threshold` pass through linearly.
//! Asymptotes to `±threshold` so signals well above the knee get
//! smoothly limited rather than hard-clipped. With the default
//! threshold of `0.7` (~ -3.1 dBFS), the knee leaves the typical
//! pre-master synth output essentially untouched while still
//! catching transients that would otherwise hard-clip in cpal.
//!
//! Two-layer patch shape (mirrors the synth crates, U3a):
//!
//! - [`SoftClipPatch`] — runtime, lives in this crate, holds the
//!   `f32` threshold the audio thread reads. `Copy` so the audio
//!   thread → host publisher path is a memcpy.
//! - [`rawdaw_model::SoftClipData`] — serialized, lives in
//!   `rawdaw-model::master_fx`. Carries `format_version` for
//!   schema migration. Bridges into the runtime via
//!   `From<SoftClipData>` so callers building a node from a
//!   `Project.master_chain` entry don't have to manually unpack.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use rawdaw_engine::buffer::ChannelCount;
use rawdaw_engine::context::ProcessContext;
use rawdaw_engine::event::{BlockMessage, EventBlock, ParamEvent};
use rawdaw_engine::node::{AudioNode, InputDescriptor, OutputDescriptor, PortAccess};
use rawdaw_model::master_fx::SoftClipData;

/// FX-kind tag for [`SoftClipParam`] in the engine's
/// [`ParamEvent::path`] (byte 0). Per the X0 design, every master-FX
/// `ParamEvent` reserves the first path byte as the FX kind so the
/// engine can route blind by NodeId while each FX crate owns its own
/// per-kind sub-path layout. SoftClip is kind `0`; future variants
/// (`Eq`, `Reverb`, …) get successive integer tags.
pub const SOFT_CLIP_FX_KIND: u8 = 0;

/// Param sub-tag inside [`SoftClipParam::encode`]'s path. v1 ships
/// one parameter so this is the only sub-tag; future SoftClip
/// extensions (e.g., a wet/dry mix, oversampling ratio) would
/// allocate successive sub-tags here.
const SOFT_CLIP_PARAM_THRESHOLD: u8 = 0;

/// The threshold below which the soft-clipper would degenerate into
/// a hard limiter (everything above `threshold` clamps to a tiny
/// asymptote, audible as a digital fuzz). Clamps inputs into a
/// musically useful range.
pub const MIN_THRESHOLD: f32 = 0.001;

/// The threshold above which the soft-clipper has no audible effect
/// (everything fits within the knee). Clamps inputs into a
/// musically useful range.
pub const MAX_THRESHOLD: f32 = 0.999;

/// Default threshold. Pinned at `0.7` — matches
/// [`SoftClipData::default`] in `rawdaw-model::master_fx` so a
/// freshly-loaded project and a freshly-constructed one sound
/// identical. ~ -3.1 dBFS.
pub const DEFAULT_THRESHOLD: f32 = 0.7;

/// Runtime patch for [`SoftClipNode`]. Mirrors
/// [`rawdaw_model::SoftClipData`] minus the serialization
/// `format_version` (the runtime patch doesn't need to remember the
/// on-disk schema version — it's already loaded). `Copy` so the
/// host-side patch publisher can `lock() → *slot = self.patch`
/// in a single memcpy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoftClipPatch {
    pub threshold: f32,
}

impl SoftClipPatch {
    /// Construct from raw threshold. Clamped into the audibly-useful
    /// range. Callers that don't care about the bound default to
    /// `SoftClipPatch::default()` for the M5-equivalent contract.
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold: threshold.clamp(MIN_THRESHOLD, MAX_THRESHOLD),
        }
    }
}

impl Default for SoftClipPatch {
    fn default() -> Self {
        Self::new(DEFAULT_THRESHOLD)
    }
}

impl From<SoftClipData> for SoftClipPatch {
    /// Bridge from the serialized model type. The clamp inside
    /// [`SoftClipPatch::new`] hardens against out-of-range data on
    /// disk (e.g., a hand-edited project file with `threshold: 5.0`).
    fn from(data: SoftClipData) -> Self {
        Self::new(data.threshold)
    }
}

/// Audio-thread → host bridge for the soft-clipper's patch state.
/// Mirrors `WavetablePublishers` / `DrumPublishers` exactly — same
/// `Arc<AtomicU64>` version counter + `Arc<Mutex<Patch>>` snapshot
/// shape, same trade-off (audio-thread `lock()` for the brief
/// memcpy; future migration to triple-buffer / arc-swap moves every
/// publisher together).
#[derive(Clone)]
pub struct SoftClipPublishers {
    /// Bumped on every successful [`ParamEvent`] apply. Host pollers
    /// watch this with `Acquire` loads to know when the snapshot is
    /// fresh; the audio-thread bumps with `Release` after writing
    /// the snapshot so the version-then-snapshot ordering is
    /// observable.
    pub version: Arc<AtomicU64>,
    /// Latest post-apply patch — mirrors the node's `self.patch`
    /// after every `ParamEvent`. Host reads via `lock()`.
    pub snapshot: Arc<Mutex<SoftClipPatch>>,
}

impl SoftClipPublishers {
    /// Build a fresh publishers pair seeded with `initial`. Version
    /// starts at 0; snapshot starts at the initial patch so the host
    /// can read the boot state without a `prepare()` having fired.
    pub fn new(initial: SoftClipPatch) -> Self {
        Self {
            version: Arc::new(AtomicU64::new(0)),
            snapshot: Arc::new(Mutex::new(initial)),
        }
    }
}

/// Parameter address for [`SoftClipNode`]. v1 carries one variant;
/// future SoftClip extensions add variants here without disturbing
/// the FX-kind discriminant in the [`ParamEvent::path`] byte 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftClipParam {
    Threshold,
}

impl SoftClipParam {
    /// Encode into the engine's opaque 8-byte
    /// [`ParamEvent::path`]. Byte 0 is the FX-kind tag
    /// ([`SOFT_CLIP_FX_KIND`]); byte 1 is the per-kind sub-tag for
    /// this variant.
    pub fn encode(self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[0] = SOFT_CLIP_FX_KIND;
        out[1] = match self {
            Self::Threshold => SOFT_CLIP_PARAM_THRESHOLD,
        };
        out
    }

    /// Decode an 8-byte path back into a [`SoftClipParam`]. Returns
    /// `None` if byte 0 doesn't match [`SOFT_CLIP_FX_KIND`] (the
    /// path belongs to a different FX kind — the engine's
    /// chain-routing code should never send it here, but a host bug
    /// is recoverable as a no-op) or byte 1 is an unrecognized
    /// sub-tag.
    pub fn decode(bytes: &[u8; 8]) -> Option<Self> {
        if bytes[0] != SOFT_CLIP_FX_KIND {
            return None;
        }
        match bytes[1] {
            SOFT_CLIP_PARAM_THRESHOLD => Some(Self::Threshold),
            _ => None,
        }
    }

    /// Apply this parameter change to a runtime patch. Clamps into
    /// the audibly-useful range so a bad host-side value can't drive
    /// the node into pathological behavior (a 0.0 threshold would
    /// degenerate into a hard limiter producing audible fuzz; a >1.0
    /// threshold would do nothing audible). Mirrors `WavetableParam::
    /// apply`'s value-validation contract.
    pub fn apply(self, patch: &mut SoftClipPatch, value: f32) {
        match self {
            Self::Threshold => {
                patch.threshold = value.clamp(MIN_THRESHOLD, MAX_THRESHOLD);
            }
        }
    }
}

/// Master-chain soft-clipper [`AudioNode`].
///
/// Single stereo input → single stereo output. Per-sample shaping;
/// the tanh is memoryless so there's no state to allocate beyond
/// the patch + publishers. `Send` because the audio thread will own
/// it after construction; `!Sync` is fine — the audio thread is the
/// sole owner mid-block.
///
/// Stateless beyond the patch fields: no envelope, no LFO, no
/// per-voice anything. The shaper takes `threshold` from
/// `self.patch` on every sample, so a mid-block `Param` event takes
/// effect on the next sample — no smoothing yet. A future
/// per-block smoother is a known follow-on if zipper noise becomes
/// audible at large threshold moves.
pub struct SoftClipNode {
    patch: SoftClipPatch,
    /// Bumped on every successful `ParamEvent` apply. Pollers watch
    /// this to know when the snapshot is fresh.
    patch_version: Arc<AtomicU64>,
    /// Latest post-apply patch — mirrors `self.patch` after every
    /// `ParamEvent`. Host reads via `lock()`.
    patch_snapshot: Arc<Mutex<SoftClipPatch>>,
}

impl SoftClipNode {
    /// Build with the default patch (threshold = 0.7). Equivalent
    /// to `with_patch(SoftClipPatch::default())`. Publishers are
    /// created internally and dropped when this node drops; tests
    /// and historical callers that don't need them stay terse.
    pub fn new() -> Self {
        Self::with_patch(SoftClipPatch::default())
    }

    /// Build with a specific runtime patch. Publishers are created
    /// internally — callers that need to observe the patch from the
    /// host side use [`Self::with_patch_publishers`] instead.
    pub fn with_patch(patch: SoftClipPatch) -> Self {
        let pubs = SoftClipPublishers::new(patch);
        Self::with_patch_publishers(patch, pubs)
    }

    /// Build with patch + host-owned publishers. The host keeps a
    /// clone of `publishers` so it can subscribe to patch changes
    /// after the node moves into the engine. The audio thread
    /// writes to `publishers.snapshot` + bumps `publishers.version`
    /// on every successful `ParamEvent` apply.
    pub fn with_patch_publishers(patch: SoftClipPatch, publishers: SoftClipPublishers) -> Self {
        Self {
            patch,
            patch_version: publishers.version,
            patch_snapshot: publishers.snapshot,
        }
    }

    /// Current threshold (test helper + future automation observer).
    pub fn threshold(&self) -> f32 {
        self.patch.threshold
    }

    /// Decode a parameter event and apply it to the runtime patch.
    /// Mirrors `WavetableSynthNode::apply_param`:
    ///
    /// 1. Decode the path. Unrecognized paths `debug_assert!` in
    ///    debug + no-op in release — a bad path is a host-side bug,
    ///    not an audio-time recoverable condition.
    /// 2. Apply via [`SoftClipParam::apply`] which clamps to the
    ///    valid range.
    /// 3. Write the new patch into `patch_snapshot` and bump
    ///    `patch_version` so host pollers see a fresh version.
    fn apply_param(&mut self, path: &[u8; 8], value: f32) {
        let Some(param) = SoftClipParam::decode(path) else {
            debug_assert!(false, "SoftClipNode: unknown ParamEvent path {path:?}");
            return;
        };
        param.apply(&mut self.patch, value);
        self.publish_patch();
    }

    /// Write the current `self.patch` into `patch_snapshot` and bump
    /// `patch_version` so host pollers see a fresh version. Brief
    /// `lock()` — `SoftClipPatch` is `Copy`, so the critical section
    /// is a memcpy. Version bump is `Release` so a host reader doing
    /// an `Acquire` load is guaranteed to see the snapshot write.
    fn publish_patch(&self) {
        if let Ok(mut slot) = self.patch_snapshot.lock() {
            *slot = self.patch;
        }
        self.patch_version.fetch_add(1, Ordering::Release);
    }
}

impl Default for SoftClipNode {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioNode for SoftClipNode {
    fn process(
        &mut self,
        ports: &mut PortAccess<'_>,
        events: &EventBlock<'_>,
        ctx: &ProcessContext,
    ) {
        // Apply any Param events that landed in this block before
        // shaping. Block-granular: a Param at sample 32 still
        // affects sample 0 of this block. v1 contract — the
        // host-side polling cadence (~20 Hz, X4) means parameter
        // changes are already sub-block-coarse, and the master FX
        // chain isn't expected to carry per-sample automation.
        for ev in events.iter() {
            if let BlockMessage::Param(ParamEvent { path, value }) = &ev.message {
                self.apply_param(path, *value);
            }
        }

        if ports.outputs.count() == 0 || ports.inputs.count() == 0 {
            return;
        }
        let block_size = ctx.block_size;
        let threshold = self.patch.threshold;
        let inv_threshold = 1.0 / threshold;

        // Snapshot input into output first (borrow-checker dance —
        // same as GainNode), then shape in place.
        let input = ports.inputs.get(0);
        let in_l = input.channel(0);
        let in_r = if input.channels() > 1 {
            input.channel(1)
        } else {
            input.channel(0)
        };

        let mut out = ports.outputs.get_mut(0);
        let (l, r) = out.stereo_mut();
        let len_l = block_size.min(l.len()).min(in_l.len());
        let len_r = block_size.min(r.len()).min(in_r.len());
        for i in 0..len_l {
            l[i] = threshold * (in_l[i] * inv_threshold).tanh();
        }
        for i in 0..len_r {
            r[i] = threshold * (in_r[i] * inv_threshold).tanh();
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
        // Memoryless — nothing to allocate.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_engine::event::BlockEventInBlock;
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

    fn run_node(node: &mut SoftClipNode, input: &[f32], events: &[BlockEventInBlock]) -> Vec<f32> {
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
        let evs = EventBlock::new(events);
        node.process(&mut ports, &evs, &ctx());
        output_buffers.into_iter().next().unwrap()
    }

    fn dc_signal(value: f32) -> Vec<f32> {
        let mut buf = vec![0.0; 2 * STRIDE];
        for s in buf.iter_mut().take(BLOCK) {
            *s = value;
        }
        for s in buf.iter_mut().skip(STRIDE).take(BLOCK) {
            *s = value;
        }
        buf
    }

    // ─── Patch + From + clamp ─────────────────────────────────────

    #[test]
    fn default_patch_threshold_is_0_7() {
        // Pins the audio-thread contract: a default-constructed
        // node must match the SoftClipData::default() threshold so
        // freshly-loaded and freshly-constructed projects sound the
        // same. If these drift, the audible cross-check between
        // rawdaw-model and rawdaw-fx breaks silently.
        assert_eq!(SoftClipPatch::default().threshold, DEFAULT_THRESHOLD);
        assert_eq!(SoftClipNode::new().threshold(), DEFAULT_THRESHOLD);
    }

    #[test]
    fn from_soft_clip_data_preserves_in_range_threshold() {
        let data = SoftClipData {
            format_version: 1,
            threshold: 0.5,
        };
        let patch: SoftClipPatch = data.into();
        assert_eq!(patch.threshold, 0.5);
    }

    #[test]
    fn from_soft_clip_data_clamps_out_of_range_threshold() {
        // A hand-edited project file (or future schema bug) could
        // produce out-of-range thresholds; the model → runtime
        // bridge must harden against that.
        let too_low: SoftClipPatch = SoftClipData {
            format_version: 1,
            threshold: -1.0,
        }
        .into();
        assert_eq!(too_low.threshold, MIN_THRESHOLD);

        let too_high: SoftClipPatch = SoftClipData {
            format_version: 1,
            threshold: 5.0,
        }
        .into();
        assert_eq!(too_high.threshold, MAX_THRESHOLD);
    }

    #[test]
    fn patch_new_clamps_threshold_to_valid_range() {
        assert_eq!(SoftClipPatch::new(0.0).threshold, MIN_THRESHOLD);
        assert_eq!(SoftClipPatch::new(1.0).threshold, MAX_THRESHOLD);
        assert_eq!(SoftClipPatch::new(0.5).threshold, 0.5);
    }

    // ─── Param encode / decode ────────────────────────────────────

    #[test]
    fn param_encode_decode_round_trips() {
        let p = SoftClipParam::Threshold;
        let bytes = p.encode();
        assert_eq!(bytes[0], SOFT_CLIP_FX_KIND);
        assert_eq!(bytes[1], SOFT_CLIP_PARAM_THRESHOLD);
        assert_eq!(SoftClipParam::decode(&bytes), Some(p));
    }

    #[test]
    fn param_decode_rejects_wrong_fx_kind() {
        // Byte 0 is the FX-kind discriminant. A path tagged for a
        // different FX (e.g. a future EQ) must NOT decode as a
        // SoftClipParam — silent misdispatch would corrupt the
        // chain's parameter state.
        let mut bytes = SoftClipParam::Threshold.encode();
        bytes[0] = 99;
        assert_eq!(SoftClipParam::decode(&bytes), None);
    }

    #[test]
    fn param_decode_rejects_unknown_subtag() {
        let mut bytes = SoftClipParam::Threshold.encode();
        bytes[1] = 99;
        assert_eq!(SoftClipParam::decode(&bytes), None);
    }

    #[test]
    fn param_apply_clamps_threshold() {
        let mut patch = SoftClipPatch::default();
        SoftClipParam::Threshold.apply(&mut patch, -1.0);
        assert_eq!(patch.threshold, MIN_THRESHOLD);
        SoftClipParam::Threshold.apply(&mut patch, 5.0);
        assert_eq!(patch.threshold, MAX_THRESHOLD);
        SoftClipParam::Threshold.apply(&mut patch, 0.3);
        assert_eq!(patch.threshold, 0.3);
    }

    // ─── Publisher contract ───────────────────────────────────────

    #[test]
    fn publisher_snapshot_and_version_update_on_param_apply() {
        // Mirrors the contract WavetableSynthNode::publish_patch
        // honors: every successful Param apply must (a) write the
        // current patch into the snapshot mutex and (b) bump the
        // version atomic with Release ordering. Pollers (X4) rely
        // on observing this transition.
        let publishers = SoftClipPublishers::new(SoftClipPatch::default());
        let observer_version = publishers.version.clone();
        let observer_snapshot = publishers.snapshot.clone();
        let mut node = SoftClipNode::with_patch_publishers(SoftClipPatch::default(), publishers);

        assert_eq!(observer_version.load(Ordering::Acquire), 0);
        assert_eq!(observer_snapshot.lock().unwrap().threshold, DEFAULT_THRESHOLD);

        let path = SoftClipParam::Threshold.encode();
        node.apply_param(&path, 0.3);

        assert_eq!(observer_version.load(Ordering::Acquire), 1);
        assert_eq!(observer_snapshot.lock().unwrap().threshold, 0.3);

        node.apply_param(&path, 0.5);
        assert_eq!(observer_version.load(Ordering::Acquire), 2);
        assert_eq!(observer_snapshot.lock().unwrap().threshold, 0.5);
    }

    // ─── DSP behavior ─────────────────────────────────────────────

    #[test]
    fn signal_below_threshold_passes_nearly_unchanged() {
        // Within ~half the threshold, the tanh is well-approximated
        // by `x` (linear region). The shape is `y = T * tanh(x/T)`;
        // at x=0.2, T=0.7: y = 0.7 * tanh(0.286) ≈ 0.7 * 0.278 ≈ 0.1944
        // — within 3% of the input. Pins that the clipper isn't
        // attenuating clean-headroom signal.
        let mut node = SoftClipNode::with_patch(SoftClipPatch::new(0.7));
        node.prepare(48_000, BLOCK);
        let input = dc_signal(0.2);
        let output = run_node(&mut node, &input, &[]);
        for &sample in output.iter().take(BLOCK) {
            assert!(
                (sample - 0.2).abs() < 0.01,
                "expected near-pass-through at 0.2 with threshold 0.7, got {sample}",
            );
        }
    }

    #[test]
    fn signal_above_threshold_is_soft_compressed_not_clipped() {
        // A full-scale input must come back attenuated toward the
        // threshold but NOT clipped to ±1.0 (which would be hard
        // clipping). With T=0.7, x=1.5: y = 0.7 * tanh(2.143) ≈
        // 0.7 * 0.973 ≈ 0.681. The compressed value sits between
        // ~T/2 and T — pins the asymptotic-to-threshold contract.
        let mut node = SoftClipNode::with_patch(SoftClipPatch::new(0.7));
        node.prepare(48_000, BLOCK);
        let input = dc_signal(1.5);
        let output = run_node(&mut node, &input, &[]);
        for &sample in output.iter().take(BLOCK) {
            assert!(
                sample < 0.7,
                "output should sit below threshold (0.7) for x=1.5, got {sample}",
            );
            assert!(
                sample > 0.35,
                "output should not be aggressively crushed for x=1.5, got {sample}",
            );
            assert!(sample.is_finite(), "output must be finite");
        }
    }

    #[test]
    fn mid_block_param_event_changes_shape_in_subsequent_block() {
        // End-to-end: a Param event reaching the node mid-stream
        // takes effect on subsequent samples. Process block 1 with
        // default threshold, then dispatch a Threshold event,
        // process block 2 at the new threshold, confirm amplitude
        // character differs. We choose a x=1.0 input — clearly
        // above either threshold — so both blocks soft-clip, but
        // to different asymptotes.
        let mut node = SoftClipNode::new();
        node.prepare(48_000, BLOCK);

        let input = dc_signal(1.0);
        let baseline = run_node(&mut node, &input, &[]);

        let path = SoftClipParam::Threshold.encode();
        let events = vec![BlockEventInBlock {
            offset_in_block: 0,
            message: BlockMessage::Param(ParamEvent { path, value: 0.2 }),
        }];
        let post_param = run_node(&mut node, &input, &events);

        // Baseline (T=0.7) and post-param (T=0.2) shapes must
        // differ meaningfully — at minimum, the post-param block's
        // amplitude should sit closer to 0.2 than to 0.7.
        let baseline_avg = baseline[0..BLOCK].iter().copied().sum::<f32>() / BLOCK as f32;
        let post_avg = post_param[0..BLOCK].iter().copied().sum::<f32>() / BLOCK as f32;
        assert!(
            baseline_avg > 0.5,
            "baseline with T=0.7, x=1.0 should compress to near 0.6, got {baseline_avg}"
        );
        assert!(
            post_avg < 0.25,
            "post-param with T=0.2, x=1.0 should compress to near 0.2, got {post_avg}"
        );
        // The new patch is also reflected in the node's published
        // threshold (X4 pollers depend on this).
        assert!((node.threshold() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn unknown_param_path_is_a_noop_in_release() {
        // Wrong FX-kind tag (byte 0) → decode returns None → apply
        // is a no-op. We can't test the debug_assert path here (it
        // panics in debug builds), so we just confirm the version
        // doesn't bump and the patch doesn't change — the audio-
        // thread effect of a bad path must be "nothing" in release
        // builds.
        //
        // Test runs in `cfg(debug_assertions)` by default with
        // `cargo test`, which would hit the `debug_assert!` panic;
        // skipped under debug to avoid being a panic-magnet.
        if cfg!(debug_assertions) {
            return;
        }
        let publishers = SoftClipPublishers::new(SoftClipPatch::default());
        let observer_version = publishers.version.clone();
        let mut node = SoftClipNode::with_patch_publishers(SoftClipPatch::default(), publishers);
        let mut bytes = SoftClipParam::Threshold.encode();
        bytes[0] = 99;
        node.apply_param(&bytes, 0.3);
        assert_eq!(observer_version.load(Ordering::Acquire), 0);
        assert_eq!(node.threshold(), DEFAULT_THRESHOLD);
    }

    #[test]
    fn descriptors_advertise_stereo_io() {
        let node = SoftClipNode::new();
        let inputs = node.input_descriptors();
        let outputs = node.output_descriptors();
        assert_eq!(inputs.len(), 1);
        assert_eq!(outputs.len(), 1);
        assert_eq!(inputs[0].channels, ChannelCount::Stereo);
        assert_eq!(outputs[0].channels, ChannelCount::Stereo);
    }
}
