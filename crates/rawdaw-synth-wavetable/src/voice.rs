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
    note_offset_hz, Adsr, ModMatrix, ModSource, Modulations, SvfLowpass, Voice, Wavetable,
    WavetableOsc, WavetableOscParams,
};

use crate::patch::WavetablePatch;
use crate::{MIDI_CC_COUNT, MOD_MATRIX_SLOTS, NUM_OSCS};

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
    /// Filter cutoff base in Hz. Read every sample as
    /// `filter_cutoff_hz_base + mods.filter_cutoff_hz_offset`. Copied
    /// from the node's [`WavetablePatch::filter_cutoff_hz`] at
    /// [`set_patch`](Self::set_patch) time so the audio thread never
    /// crosses the patch boundary mid-tick.
    filter_cutoff_hz_base: f32,
    velocity_amp: f32,
    /// K4 — current pitch-bend offset in semitones (signed). The
    /// synth node calls [`set_pitch_bend_semitones`] on every
    /// `PitchBend` event; the value is also read at `note_on` so
    /// fresh notes inherit the pedal-held bend.
    pitch_bend_semitones: f32,
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
            filter_cutoff_hz_base: 0.0,
            velocity_amp: 0.0,
            pitch_bend_semitones: 0.0,
        }
    }

    /// K4 — set the pitch-bend offset (in semitones) and re-derive
    /// each oscillator's frequency from its cached
    /// [`osc_hz_base`](Self::osc_hz_base) so the change applies
    /// instantly without waiting for the next note-on. Idle voices
    /// store the new bend but skip the frequency update (their
    /// `osc_hz_base` is stale from the prior note); the next
    /// `note_on` reads `pitch_bend_semitones` and applies the bend
    /// then.
    pub(crate) fn set_pitch_bend_semitones(&mut self, semitones: f32) {
        self.pitch_bend_semitones = semitones;
        if self.is_active() {
            let ratio = bend_ratio(semitones);
            for i in 0..NUM_OSCS {
                self.oscs[i].set_frequency(self.osc_hz_base[i] * ratio);
            }
        }
    }

    /// Configure sample rate + install the runtime patch. Called by
    /// the node's [`prepare`](crate::WavetableSynthNode::prepare) once
    /// per voice before the active graph runs. Folds in `set_patch`'s
    /// work so the per-voice state lands in one call.
    pub(crate) fn prepare(&mut self, sample_rate: u32, patch: &WavetablePatch) {
        for osc in &mut self.oscs {
            osc.prepare(sample_rate);
        }
        self.amp.prepare(sample_rate);
        self.amp.set_params(patch.env_params[0]);
        self.env2.prepare(sample_rate);
        self.env2.set_params(patch.env_params[1]);
        self.env3.prepare(sample_rate);
        self.env3.set_params(patch.env_params[2]);
        self.filter.prepare(sample_rate);
        self.filter.set_cutoff(patch.filter_cutoff_hz);
        self.filter.set_resonance(patch.filter_resonance);
        self.set_patch(patch);
    }

    /// Install a complete patch snapshot into this voice. Called on
    /// every `apply_param` so live tweaks (slider drags, MIDI Learn,
    /// preset application) reach held notes immediately — matches the
    /// behavior of every modern synth where any control reshapes the
    /// sounding voice in real time.
    ///
    /// Mirrors `prepare()` field-for-field, minus the sample-rate
    /// configuration (which never changes mid-session). Specifically:
    ///
    /// - Per-osc params + matrix + filter cutoff base — read each
    ///   tick from the per-voice copies, so a plain write suffices.
    /// - Envelope ADSR shapes — pushed into each ENV's `set_params`
    ///   so the *remaining* envelope stages honor the new shape
    ///   (e.g. lengthening release while a note is in sustain
    ///   produces a longer tail).
    /// - Filter resonance — pushed into the SVF. Cutoff is re-set
    ///   every tick from `filter_cutoff_hz_base + offsets`, but
    ///   resonance has no per-tick re-set, so it has to be poked
    ///   here.
    /// - Per-osc base frequency — re-derived from the (possibly
    ///   new) tune/fine cents on the currently-held note so tune
    ///   sliders re-pitch a sounding voice. Skipped for idle
    ///   voices (their `note` is stale; the next `note_on` will
    ///   re-derive correctly).
    pub(crate) fn set_patch(&mut self, patch: &WavetablePatch) {
        self.osc_params = patch.osc_params;
        self.matrix.set_slots(patch.matrix);
        self.filter_cutoff_hz_base = patch.filter_cutoff_hz;
        self.filter.set_resonance(patch.filter_resonance);
        self.amp.set_params(patch.env_params[0]);
        self.env2.set_params(patch.env_params[1]);
        self.env3.set_params(patch.env_params[2]);
        if self.is_active() {
            let ratio = bend_ratio(self.pitch_bend_semitones);
            for i in 0..NUM_OSCS {
                let hz = note_offset_hz(
                    self.note,
                    self.osc_params[i].tune_semitones,
                    self.osc_params[i].fine_cents,
                );
                self.osc_hz_base[i] = hz;
                self.oscs[i].set_frequency(hz * ratio);
            }
        }
    }

    /// Per-sample tick. Two-stage modulation evaluation:
    ///
    /// 1. **Control-rate pass.** Tick ENV1/2/3, then iterate the
    ///    matrix's control-rate slots (Env*/Lfo*/MidiCC sources)
    ///    into a fresh `Modulations` bag. The bag holds per-
    ///    destination contributions in destination-native units
    ///    (Hz, semitones, coefficients, …).
    /// 2. **Per-osc render in topo-sorted order.** For each osc in
    ///    `matrix.audio_rate_osc_order()`, fold in audio-rate slot
    ///    contributions (`OscN → ...Of(i)`) by reading the source's
    ///    sample from `osc_samples` (already rendered, since topo
    ///    sort ensures sources before destinations), then apply
    ///    pre-tick mods (PM offset, tune offset) and post-tick mods
    ///    (AM, RM coefficients) before storing the result.
    ///
    /// `cc_state` is the synth node's per-MIDI-CC normalized
    /// `[0.0, 1.0]` table. K5 routes `ModSource::MidiCC(cc)` reads
    /// directly into it; the per-event update in `apply_control_change`
    /// happens *before* this tick on the same sample (the synth
    /// node's process loop drains events at offset ≤ i for the i-th
    /// sample), so sample-accuracy within a block is preserved.
    pub(crate) fn tick(
        &mut self,
        wavetable: &Wavetable,
        lfo_value: f32,
        cc_state: &[f32; MIDI_CC_COUNT],
    ) -> f32 {
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
                ModSource::MidiCC(cc) => {
                    // `set_slots` already sanitized any cc > 127 to
                    // ModSource::None, so the index is in-range. A
                    // bounds-checked get() preserves safety against
                    // a future caller that bypasses set_slots.
                    match cc_state.get(cc as usize) {
                        Some(v) => *v,
                        None => continue,
                    }
                }
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
        let cutoff_hz = self.filter_cutoff_hz_base + mods.filter_cutoff_hz_offset;
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
        let bend_ratio = bend_ratio(self.pitch_bend_semitones);
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
            //
            // K4: oscillators play at base * pitch-bend ratio. The
            // base stays unbent so set_pitch_bend_semitones can
            // re-derive frequency on every wheel move without
            // re-running note_offset_hz.
            self.osc_hz_base[i] = hz;
            self.oscs[i].set_frequency(hz * bend_ratio);
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

/// K4 helper: convert pitch-bend semitones to a frequency-multiplier
/// ratio. Equal-temperament: each semitone is 2^(1/12) ≈ 1.0595.
/// Inlined-friendly + cheap on modern CPUs (one `powf`); called once
/// per note-on and once per `set_pitch_bend_semitones`.
fn bend_ratio(semitones: f32) -> f32 {
    if semitones == 0.0 {
        1.0
    } else {
        2.0_f32.powf(semitones / 12.0)
    }
}
