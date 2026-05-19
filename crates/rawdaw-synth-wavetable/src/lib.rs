//! rawdaw-synth-wavetable — the Vital-class wavetable+FM synth.
//!
//! Three wavetable oscillators per voice + three envelopes + one
//! shared LFO + a per-voice SVF lowpass, all routed through a
//! 16-slot mod matrix. The runtime patch lives in [`WavetablePatch`];
//! the serialized mirror lives in `rawdaw-model::patch::wavetable`.
//!
//! Construct via [`WavetableSynthNode::with_patch`] when the host has
//! patch data to install (the round-1 audio path constructs from
//! `track.synth`); [`WavetableSynthNode::new`] is the convenience
//! delegate that picks the M5 default.

#![forbid(unsafe_code)]

mod param;
mod patch;
mod voice;
#[cfg(test)]
mod tests;

#[cfg(test)]
use rawdaw_dsp::{ModSlot, WavetableOscParams};
use rawdaw_dsp::{SineLfo, Voice, VoicePool, Wavetable};
use rawdaw_engine::buffer::ChannelCount;
use rawdaw_engine::context::ProcessContext;
use rawdaw_engine::event::{BlockMessage, EventBlock, ParamEvent};
use rawdaw_engine::node::{AudioNode, OutputDescriptor, PortAccess};
use rawdaw_model::{Midi2Message, U16Velocity};

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub use param::{
    encode_mod_destination, encode_mod_source,
    patch_to_param_events as wavetable_patch_to_param_events, WavetableParam,
};

/// Re-export the mod-matrix vocabulary so the UI editor can build
/// dropdowns without adding a direct `rawdaw-dsp` dep on the app
/// crate. Both enums are `pub` in `rawdaw-dsp::modulation::matrix`;
/// surfacing them here lets `rawdaw-app` go through the synth crate
/// as a single boundary.
pub use rawdaw_dsp::{ModDestination, ModSource};
pub use patch::WavetablePatch;
use voice::WavetableVoice;

/// Number of voices. Matches `SineNode` so the wavetable synth feels
/// identical at the queue layer when it replaces the sine in
/// rawdaw-app's track routing.
const NUM_VOICES: usize = 16;

/// Oscillators per voice. Vital-style: three is enough for chained
/// PM (osc[2] → osc[1] → osc[0]) plus an independent parallel layer
/// when patches want it; the routing-constraint rule (modulator
/// index > carrier index) keeps render order trivial at this width.
pub(crate) const NUM_OSCS: usize = 3;

/// Modulation matrix slot count. Generous for a small synth, RT-
/// safe (no heap), well under the topo-sort scratch-buffer budget
/// in `rawdaw-dsp::modulation`.
pub(crate) const MOD_MATRIX_SLOTS: usize = 16;

/// Empty slot — used by tests to clear the matrix.
/// `ModSlot::default()` isn't `const`, so we open-code one.
#[cfg(test)]
pub(crate) const EMPTY_SLOT: ModSlot = ModSlot {
    source: ModSource::None,
    destination: ModDestination::FilterCutoff,
    amount: 0.0,
};

// WavetableVoice and `Voice for WavetableVoice` live in `voice.rs` —
// split out to keep this file under the workspace ~700-line cap.

/// Audio-thread → host publishers for one [`WavetableSynthNode`].
///
/// The synth node owns the canonical [`WavetablePatch`]; this struct
/// publishes a snapshot for the UI to read so editor sliders can
/// reflect the live patch state (including future updates driven by
/// MIDI Learn / automation, not just by the editor itself).
///
/// - `version` increments on every successful `ParamEvent` apply.
///   Pollers watch this to detect that they need to re-snapshot.
/// - `snapshot` holds the most recent post-apply patch. The audio
///   thread takes a brief `lock()` to write it on every Param event;
///   the host takes a brief `lock()` to read it on each poll. The
///   `WavetablePatch` is `Copy` (~few hundred bytes), so the
///   critical section is a memcpy — fast enough that the worst-case
///   audio-thread block (a ~µs while a poll is in flight) is well
///   under one audio block at 44.1 kHz. The cleaner triple-buffer /
///   `arc-swap` rework is a known future optimization; measurement
///   should justify it before we add a dependency.
///
/// Constructed by the host via [`WavetablePublishers::new`] and
/// passed into [`WavetableSynthNode::with_patch_publishers`]. The
/// host keeps a clone (the Arcs make this cheap) so it can read the
/// version + snapshot after the synth node has moved into the
/// engine.
#[derive(Clone)]
pub struct WavetablePublishers {
    pub version: Arc<AtomicU64>,
    pub snapshot: Arc<Mutex<WavetablePatch>>,
}

impl WavetablePublishers {
    /// Build a fresh publishers pair seeded with `initial`. Version
    /// starts at 0; the snapshot starts at the initial patch so the
    /// host can read the boot state without a `prepare()` callback
    /// having fired.
    pub fn new(initial: WavetablePatch) -> Self {
        Self {
            version: Arc::new(AtomicU64::new(0)),
            snapshot: Arc::new(Mutex::new(initial)),
        }
    }
}

/// The Vital-class wavetable synth's `AudioNode`.
///
/// Stateful: voices and shared LFO live across `process` calls. The
/// canonical [`WavetablePatch`] is owned by the node and mirrored
/// into each voice at `prepare()` time — voices read their own copy
/// so the audio thread never crosses the patch-source ↔ voice
/// boundary mid-tick.
///
/// Patch publishers (`patch_version`, `patch_snapshot`) sit alongside
/// so the host's UI can observe the audio thread's patch state
/// without holding a reference to the node itself (which moves into
/// the engine after construction). See [`WavetablePublishers`].
pub struct WavetableSynthNode {
    wavetable: Wavetable,
    lfo: SineLfo,
    voices: VoicePool<WavetableVoice>,
    patch: WavetablePatch,
    /// Bumped on every successful `ParamEvent` apply. Pollers watch
    /// this to know when the snapshot is fresh.
    patch_version: Arc<AtomicU64>,
    /// Latest post-apply patch — mirrors `self.patch` after every
    /// `ParamEvent`. Host reads via `lock()`.
    patch_snapshot: Arc<Mutex<WavetablePatch>>,
}

impl WavetableSynthNode {
    /// Build with the M5 default patch. Equivalent to
    /// `with_patch(WavetablePatch::default())`. Publishers are
    /// created internally and dropped when this node drops; tests
    /// and historical callers that don't need them stay terse.
    pub fn new() -> Self {
        Self::with_patch(WavetablePatch::default())
    }

    /// Build with a specific runtime patch. Publishers are created
    /// internally — callers that need to observe the patch from the
    /// host side use [`Self::with_patch_publishers`] instead.
    pub fn with_patch(patch: WavetablePatch) -> Self {
        let pubs = WavetablePublishers::new(patch);
        Self::with_patch_publishers(patch, pubs)
    }

    /// Build with patch + host-owned publishers. The host keeps a
    /// clone of `publishers` so it can subscribe to patch changes
    /// after the node moves into the engine. The audio thread writes
    /// to `publishers.snapshot` + bumps `publishers.version` on every
    /// successful `ParamEvent` apply.
    pub fn with_patch_publishers(patch: WavetablePatch, publishers: WavetablePublishers) -> Self {
        Self {
            wavetable: Wavetable::saw_default(),
            lfo: SineLfo::new(),
            voices: VoicePool::new(NUM_VOICES, WavetableVoice::new),
            patch,
            patch_version: publishers.version,
            patch_snapshot: publishers.snapshot,
        }
    }

    fn apply_event(&mut self, message: &BlockMessage) {
        match message {
            BlockMessage::Midi(Midi2Message::NoteOn { note, velocity, .. }) => {
                let amp = u16_velocity_to_amplitude(*velocity);
                self.voices.note_on(note.get(), amp);
            }
            BlockMessage::Midi(Midi2Message::NoteOff { note, .. }) => {
                self.voices.note_off(note.get());
            }
            BlockMessage::Param(ParamEvent { path, value }) => {
                self.apply_param(path, *value);
            }
        }
    }

    /// Decode a parameter event and apply it to the runtime patch.
    /// Unrecognized paths `debug_assert!` in debug + no-op in release
    /// — a bad path is a host-side bug, not an audio-time recoverable
    /// condition. After mutation:
    ///
    /// 1. Propagate the new patch to every voice so subsequent
    ///    samples see the change.
    /// 2. Write the new patch into `patch_snapshot` and bump
    ///    `patch_version` so host pollers can re-snapshot.
    fn apply_param(&mut self, path: &[u8; 8], value: f32) {
        let Some(param) = WavetableParam::decode(path) else {
            debug_assert!(false, "WavetableSynthNode: unknown ParamEvent path {path:?}");
            return;
        };
        param.apply(&mut self.patch, value);
        self.propagate_patch_to_voices();
        self.publish_patch();
    }

    /// Write the current `self.patch` into `patch_snapshot` and bump
    /// `patch_version` so host pollers see a fresh version. Brief
    /// `lock()` — `WavetablePatch` is `Copy`, so the critical section
    /// is a memcpy. Version bump is `Release` so a host reader doing
    /// an `Acquire` load is guaranteed to see the snapshot write.
    fn publish_patch(&self) {
        if let Ok(mut slot) = self.patch_snapshot.lock() {
            *slot = self.patch;
        }
        self.patch_version.fetch_add(1, Ordering::Release);
    }

    /// Push the canonical patch (osc params + matrix slots + filter
    /// base) into every voice. Called by `apply_param` after each
    /// parameter mutation and by the test helpers below.
    fn propagate_patch_to_voices(&mut self) {
        for v in self.voices.voices_mut() {
            v.set_patch(&self.patch);
        }
    }

    /// Replace the per-osc parameters and propagate. Test-only until
    /// U3b lifts the parameter event protocol into a single path.
    #[cfg(test)]
    pub(crate) fn set_patch_for_test(
        &mut self,
        params: [WavetableOscParams; NUM_OSCS],
    ) {
        self.patch.osc_params = params;
        self.propagate_patch_to_voices();
    }

    /// Replace the matrix slots and propagate. Test-only.
    #[cfg(test)]
    pub(crate) fn set_matrix_for_test(
        &mut self,
        slots: [ModSlot; MOD_MATRIX_SLOTS],
    ) {
        self.patch.matrix = slots;
        self.propagate_patch_to_voices();
    }
}

impl Default for WavetableSynthNode {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioNode for WavetableSynthNode {
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
            // Drain every event whose offset has now been reached.
            while next_event < event_slice.len()
                && (event_slice[next_event].offset_in_block as usize) <= i
            {
                let msg = &event_slice[next_event].message;
                self.apply_event(msg);
                next_event += 1;
            }

            // Shared LFO tick once per sample; all voices read the
            // same modulation value, matching a typical analog synth's
            // global LFO routing.
            let lfo_value = self.lfo.tick();

            let mut sample = 0.0_f32;
            for v in self.voices.voices_mut() {
                if !v.is_active() {
                    continue;
                }
                sample += v.tick(&self.wavetable, lfo_value);
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
        // Configure shared modulation.
        self.lfo.prepare(sample_rate);
        self.lfo.set_rate_hz(self.patch.lfo_rate_hz);

        // Prepare every voice's per-voice state from the canonical
        // patch — `WavetableVoice::prepare` covers sample-rate setup
        // *and* installs envelope params, filter coefficients, osc
        // params, matrix slots in one call.
        for v in self.voices.voices_mut() {
            v.prepare(sample_rate, &self.patch);
        }
    }
}

/// Per-voice headroom: each voice peaks at `PER_VOICE_HEADROOM` times
/// the velocity-normalized amplitude. Three full-velocity voices
/// summed still leave ~0.4 of headroom under the master GainNode,
/// preventing pre-clip in the mixer. v1 should replace this with a
/// proper voice-level VCA and a soft-clipper at the master.
const PER_VOICE_HEADROOM: f32 = 0.5;

fn u16_velocity_to_amplitude(v: U16Velocity) -> f32 {
    (v.get() as f32 / u16::MAX as f32) * PER_VOICE_HEADROOM
}

