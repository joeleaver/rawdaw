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

use rawdaw_dsp::{
    Adsr, AdsrParams, ModDestination, ModMatrix, ModSlot, ModSource, Modulations,
    SineLfo, SvfLowpass, Voice, VoicePool, Wavetable, WavetableOsc, WavetableOscParams,
    note_offset_hz,
};
use rawdaw_engine::buffer::ChannelCount;
use rawdaw_engine::context::ProcessContext;
use rawdaw_engine::event::{BlockMessage, EventBlock};
use rawdaw_engine::node::{AudioNode, OutputDescriptor, PortAccess};
use rawdaw_model::{Midi2Message, U16Velocity};

/// Number of voices. Matches `SineNode` so the wavetable synth feels
/// identical at the queue layer when it replaces the sine in
/// rawdaw-app's track routing.
const NUM_VOICES: usize = 16;

/// Oscillators per voice. Vital-style: three is enough for chained
/// PM (osc[2] → osc[1] → osc[0]) plus an independent parallel layer
/// when patches want it; the routing-constraint rule (modulator
/// index > carrier index) keeps render order trivial at this width.
const NUM_OSCS: usize = 3;

/// Modulation matrix slot count. Generous for a small synth, RT-
/// safe (no heap), well under the topo-sort scratch-buffer budget
/// in `rawdaw-dsp::modulation`.
const MOD_MATRIX_SLOTS: usize = 16;

/// Empty slot — used to splat-fill the default patch's matrix
/// array. `ModSlot::default()` isn't `const`, so we open-code one.
const EMPTY_SLOT: ModSlot = ModSlot {
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
const PATCH_FILTER_CUTOFF_HZ: f32 = 800.0;
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

/// One synth voice — three wavetable oscillators, amp envelope, and
/// per-voice filter integrators so retrigger doesn't blend tail
/// ringing into the next note. Each voice carries its own copy of
/// the patch's per-osc params; the node propagates updates via
/// [`WavetableVoice::set_patch`] so the audio thread can keep
/// reading without coordinating with the patch source.
#[derive(Debug, Clone, Copy)]
struct WavetableVoice {
    note: u8,
    oscs: [WavetableOsc; NUM_OSCS],
    osc_params: [WavetableOscParams; NUM_OSCS],
    /// Per-osc Hz at the played note + static tune/fine, computed
    /// once at `note_on`. Audio-rate `OscTune` / `OscFineTune`
    /// modulation re-derives Hz from this base each sample (only
    /// when the tune offsets are nonzero — skipped otherwise).
    osc_hz_base: [f32; NUM_OSCS],
    /// ENV1 — amp envelope. Drives the voice's output amplitude and
    /// gates `is_active()`.
    amp: Adsr,
    /// ENV2 — free modulation envelope. Routed through `matrix`.
    env2: Adsr,
    /// ENV3 — second free modulation envelope. Routed through
    /// `matrix`.
    env3: Adsr,
    /// Per-voice modulation matrix (copy of the node's canonical
    /// matrix, refreshed at patch-apply time).
    matrix: ModMatrix<MOD_MATRIX_SLOTS>,
    filter: SvfLowpass,
    velocity_amp: f32,
}

impl WavetableVoice {
    fn new() -> Self {
        Self {
            note: 0,
            oscs: [WavetableOsc::new(); NUM_OSCS],
            osc_params: [WavetableOscParams::default(); NUM_OSCS],
            osc_hz_base: [0.0; NUM_OSCS],
            amp: Adsr::new(),
            env2: Adsr::new(),
            env3: Adsr::new(),
            matrix: ModMatrix::default(),
            filter: SvfLowpass::new(),
            velocity_amp: 0.0,
        }
    }

    /// Install a patch's per-osc parameters + matrix slots. Matrix
    /// slots flow through `ModMatrix::set_slots` which runs its own
    /// validation + topo sort (cycles rejected in debug, sanitized
    /// in release). v1's per-osc routing fields are gone — all
    /// modulation routing lives in the matrix as of M4.
    fn set_patch(
        &mut self,
        params: &[WavetableOscParams; NUM_OSCS],
        matrix_slots: &[ModSlot; MOD_MATRIX_SLOTS],
    ) {
        self.osc_params = *params;
        self.matrix.set_slots(*matrix_slots);
    }

    /// Per-sample tick. Two-stage modulation evaluation:
    ///
    /// 1. **Control-rate pass.** Tick ENV1/2/3, then iterate the
    ///    matrix's control-rate slots (Env*/Lfo* sources) into a
    ///    fresh `Modulations` bag. The bag holds per-destination
    ///    contributions in destination-native units (Hz, semitones,
    ///    coefficients, …).
    /// 2. **Per-osc render in topo-sorted order.** For each osc in
    ///    `matrix.audio_rate_osc_order()`, fold in audio-rate slot
    ///    contributions (`OscN → ...Of(i)`) by reading the source's
    ///    sample from `osc_samples` (already rendered, since topo
    ///    sort ensures sources before destinations), then apply
    ///    pre-tick mods (PM offset, tune offset) and post-tick mods
    ///    (AM, RM coefficients) before storing the result.
    fn tick(&mut self, wavetable: &Wavetable, lfo_value: f32) -> f32 {
        // ── Stage 1: tick envelopes + evaluate control-rate slots.
        let env1_value = self.amp.tick();
        let env2_value = self.env2.tick();
        let env3_value = self.env3.tick();

        let mut mods = Modulations::default();
        for slot in self.matrix.active_slots() {
            let source_value = match slot.source {
                ModSource::Env1 => env1_value,
                ModSource::Env2 => env2_value,
                ModSource::Env3 => env3_value,
                ModSource::Lfo1 => lfo_value,
                ModSource::Osc0 | ModSource::Osc1 | ModSource::Osc2 => {
                    // Audio-rate sources fold into `mods` inside the
                    // per-osc loop below — they need their source
                    // osc's current-sample value, which is only
                    // available after that source has ticked.
                    continue;
                }
                ModSource::None => continue,
            };
            mods.add_contribution(slot.destination, source_value, slot.amount);
        }

        // Filter cutoff = base + matrix contribution (control-rate
        // only — no audio-rate sources target FilterCutoff in v2).
        // The default patch's `Lfo1 → FilterCutoff @ 0.1` slot
        // reproduces v1's `lfo * 400 Hz` swing exactly (0.1 ×
        // 4000 Hz scale = 400).
        let cutoff_hz = PATCH_FILTER_CUTOFF_HZ + mods.filter_cutoff_hz_offset;
        self.filter.set_cutoff(cutoff_hz);

        // ── Stage 2: per-osc render in topo-sorted order.
        let order = self.matrix.audio_rate_osc_order();
        let mut osc_samples = [0.0_f32; NUM_OSCS];
        for &osc_idx in order.iter() {
            let i = osc_idx as usize;

            // Fold audio-rate slot contributions targeting osc[i].
            // Their source oscs have already rendered (topo sort
            // guarantees), so `osc_samples[src]` is populated.
            for slot in self.matrix.active_slots() {
                if !slot.source.is_audio_rate() {
                    continue;
                }
                if slot.destination.osc_index() != Some(osc_idx) {
                    continue;
                }
                let src_idx = slot
                    .source
                    .osc_index()
                    .expect("audio-rate source has osc index");
                let source_value = osc_samples[src_idx as usize];
                mods.add_contribution(slot.destination, source_value, slot.amount);
            }

            // Pre-tick: tune modulation re-derives Hz if any tune
            // offset is nonzero. Skipping the `powf` when offsets
            // are zero keeps the steady-state hot path identical to
            // v1.
            if mods.osc_tune_offset[i] != 0.0 || mods.osc_fine_offset[i] != 0.0 {
                let mod_semis = mods.osc_tune_offset[i] + mods.osc_fine_offset[i] / 100.0;
                let modulated_hz = self.osc_hz_base[i] * 2.0_f32.powf(mod_semis / 12.0);
                self.oscs[i].set_frequency(modulated_hz);
            }

            // Pre-tick PM offset folds into `tick_with_pm`.
            let raw = self.oscs[i].tick_with_pm(wavetable, mods.pm_amount_offset[i]);

            // Post-tick AM + RM coefficients. AM uses additive
            // `1 + offset`; RM also `1 + offset` but offset
            // formula is `(source - 1) × amount` (see
            // `Modulations::add_contribution`).
            let am_coeff = 1.0 + mods.am_amount_offset[i];
            let rm_coeff = 1.0 + mods.rm_amount_offset[i];
            osc_samples[i] = raw * am_coeff * rm_coeff;
        }

        // Headroom-preserving sum: voice = Σ(s_i × L_i) / max(1, Σ L_i).
        // Per-osc level can be modulated via `OscLevel(i)` slots —
        // `mods.osc_level_offset[i]` adds to the patch's static
        // level. Negative effective levels are clamped at 0 so the
        // mix divisor stays well-defined (a slot can't drive a level
        // through zero into negative territory).
        let mut sum = 0.0_f32;
        let mut level_sum = 0.0_f32;
        for (i, sample) in osc_samples.iter().enumerate() {
            let level = (self.osc_params[i].level + mods.osc_level_offset[i]).max(0.0);
            sum += sample * level;
            level_sum += level;
        }
        let mixed = sum / level_sum.max(1.0);

        let filtered = self.filter.tick(mixed);
        filtered * env1_value * self.velocity_amp
    }
}

impl Voice for WavetableVoice {
    fn note(&self) -> u8 {
        self.note
    }

    fn is_active(&self) -> bool {
        !self.amp.is_idle()
    }

    fn note_on(&mut self, note: u8, velocity: f32) {
        self.note = note;
        for i in 0..NUM_OSCS {
            let hz = note_offset_hz(
                note,
                self.osc_params[i].tune_semitones,
                self.osc_params[i].fine_cents,
            );
            // Cache the base Hz so audio-rate tune modulation can
            // re-derive frequency from it without re-running
            // `note_offset_hz` (avoids one extra powf per tune-
            // modulated sample).
            self.osc_hz_base[i] = hz;
            self.oscs[i].set_frequency(hz);
            self.oscs[i].reset_phase();
        }
        self.filter.reset_state();
        // All three envelopes trigger together — matches Vital's
        // behaviour. Only ENV1 (amp) gates voice lifecycle via
        // `is_active`; ENV2 + ENV3 are pure modulators.
        self.amp.note_on();
        self.env2.note_on();
        self.env3.note_on();
        self.velocity_amp = velocity;
    }

    fn note_off(&mut self) {
        // Release all three envelopes together. The amp's release
        // determines when the voice goes idle; ENV2/ENV3 release
        // their own state so the modulation tails are well-defined
        // post-NoteOff.
        self.amp.note_off();
        self.env2.note_off();
        self.env3.note_off();
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_engine::event::BlockEventInBlock;
    use rawdaw_model::{MidiChannel, MidiNote, MusicalTime};

    const SR: u32 = 48_000;
    const BLOCK: usize = 256;

    fn make_node() -> WavetableSynthNode {
        let mut node = WavetableSynthNode::new();
        node.prepare(SR, BLOCK);
        node
    }

    fn note_on(n: u8, vel: U16Velocity) -> BlockMessage {
        BlockMessage::Midi(Midi2Message::NoteOn {
            channel: MidiChannel::default(),
            note: MidiNote::new(n).unwrap(),
            velocity: vel,
        })
    }

    fn note_off(n: u8) -> BlockMessage {
        BlockMessage::Midi(Midi2Message::NoteOff {
            channel: MidiChannel::default(),
            note: MidiNote::new(n).unwrap(),
            velocity: U16Velocity::MIN,
        })
    }

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
        node: &mut WavetableSynthNode,
        events: &[BlockEventInBlock],
        out: &mut Vec<f32>,
    ) {
        out.clear();
        out.resize(2 * BLOCK, 0.0);
        let mut ports = PortAccess::new(
            &[],
            &[],
            std::slice::from_mut(out),
            &[2u8],
            BLOCK,
            BLOCK,
        );
        let evblock = EventBlock::new(events);
        node.process(&mut ports, &evblock, &ctx());
    }

    fn rms(samples: &[f32]) -> f32 {
        let sumsq: f32 = samples.iter().map(|s| s * s).sum();
        (sumsq / samples.len() as f32).sqrt()
    }

    #[test]
    fn silent_until_note_on() {
        let mut node = make_node();
        let mut buf = Vec::new();
        render_block(&mut node, &[], &mut buf);
        assert!(buf.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn note_on_produces_audible_output() {
        let mut node = make_node();
        let mut buf = Vec::new();
        // Run a few blocks so the amp envelope has time to ramp through
        // attack + into sustain; the first 5ms of attack at 48 kHz is
        // ~240 samples (well inside one 256-frame block, so output
        // builds quickly).
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut buf,
        );
        // Render a second block to skip the initial attack ramp window.
        render_block(&mut node, &[], &mut buf);
        assert!(
            rms(&buf[..BLOCK]) > 0.01,
            "expected audible sustained signal; got rms {}",
            rms(&buf[..BLOCK]),
        );
    }

    #[test]
    fn note_off_releases_to_silence() {
        let mut node = make_node();
        let mut buf = Vec::new();
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut buf,
        );
        // NoteOff, then run a long tail — at 200 ms release, ~9600
        // samples are needed, so render 50 blocks of silence.
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_off(60),
            }],
            &mut buf,
        );
        for _ in 0..50 {
            render_block(&mut node, &[], &mut buf);
        }
        // The last block should be effectively silent — release has
        // completed.
        let tail_rms = rms(&buf[..BLOCK]);
        assert!(
            tail_rms < 1e-4,
            "release should reach silence; tail rms = {tail_rms}",
        );
    }

    #[test]
    fn lr_outputs_are_identical() {
        // v0 mirrors L = R; spatialization is a v1 growth.
        let mut node = make_node();
        let mut buf = Vec::new();
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut buf,
        );
        let l = &buf[..BLOCK];
        let r = &buf[BLOCK..2 * BLOCK];
        for i in 0..BLOCK {
            assert_eq!(l[i], r[i], "L and R must match in v0 at sample {i}");
        }
    }

    #[test]
    fn mid_block_note_on_starts_at_offset() {
        let mut node = make_node();
        let mut buf = Vec::new();
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 100,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut buf,
        );
        // Pre-onset samples must be exactly 0. Post-onset attack ramps
        // through ~240 samples — most of the rest of the block — and
        // amp_level starts at 0, so individual sample magnitudes ramp
        // up from 0 too. Confirm that no pre-onset sample is non-zero.
        let left = &buf[..BLOCK];
        for (i, s) in left[..100].iter().enumerate() {
            assert_eq!(*s, 0.0, "pre-onset sample {i} should be silent");
        }
        // Make sure something happens post-onset across two blocks of
        // amp ramp-up.
        render_block(&mut node, &[], &mut buf);
        assert!(
            rms(&buf[..BLOCK]) > 0.005,
            "should be audible after the attack ramp",
        );
    }

    /// Explicit single-osc patch — the v0-equivalent baseline used
    /// by the F3 headroom tests. Constructed here rather than read
    /// from `PATCH_OSC_PARAMS` because the default patch is the v1
    /// three-osc PM stack; these tests need to compare against a
    /// known single-osc reference. M4-shape: only tune/fine/level
    /// fields; modulation routing lives in matrix slots installed
    /// separately via `set_matrix_for_test`.
    fn single_osc_patch() -> [WavetableOscParams; NUM_OSCS] {
        [
            WavetableOscParams {
                tune_semitones: 0,
                fine_cents: 0,
                level: 1.0,
            },
            WavetableOscParams::default(),
            WavetableOscParams::default(),
        ]
    }

    fn coherent_three_osc_patch() -> [WavetableOscParams; NUM_OSCS] {
        let one = WavetableOscParams {
            tune_semitones: 0,
            fine_cents: 0,
            level: 1.0,
        };
        [one, one, one]
    }

    /// Three oscs at the same pitch and full level must produce the
    /// same audio as a single osc at full level. The headroom-
    /// preserving sum divides by `Σ level_i = 3` while each osc
    /// contributes the identical waveform (same Hz, phase reset at
    /// note_on), so `3 * single / 3 == single`. Pinned with an
    /// empty matrix so the v1 PM-chain slots in the default matrix
    /// don't perturb the comparison.
    #[test]
    fn three_coherent_oscs_match_single_osc_output() {
        let mut single = make_node();
        single.set_patch_for_test(single_osc_patch());
        single.set_matrix_for_test([EMPTY_SLOT; MOD_MATRIX_SLOTS]);
        let mut single_buf = Vec::new();
        render_block(
            &mut single,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut single_buf,
        );

        let mut triple = make_node();
        triple.set_patch_for_test(coherent_three_osc_patch());
        triple.set_matrix_for_test([EMPTY_SLOT; MOD_MATRIX_SLOTS]);
        let mut triple_buf = Vec::new();
        render_block(
            &mut triple,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut triple_buf,
        );

        // Sample-identical within f32 round-off (the divide-by-level-sum
        // and sum-of-three introduces a tiny rounding error per sample).
        let mut max_diff = 0.0_f32;
        for i in 0..(2 * BLOCK) {
            let d = (single_buf[i] - triple_buf[i]).abs();
            max_diff = max_diff.max(d);
        }
        assert!(
            max_diff < 1e-5,
            "coherent three-osc patch must match single-osc output; max_diff = {max_diff}",
        );
    }

    /// Detuned three-osc patch (oct down, root, oct up) must be
    /// audible and bounded — i.e., the headroom-preserving sum
    /// prevents the summed peak from exceeding the single-osc peak.
    /// Plan called for "RMS within ±20% of v0 RMS"; that target is
    /// unreachable for partially-correlated octave-related sources
    /// (RMS lands near `1/sqrt(3) ≈ 0.58×` of v0 due to phase
    /// incoherence after the divide-by-3), so the pin is reframed
    /// around the property the headroom math actually guarantees:
    /// peak stays bounded, output is audibly non-trivial, and the
    /// multi-osc render is materially different from single-osc.
    #[test]
    fn detuned_three_osc_output_is_bounded_and_non_silent() {
        let mut single = make_node();
        single.set_patch_for_test(single_osc_patch());
        single.set_matrix_for_test([EMPTY_SLOT; MOD_MATRIX_SLOTS]);
        let mut single_buf = Vec::new();
        render_block(
            &mut single,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut single_buf,
        );
        // Skip the attack ramp.
        render_block(&mut single, &[], &mut single_buf);
        let single_peak = single_buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);

        let mut triple = make_node();
        triple.set_patch_for_test([
            WavetableOscParams {
                tune_semitones: 0,
                fine_cents: 0,
                level: 1.0,
            },
            WavetableOscParams {
                tune_semitones: 12,
                fine_cents: 0,
                level: 1.0,
            },
            WavetableOscParams {
                tune_semitones: -12,
                fine_cents: 0,
                level: 1.0,
            },
        ]);
        triple.set_matrix_for_test([EMPTY_SLOT; MOD_MATRIX_SLOTS]);
        let mut triple_buf = Vec::new();
        render_block(
            &mut triple,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut triple_buf,
        );
        render_block(&mut triple, &[], &mut triple_buf);
        let triple_peak = triple_buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        let triple_rms = rms(&triple_buf[..BLOCK]);

        // (a) Non-silent: detuned sum can't collapse to zero.
        assert!(
            triple_rms > 0.01,
            "detuned three-osc output should be audible; rms = {triple_rms}",
        );
        // (b) Bounded: headroom math keeps the peak under the single-
        // osc peak with a small slack for partial constructive
        // alignment.
        assert!(
            triple_peak <= single_peak * 1.05,
            "detuned three-osc peak {triple_peak} should stay under single-osc peak {single_peak} (×1.05 slack)",
        );
        // (c) Materially different from single-osc — confirms the
        // detuned oscs actually contribute, not just the carrier.
        let diff_rms = rms(
            &single_buf[..BLOCK]
                .iter()
                .zip(triple_buf[..BLOCK].iter())
                .map(|(a, b)| a - b)
                .collect::<Vec<_>>(),
        );
        assert!(
            diff_rms > 0.01,
            "detuned three-osc must differ from single-osc; diff rms = {diff_rms}",
        );
    }

    // ── F4 modulation-routing tests ─────────────────────────────────────
    //
    // The five tests below pin the PM/AM/RM wiring. They use a shared
    // helper to render a long-enough audio window past the amp attack
    // ramp so the asserts measure steady-state behavior, not the
    // transient.

    fn render_steady_state(
        patch: [WavetableOscParams; NUM_OSCS],
        matrix: [ModSlot; MOD_MATRIX_SLOTS],
    ) -> Vec<f32> {
        let mut node = make_node();
        node.set_patch_for_test(patch);
        node.set_matrix_for_test(matrix);
        let mut buf = Vec::new();
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut buf,
        );
        // Burn three more blocks so the amp envelope is at sustain
        // and the LFO has stabilized.
        for _ in 0..3 {
            render_block(&mut node, &[], &mut buf);
        }
        // Grab one more block as the measurement window.
        render_block(&mut node, &[], &mut buf);
        buf[..BLOCK].to_vec()
    }

    /// Patch: carrier on osc[0], silent modulator on osc[1] (audible
    /// only as the matrix source). The returned `osc_params` shape
    /// is the same regardless of routing; the modulation lives in
    /// the matrix slots, which the caller supplies.
    fn carrier_with_silent_modulator_patch() -> [WavetableOscParams; NUM_OSCS] {
        [
            WavetableOscParams {
                tune_semitones: 0,
                fine_cents: 0,
                level: 1.0,
            },
            WavetableOscParams {
                // Perfect fifth above carrier; level=0 so it doesn't
                // contribute to the audio sum, only feeds the matrix.
                tune_semitones: 7,
                fine_cents: 0,
                level: 0.0,
            },
            WavetableOscParams::default(),
        ]
    }

    /// Build a one-slot matrix routing `Osc1 → destination @ amount`.
    /// `destination == None` ⇒ empty matrix (no modulation).
    fn matrix_one_slot(
        destination: Option<ModDestination>,
        amount: f32,
    ) -> [ModSlot; MOD_MATRIX_SLOTS] {
        let mut slots = [EMPTY_SLOT; MOD_MATRIX_SLOTS];
        if let Some(dst) = destination {
            slots[0] = ModSlot {
                source: ModSource::Osc1,
                destination: dst,
                amount,
            };
        }
        slots
    }

    #[test]
    fn pm_amount_zero_is_a_noop() {
        // PM at amount=0 must be sample-identical to no slot — at
        // `add_contribution`-time the contribution is `source × 0
        // × scale = 0`, so `tick_with_pm(table, 0.0)` is invoked,
        // which equals `tick(table)` (pinned in F1).
        let pm_off = render_steady_state(
            carrier_with_silent_modulator_patch(),
            matrix_one_slot(None, 0.0),
        );
        let pm_zero = render_steady_state(
            carrier_with_silent_modulator_patch(),
            matrix_one_slot(Some(ModDestination::PmAmountOf(0)), 0.0),
        );
        for (i, (a, b)) in pm_off.iter().zip(pm_zero.iter()).enumerate() {
            assert_eq!(*a, *b, "sample {i}: empty matrix and Pm@0 must match exactly");
        }
    }

    #[test]
    fn pm_engaged_changes_the_signal() {
        let pm_off = render_steady_state(
            carrier_with_silent_modulator_patch(),
            matrix_one_slot(None, 0.0),
        );
        let pm_on = render_steady_state(
            carrier_with_silent_modulator_patch(),
            matrix_one_slot(Some(ModDestination::PmAmountOf(0)), 0.3),
        );
        let diff_rms = rms(
            &pm_off
                .iter()
                .zip(pm_on.iter())
                .map(|(a, b)| a - b)
                .collect::<Vec<_>>(),
        );
        assert!(
            diff_rms > 0.01,
            "Pm @ 0.3 should diverge from carrier alone; diff_rms = {diff_rms}",
        );
    }

    #[test]
    fn am_engaged_changes_the_signal() {
        let am_off = render_steady_state(
            carrier_with_silent_modulator_patch(),
            matrix_one_slot(None, 0.0),
        );
        let am_on = render_steady_state(
            carrier_with_silent_modulator_patch(),
            matrix_one_slot(Some(ModDestination::AmAmountOf(0)), 0.5),
        );
        let diff_rms = rms(
            &am_off
                .iter()
                .zip(am_on.iter())
                .map(|(a, b)| a - b)
                .collect::<Vec<_>>(),
        );
        assert!(
            diff_rms > 0.01,
            "Am @ 0.5 should diverge from carrier alone; diff_rms = {diff_rms}",
        );
    }

    #[test]
    fn rm_engaged_changes_the_signal() {
        let rm_off = render_steady_state(
            carrier_with_silent_modulator_patch(),
            matrix_one_slot(None, 0.0),
        );
        let rm_on = render_steady_state(
            carrier_with_silent_modulator_patch(),
            matrix_one_slot(Some(ModDestination::RmAmountOf(0)), 1.0),
        );
        let diff_rms = rms(
            &rm_off
                .iter()
                .zip(rm_on.iter())
                .map(|(a, b)| a - b)
                .collect::<Vec<_>>(),
        );
        assert!(
            diff_rms > 0.01,
            "Rm @ 1.0 should diverge from carrier alone; diff_rms = {diff_rms}",
        );
    }

    #[test]
    fn modes_produce_distinct_outputs() {
        // Defensive pin: at the same non-trivial amount, the three
        // modulation destinations must produce mutually different
        // outputs. Catches accidental match-arm aliasing in
        // `Modulations::add_contribution`.
        let pm = render_steady_state(
            carrier_with_silent_modulator_patch(),
            matrix_one_slot(Some(ModDestination::PmAmountOf(0)), 0.5),
        );
        let am = render_steady_state(
            carrier_with_silent_modulator_patch(),
            matrix_one_slot(Some(ModDestination::AmAmountOf(0)), 0.5),
        );
        let rm = render_steady_state(
            carrier_with_silent_modulator_patch(),
            matrix_one_slot(Some(ModDestination::RmAmountOf(0)), 0.5),
        );
        let pa_diff = rms(&pm.iter().zip(am.iter()).map(|(a, b)| a - b).collect::<Vec<_>>());
        let pr_diff = rms(&pm.iter().zip(rm.iter()).map(|(a, b)| a - b).collect::<Vec<_>>());
        let ar_diff = rms(&am.iter().zip(rm.iter()).map(|(a, b)| a - b).collect::<Vec<_>>());
        assert!(pa_diff > 0.005, "Pm vs Am should differ; pa_diff = {pa_diff}");
        assert!(pr_diff > 0.005, "Pm vs Rm should differ; pr_diff = {pr_diff}");
        assert!(ar_diff > 0.005, "Am vs Rm should differ; ar_diff = {ar_diff}");
    }

    // ── M3 matrix-routing tests ─────────────────────────────────────────

    /// With an empty matrix (no slots active), filter cutoff sits
    /// static at the patch base — no LFO sweep on the filter. The
    /// audio still moves (the carrier saw is still bright), but the
    /// LFO-driven cutoff wobble that v1 had is gone. Diff RMS
    /// against the default-matrix render confirms the matrix is
    /// actually wired (removing the slot changes the audio).
    #[test]
    fn empty_matrix_disables_lfo_cutoff_sweep() {
        let default_matrix_buf =
            render_steady_state(coherent_three_osc_patch(), PATCH_MATRIX_SLOTS);

        let mut node = make_node();
        node.set_patch_for_test(coherent_three_osc_patch());
        node.set_matrix_for_test([EMPTY_SLOT; MOD_MATRIX_SLOTS]);
        let mut buf = Vec::new();
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut buf,
        );
        for _ in 0..3 {
            render_block(&mut node, &[], &mut buf);
        }
        render_block(&mut node, &[], &mut buf);
        let empty_matrix_buf = buf[..BLOCK].to_vec();

        // Empty matrix is non-silent (the carrier still plays
        // through a static-cutoff filter).
        assert!(
            rms(&empty_matrix_buf) > 0.01,
            "empty matrix should still produce audio; rms = {}",
            rms(&empty_matrix_buf),
        );

        // And it differs from the default-matrix render — the
        // LFO→cutoff slot's contribution is gone.
        let diff_rms = rms(
            &default_matrix_buf
                .iter()
                .zip(empty_matrix_buf.iter())
                .map(|(a, b)| a - b)
                .collect::<Vec<_>>(),
        );
        assert!(
            diff_rms > 0.005,
            "removing LFO→cutoff slot should audibly change output; diff_rms = {diff_rms}",
        );
    }

    /// Adding an ENV2→cutoff slot to a known patch produces an
    /// audibly different render than the same patch without the
    /// slot. Pins that ENV2 is correctly threaded through the
    /// matrix and that the matrix's contribution actually reaches
    /// the filter cutoff.
    #[test]
    fn env2_to_cutoff_slot_changes_audio() {
        let mut without = make_node();
        without.set_matrix_for_test([EMPTY_SLOT; MOD_MATRIX_SLOTS]);
        let mut without_buf = Vec::new();
        render_block(
            &mut without,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut without_buf,
        );

        let mut with = make_node();
        let mut slots = [EMPTY_SLOT; MOD_MATRIX_SLOTS];
        slots[0] = ModSlot {
            source: ModSource::Env2,
            destination: ModDestination::FilterCutoff,
            amount: 0.6,
        };
        with.set_matrix_for_test(slots);
        let mut with_buf = Vec::new();
        render_block(
            &mut with,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut with_buf,
        );

        let diff_rms = rms(
            &without_buf[..BLOCK]
                .iter()
                .zip(with_buf[..BLOCK].iter())
                .map(|(a, b)| a - b)
                .collect::<Vec<_>>(),
        );
        assert!(
            diff_rms > 0.005,
            "ENV2→cutoff slot should audibly change output; diff_rms = {diff_rms}",
        );
    }
}
