//! rawdaw-synth-wavetable — the Vital-class wavetable+FM synth.
//!
//! v0 wires the minimum-viable real synth: one wavetable oscillator,
//! an amp ADSR, a state-variable lowpass filter, and one LFO routed
//! to the filter cutoff. Stereo output mirrors L to R; spatialization
//! comes later. The growth target (multi-oscillator FM/AM, second
//! envelope, mod matrix, factory wavetable bank, unison) lives in
//! `docs/wavetable-synth-plan.md`.
//!
//! ## Hardcoded v0 patch
//!
//! - Wavetable: 128-harmonic band-limited saw (`Wavetable::saw_default`).
//! - Amp envelope: 5 ms / 80 ms / 0.7 / 200 ms ADSR.
//! - Filter: SVF lowpass, 1500 Hz cutoff, Q = 0.7.
//! - LFO: 4 Hz sine, ±400 Hz cutoff modulation.
//!
//! These are intentionally not exposed as parameters yet — when the
//! synth UI lands (round 3) the editor panel will read/write these
//! through a parameter struct. For now editing the patch means
//! changing the constants below and recompiling.

#![forbid(unsafe_code)]

use rawdaw_dsp::{Adsr, AdsrParams, SineLfo, SvfLowpass, Voice, VoicePool, Wavetable, WavetableOsc};
use rawdaw_engine::buffer::ChannelCount;
use rawdaw_engine::context::ProcessContext;
use rawdaw_engine::event::EventBlock;
use rawdaw_engine::node::{AudioNode, OutputDescriptor, PortAccess};
use rawdaw_model::{Midi2Message, U16Velocity};

/// Standard concert tuning. Aligned with `rawdaw-engine::nodes::sine`
/// so the wavetable synth and the placeholder sine play the same
/// frequencies for the same MIDI notes.
const A4_HZ: f32 = 440.0;
const A4_NOTE: f32 = 69.0;

/// Number of voices. Matches `SineNode` so the wavetable synth feels
/// identical at the queue layer when it replaces the sine in
/// rawdaw-app's track routing.
const NUM_VOICES: usize = 16;

// ── Hardcoded v0 patch ───────────────────────────────────────────────────
const PATCH_ATTACK_S: f32 = 0.005;
const PATCH_DECAY_S: f32 = 0.080;
const PATCH_SUSTAIN: f32 = 0.7;
const PATCH_RELEASE_S: f32 = 0.200;
const PATCH_FILTER_CUTOFF_HZ: f32 = 1500.0;
const PATCH_FILTER_RESONANCE: f32 = 0.7;
const PATCH_LFO_RATE_HZ: f32 = 4.0;
/// LFO depth in Hz. Filter cutoff is modulated by `PATCH_FILTER_CUTOFF_HZ +
/// lfo_value * PATCH_LFO_DEPTH_HZ` where `lfo_value` ∈ [-1, 1]. Cutoff
/// is clamped to a safe range by `SvfLowpass::set_cutoff` so a deep
/// modulation can't push the filter unstable.
const PATCH_LFO_DEPTH_HZ: f32 = 400.0;

/// One synth voice — phase-accumulator wavetable osc, amp envelope,
/// and its own filter (per-voice integrators so retrigger doesn't
/// blend tail ringing into the next note).
#[derive(Debug, Clone, Copy)]
struct WavetableVoice {
    note: u8,
    osc: WavetableOsc,
    amp: Adsr,
    filter: SvfLowpass,
    velocity_amp: f32,
}

impl WavetableVoice {
    fn new() -> Self {
        Self {
            note: 0,
            osc: WavetableOsc::new(),
            amp: Adsr::new(),
            filter: SvfLowpass::new(),
            velocity_amp: 0.0,
        }
    }

    /// Per-sample tick. `wavetable`, `lfo_value`, and the per-block
    /// `cutoff_center_hz` are shared inputs from the node.
    fn tick(&mut self, wavetable: &Wavetable, lfo_value: f32) -> f32 {
        // Modulate cutoff: center + LFO * depth. The filter clamps
        // the result, so the modulation can't push it unstable.
        let cutoff_hz = PATCH_FILTER_CUTOFF_HZ + lfo_value * PATCH_LFO_DEPTH_HZ;
        self.filter.set_cutoff(cutoff_hz);

        let raw = self.osc.tick(wavetable);
        let filtered = self.filter.tick(raw);
        let amp_level = self.amp.tick();
        filtered * amp_level * self.velocity_amp
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
        let hz = note_to_hz(note);
        self.osc.set_frequency(hz);
        self.osc.reset_phase();
        self.filter.reset_state();
        self.amp.note_on();
        self.velocity_amp = velocity;
    }

    fn note_off(&mut self) {
        self.amp.note_off();
    }
}

/// The Vital-class wavetable synth's `AudioNode`.
///
/// Stateful: voices and shared LFO live across `process` calls. The
/// wavetable is generated once at construction; future versions will
/// support a bank with patch-time selection.
pub struct WavetableSynthNode {
    wavetable: Wavetable,
    lfo: SineLfo,
    voices: VoicePool<WavetableVoice>,
    /// Cached MIDI-note → Hz so per-event work doesn't run a powf
    /// on the audio thread. Filled at `prepare` time.
    note_freq_table: Vec<f32>,
}

impl WavetableSynthNode {
    pub fn new() -> Self {
        Self {
            wavetable: Wavetable::saw_default(),
            lfo: SineLfo::new(),
            voices: VoicePool::new(NUM_VOICES, WavetableVoice::new),
            note_freq_table: Vec::new(),
        }
    }

    fn apply_event(&mut self, message: &Midi2Message) {
        match message {
            Midi2Message::NoteOn { note, velocity, .. } => {
                let amp = u16_velocity_to_amplitude(*velocity);
                self.voices.note_on(note.get(), amp);
            }
            Midi2Message::NoteOff { note, .. } => {
                self.voices.note_off(note.get());
            }
        }
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

        // Build the MIDI-note → Hz lookup once so per-event work is
        // a table read instead of a powf.
        self.note_freq_table = (0..=127).map(note_to_hz).collect();

        // Prepare every voice's per-voice state.
        let params = AdsrParams {
            attack_s: PATCH_ATTACK_S,
            decay_s: PATCH_DECAY_S,
            sustain_level: PATCH_SUSTAIN,
            release_s: PATCH_RELEASE_S,
        };
        for v in self.voices.voices_mut() {
            v.osc.prepare(sample_rate);
            v.amp.prepare(sample_rate);
            v.amp.set_params(params);
            v.filter.prepare(sample_rate);
            v.filter.set_cutoff(PATCH_FILTER_CUTOFF_HZ);
            v.filter.set_resonance(PATCH_FILTER_RESONANCE);
        }
    }
}

fn note_to_hz(note: u8) -> f32 {
    A4_HZ * 2.0_f32.powf((note as f32 - A4_NOTE) / 12.0)
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

    fn note_on(n: u8, vel: U16Velocity) -> Midi2Message {
        Midi2Message::NoteOn {
            channel: MidiChannel::default(),
            note: MidiNote::new(n).unwrap(),
            velocity: vel,
        }
    }

    fn note_off(n: u8) -> Midi2Message {
        Midi2Message::NoteOff {
            channel: MidiChannel::default(),
            note: MidiNote::new(n).unwrap(),
            velocity: U16Velocity::MIN,
        }
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
}
