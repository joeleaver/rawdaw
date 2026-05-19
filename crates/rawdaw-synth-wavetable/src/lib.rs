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

mod patch;
mod voice;
#[cfg(test)]
mod tests;

#[cfg(test)]
use rawdaw_dsp::{ModDestination, ModSlot, ModSource, WavetableOscParams};
use rawdaw_dsp::{SineLfo, Voice, VoicePool, Wavetable};
use rawdaw_engine::buffer::ChannelCount;
use rawdaw_engine::context::ProcessContext;
use rawdaw_engine::event::{BlockMessage, EventBlock};
use rawdaw_engine::node::{AudioNode, OutputDescriptor, PortAccess};
use rawdaw_model::{Midi2Message, U16Velocity};

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

/// The Vital-class wavetable synth's `AudioNode`.
///
/// Stateful: voices and shared LFO live across `process` calls. The
/// canonical [`WavetablePatch`] is owned by the node and mirrored
/// into each voice at `prepare()` time — voices read their own copy
/// so the audio thread never crosses the patch-source ↔ voice
/// boundary mid-tick.
pub struct WavetableSynthNode {
    wavetable: Wavetable,
    lfo: SineLfo,
    voices: VoicePool<WavetableVoice>,
    patch: WavetablePatch,
}

impl WavetableSynthNode {
    /// Build with the M5 default patch. Equivalent to
    /// `with_patch(WavetablePatch::default())`.
    pub fn new() -> Self {
        Self::with_patch(WavetablePatch::default())
    }

    /// Build with a specific runtime patch. Round-1's audio graph
    /// constructs through this path so each `Track::synth`'s patch
    /// becomes the voice's reading material.
    pub fn with_patch(patch: WavetablePatch) -> Self {
        Self {
            wavetable: Wavetable::saw_default(),
            lfo: SineLfo::new(),
            voices: VoicePool::new(NUM_VOICES, WavetableVoice::new),
            patch,
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
            BlockMessage::Param(_) => {
                // U1 ships the event channel; U3 wires the wavetable
                // synth's parameter decoder onto this arm. Until then
                // a Param event arriving here is a host-side bug — flag
                // it in debug, no-op in release.
                debug_assert!(
                    false,
                    "WavetableSynthNode received a Param event before U3; \
                     host should not be pushing params yet",
                );
            }
        }
    }

    /// Push the canonical patch (osc params + matrix slots + filter
    /// base) into every voice. Test-only — `prepare()` calls
    /// `WavetableVoice::prepare` directly, which folds patch
    /// propagation in. U3b's parameter event protocol will replace
    /// the test helpers with event-driven updates.
    #[cfg(test)]
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

