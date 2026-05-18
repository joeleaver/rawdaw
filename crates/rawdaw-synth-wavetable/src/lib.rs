//! rawdaw-synth-wavetable — the Vital-class wavetable+FM synth.
//!
//! v1 wires three wavetable oscillators per voice with PM/AM/RM
//! routing between them (modulator's osc index must exceed the
//! carrier's), one shared LFO routed to filter cutoff, a per-voice
//! state-variable lowpass, and a per-voice amp ADSR. Stereo output
//! mirrors L to R; spatialization comes later.
//!
//! ## Hardcoded patch
//!
//! Patches live as constants in this file until the parameter event
//! protocol lands (see the "Future plan needed: synth UI
//! integration" section of `docs/wavetable-synth-fm-plan.md`).
//!
//! - Wavetable: 128-harmonic band-limited saw (`Wavetable::saw_default`).
//! - Amp envelope: 5 ms / 80 ms / 0.7 / 200 ms ADSR.
//! - Filter: SVF lowpass, 1500 Hz cutoff, Q = 0.7.
//! - LFO: 4 Hz sine, ±400 Hz cutoff modulation.
//! - Per-osc params: see [`PATCH_OSC_PARAMS`] — F3's v0-equivalent
//!   single-osc patch; F5 replaces this with the v1 three-osc PM
//!   patch.

#![forbid(unsafe_code)]

mod voice;
#[cfg(test)]
mod tests;

use rawdaw_dsp::{
    AdsrParams, ModDestination, ModSlot, ModSource, SineLfo, Voice, VoicePool, Wavetable,
    WavetableOscParams,
};
use rawdaw_engine::buffer::ChannelCount;
use rawdaw_engine::context::ProcessContext;
use rawdaw_engine::event::{BlockMessage, EventBlock};
use rawdaw_engine::node::{AudioNode, OutputDescriptor, PortAccess};
use rawdaw_model::{Midi2Message, U16Velocity};

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

/// Empty slot — used to splat-fill the default patch's matrix
/// array. `ModSlot::default()` isn't `const`, so we open-code one.
pub(crate) const EMPTY_SLOT: ModSlot = ModSlot {
    source: ModSource::None,
    destination: ModDestination::FilterCutoff,
    amount: 0.0,
};

// ── Hardcoded patch ──────────────────────────────────────────────────────

/// Amp envelope (ENV1) — drives the voice's output amplitude and
/// gates voice lifecycle via `is_active()`. Values from v1.
const PATCH_ENV1_PARAMS: AdsrParams = AdsrParams {
    attack_s: 0.005,
    decay_s: 0.080,
    sustain_level: 0.7,
    release_s: 0.200,
};

/// Free envelope ENV2 — modulation-only. M1 adds it as a ticking
/// envelope whose output nothing consumes yet; M3 will route it
/// through the mod matrix (M5's default patch sends it at the
/// filter cutoff for a classic plucked-filter timbre, hence the
/// fast decay into zero sustain).
const PATCH_ENV2_PARAMS: AdsrParams = AdsrParams {
    attack_s: 0.005,
    decay_s: 0.250,
    sustain_level: 0.0,
    release_s: 0.050,
};

/// Free envelope ENV3 — modulation-only. Slower shape than ENV2
/// for a longer "movement" arc; M5 routes it as a downward PM-
/// amount modulator so the timbre brightens then mellows.
const PATCH_ENV3_PARAMS: AdsrParams = AdsrParams {
    attack_s: 0.005,
    decay_s: 0.400,
    sustain_level: 0.3,
    release_s: 0.200,
};

/// Filter cutoff base. Lowered from v1's 1500 Hz to 800 Hz so M5's
/// `ENV2 → FilterCutoff` slot has room to sweep upward at note onset
/// for a classic plucked-filter timbre; ENV2 + LFO together swing
/// the cutoff up into the brighter range, decaying back to the
/// dark 800 Hz over ~250 ms.
pub(crate) const PATCH_FILTER_CUTOFF_HZ: f32 = 800.0;
/// Filter resonance. v1 sat at 0.7 (effectively no peak — the
/// filter was a gentle slope). M5 bumps to 2.5 so the cutoff sweep
/// from `ENV2 → FilterCutoff` is audible as a resonant "wah", which
/// is what gives a plucked-filter envelope its character. The SVF
/// is stable at this Q; deep modulation can't push it unstable
/// because `set_cutoff` clamps the post-modulation frequency.
const PATCH_FILTER_RESONANCE: f32 = 2.5;
const PATCH_LFO_RATE_HZ: f32 = 4.0;
// LFO depth-to-cutoff coupling moved to a `ModSlot { source: Lfo1,
// destination: FilterCutoff, amount: 0.1 }` in `PATCH_MATRIX_SLOTS`
// (M3). `amount = 0.1` × `scale::FILTER_CUTOFF_HZ = 4000` reproduces
// v1's `PATCH_LFO_DEPTH_HZ = 400` Hz swing byte-for-byte.

/// v1 per-osc patch — three-osc chained PM stack.
///
/// - osc[0]: saw at played pitch, full mix level, phase-modulated by
///   osc[1] at depth 0.3 — the principal voice the listener hears.
/// - osc[1]: saw an octave above played pitch, contributes to the
///   audio mix at level 0.4, phase-modulated by osc[2] at depth
///   0.15 so its own spectrum is a touch brighter than pure saw.
/// - osc[2]: saw at a perfect twelfth (+19 semitones) above played
///   pitch, mix level 0.0 — exists purely as the top of the
///   modulator stack. Excluding it from the mix keeps the v1
///   spectrum from getting harsh while still giving osc[1] a
///   harmonically-interesting modulator.
///
/// Σ level = 1.4, so the headroom-preserving sum divides by 1.4 →
/// per-osc contribution caps at `osc / 1.4` peak. With osc[0] at its
/// natural ~1.0 peak, the pre-filter mix peaks around 1.0 / 1.4 ≈
/// 0.71 — comfortable headroom into the SVF + amp envelope.
///
/// Starting-point numbers; tune by ear via the round-1 fixture if
/// the timbre wants adjustment (F5 is explicitly ear-test driven).
const PATCH_OSC_PARAMS: [WavetableOscParams; NUM_OSCS] = [
    WavetableOscParams {
        tune_semitones: 0,
        fine_cents: 0,
        level: 1.0,
    },
    WavetableOscParams {
        tune_semitones: 12,
        fine_cents: 0,
        level: 0.4,
    },
    WavetableOscParams {
        tune_semitones: 19,
        fine_cents: 0,
        level: 0.0,
    },
];

/// M5's v2 default matrix. Slots:
///
/// - 0: `Lfo1 → FilterCutoff @ 0.1` — preserves v1's gentle LFO
///   wobble on the cutoff (0.1 × 4000 Hz = 400 Hz swing).
/// - 1: `Env2 → FilterCutoff @ 0.6` — ENV2 pluck shape sweeps the
///   cutoff from 800 Hz up to ~3200 Hz at attack peak, decaying
///   back to 800 Hz over 250 ms. Classic plucked-filter motion.
/// - 2: `Osc1 → PmAmountOf(0) @ 0.3` — v1's primary PM route.
/// - 3: `Osc2 → PmAmountOf(1) @ 0.15` — v1's secondary PM route.
/// - 4: `Env3 → PmAmountOf(0) @ -0.15` — ENV3's slow decay pulls
///   PM depth down at note onset (effective PM = 0.3 - 0.15 = 0.15
///   at attack peak, rising to 0.255 at ENV3 sustain). Note's
///   timbre starts cleaner and fills in as the modulation accent
///   decays. Sign is intentional — patches that prefer the
///   opposite phrasing (bright onset, mellow tail) flip to +0.15.
const PATCH_MATRIX_SLOTS: [ModSlot; MOD_MATRIX_SLOTS] = {
    let mut slots = [EMPTY_SLOT; MOD_MATRIX_SLOTS];
    slots[0] = ModSlot {
        source: ModSource::Lfo1,
        destination: ModDestination::FilterCutoff,
        amount: 0.1,
    };
    slots[1] = ModSlot {
        source: ModSource::Env2,
        destination: ModDestination::FilterCutoff,
        amount: 0.6,
    };
    slots[2] = ModSlot {
        source: ModSource::Osc1,
        destination: ModDestination::PmAmountOf(0),
        amount: 0.3,
    };
    slots[3] = ModSlot {
        source: ModSource::Osc2,
        destination: ModDestination::PmAmountOf(1),
        amount: 0.15,
    };
    slots[4] = ModSlot {
        source: ModSource::Env3,
        destination: ModDestination::PmAmountOf(0),
        amount: -0.15,
    };
    slots
};

// WavetableVoice and `Voice for WavetableVoice` live in `voice.rs` —
// split out to keep this file under the workspace ~700-line cap.

/// The Vital-class wavetable synth's `AudioNode`.
///
/// Stateful: voices and shared LFO live across `process` calls. The
/// wavetable is generated once at construction; future versions will
/// support a bank with patch-time selection. The per-osc patch
/// (`osc_params`) is owned by the node and mirrored into each voice
/// at `prepare()` time — voices read their own copy so the audio
/// thread never crosses the patch-source ↔ voice boundary mid-tick.
pub struct WavetableSynthNode {
    wavetable: Wavetable,
    lfo: SineLfo,
    voices: VoicePool<WavetableVoice>,
    osc_params: [WavetableOscParams; NUM_OSCS],
    matrix_slots: [ModSlot; MOD_MATRIX_SLOTS],
}

impl WavetableSynthNode {
    pub fn new() -> Self {
        Self {
            wavetable: Wavetable::saw_default(),
            lfo: SineLfo::new(),
            voices: VoicePool::new(NUM_VOICES, WavetableVoice::new),
            osc_params: PATCH_OSC_PARAMS,
            matrix_slots: PATCH_MATRIX_SLOTS,
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

    /// Push the canonical patch (osc params + matrix slots) into
    /// every voice. Called at `prepare()` and — until the parameter
    /// event protocol lands — by tests that want to install a
    /// non-default patch.
    fn propagate_patch_to_voices(&mut self) {
        for v in self.voices.voices_mut() {
            v.set_patch(&self.osc_params, &self.matrix_slots);
        }
    }

    /// Install a per-osc patch and propagate it to every voice.
    /// Test-only for v1/v2 — the parameter event protocol will
    /// replace this with an event-driven path so the audio thread
    /// can update without coordination from the host.
    #[cfg(test)]
    pub(crate) fn set_patch_for_test(
        &mut self,
        params: [WavetableOscParams; NUM_OSCS],
    ) {
        self.osc_params = params;
        self.propagate_patch_to_voices();
    }

    /// Install a matrix slot configuration and propagate it to
    /// every voice. Test-only. Used by M3+ tests that need to vary
    /// the matrix without changing osc params.
    #[cfg(test)]
    pub(crate) fn set_matrix_for_test(
        &mut self,
        slots: [ModSlot; MOD_MATRIX_SLOTS],
    ) {
        self.matrix_slots = slots;
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
        self.lfo.set_rate_hz(PATCH_LFO_RATE_HZ);

        // Prepare every voice's per-voice state.
        for v in self.voices.voices_mut() {
            for osc in &mut v.oscs {
                osc.prepare(sample_rate);
            }
            v.amp.prepare(sample_rate);
            v.amp.set_params(PATCH_ENV1_PARAMS);
            v.env2.prepare(sample_rate);
            v.env2.set_params(PATCH_ENV2_PARAMS);
            v.env3.prepare(sample_rate);
            v.env3.set_params(PATCH_ENV3_PARAMS);
            v.filter.prepare(sample_rate);
            v.filter.set_cutoff(PATCH_FILTER_CUTOFF_HZ);
            v.filter.set_resonance(PATCH_FILTER_RESONANCE);
        }

        // Push the node's patch into every voice so they're in sync
        // before the first event lands.
        self.propagate_patch_to_voices();
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

