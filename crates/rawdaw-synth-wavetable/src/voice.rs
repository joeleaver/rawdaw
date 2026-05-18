//! Per-voice state and the per-sample render loop.
//!
//! One [`WavetableVoice`] owns three [`WavetableOsc`]s, three
//! [`Adsr`] envelopes, a per-voice copy of the node's mod matrix,
//! and an SVF lowpass. The per-sample [`tick`](WavetableVoice::tick)
//! evaluates control-rate matrix slots into a [`Modulations`] bag,
//! renders oscillators in topo-sorted order while folding audio-rate
//! contributions, then sums + filters + amp-envelopes the result.
//!
//! Voices are addressed through a [`VoicePool`] in the synth node;
//! `Voice for WavetableVoice` provides the pool's allocation /
//! note_on / note_off / is_active contract.

use rawdaw_dsp::{
    note_offset_hz, Adsr, ModMatrix, ModSlot, ModSource, Modulations, SvfLowpass, Voice,
    Wavetable, WavetableOsc, WavetableOscParams,
};

use crate::{MOD_MATRIX_SLOTS, NUM_OSCS, PATCH_FILTER_CUTOFF_HZ};

/// One synth voice — three wavetable oscillators, amp envelope, and
/// per-voice filter integrators so retrigger doesn't blend tail
/// ringing into the next note. Each voice carries its own copy of
/// the patch's per-osc params; the node propagates updates via
/// [`Self::set_patch`] so the audio thread can keep reading without
/// coordinating with the patch source.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WavetableVoice {
    note: u8,
    pub(crate) oscs: [WavetableOsc; NUM_OSCS],
    osc_params: [WavetableOscParams; NUM_OSCS],
    /// Per-osc Hz at the played note + static tune/fine, computed
    /// once at `note_on`. Audio-rate `OscTune` / `OscFineTune`
    /// modulation re-derives Hz from this base each sample (only
    /// when the tune offsets are nonzero — skipped otherwise).
    osc_hz_base: [f32; NUM_OSCS],
    /// ENV1 — amp envelope. Drives the voice's output amplitude and
    /// gates `is_active()`.
    pub(crate) amp: Adsr,
    /// ENV2 — free modulation envelope. Routed through `matrix`.
    pub(crate) env2: Adsr,
    /// ENV3 — second free modulation envelope. Routed through
    /// `matrix`.
    pub(crate) env3: Adsr,
    /// Per-voice modulation matrix (copy of the node's canonical
    /// matrix, refreshed at patch-apply time).
    matrix: ModMatrix<MOD_MATRIX_SLOTS>,
    pub(crate) filter: SvfLowpass,
    velocity_amp: f32,
}

impl WavetableVoice {
    pub(crate) fn new() -> Self {
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
    pub(crate) fn set_patch(
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
    pub(crate) fn tick(&mut self, wavetable: &Wavetable, lfo_value: f32) -> f32 {
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
