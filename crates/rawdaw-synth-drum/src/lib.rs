//! rawdaw-synth-drum — v0 basic drum synth.
//!
//! Dispatches MIDI NoteOn events to per-drum-type voice pools.
//! General-MIDI percussion mapping (subset):
//!
//! | MIDI | Voice            |
//! |------|------------------|
//! | 36   | [`KickVoice`]    |
//! | 38   | [`SnareVoice`]   |
//! | 42   | [`HatVoice`] (closed) |
//! | 46   | [`HatVoice`] (open)   |
//!
//! Unknown MIDI notes are dropped silently for now; the round-3 UI
//! will surface them as "voice not in kit" diagnostics.
//!
//! Each voice type has its own pool because the synthesis state
//! per drum is type-specific — a kick voice has a pitch envelope,
//! a hat voice has a filter, etc. Sharing one pool across types
//! would mean every voice carrying every drum's state.
//!
//! See `docs/wavetable-synth-plan.md` for the architecture lineage
//! (the wavetable synth was the v0 reference; the drum synth follows
//! the same VoicePool + AudioNode shape).

#![forbid(unsafe_code)]

mod param;
mod patch;
mod voices;

use rawdaw_dsp::{NoiseSource, VoicePool};
use rawdaw_engine::buffer::ChannelCount;
use rawdaw_engine::context::ProcessContext;
use rawdaw_engine::event::{BlockMessage, EventBlock, ParamEvent};
use rawdaw_engine::node::{AudioNode, OutputDescriptor, PortAccess};
use rawdaw_model::{Midi2Message, U16Velocity};

pub use param::DrumParam;
pub use patch::{DrumPatch, HatPatch, KickPatch, SnarePatch};
pub use voices::{HatVoice, KickVoice, SnareVoice};

/// Polyphony per drum type. Drum hits rarely overlap deeply, but a
/// few simultaneous hits are common (e.g. open-hat + crash + ride
/// on a single beat). 4 voices per type is generous for v0.
const POLYPHONY_PER_TYPE: usize = 4;

/// MIDI note classification for the GM drum subset we support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrumKind {
    Kick,
    Snare,
    Hat(HatRole),
}

/// Closed vs open hat — selects which sub-patch to install on the
/// hat voice before triggering. Replaces the previous public
/// `HatStyle` enum from `voices::hat`; now an internal classifier
/// detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HatRole {
    Closed,
    Open,
}

fn classify(midi_note: u8) -> Option<DrumKind> {
    match midi_note {
        36 => Some(DrumKind::Kick),
        38 => Some(DrumKind::Snare),
        42 => Some(DrumKind::Hat(HatRole::Closed)),
        46 => Some(DrumKind::Hat(HatRole::Open)),
        _ => None,
    }
}

/// The drum synth node. Owns per-drum-type voice pools, a shared
/// noise source, and the canonical [`DrumPatch`] that every voice
/// copies from at `prepare()` and at per-note patch install (hats).
pub struct DrumSynthNode {
    kicks: VoicePool<KickVoice>,
    snares: VoicePool<SnareVoice>,
    hats: VoicePool<HatVoice>,
    noise: NoiseSource,
    patch: DrumPatch,
}

impl DrumSynthNode {
    /// Build with the v0 default drum patch. Delegates to
    /// `with_patch(DrumPatch::default())`.
    pub fn new() -> Self {
        Self::with_patch(DrumPatch::default())
    }

    /// Build with a specific runtime patch. Round-1's audio graph
    /// constructs through this path so each `Track::synth`'s patch
    /// flows into the voices.
    pub fn with_patch(patch: DrumPatch) -> Self {
        Self {
            kicks: VoicePool::new(POLYPHONY_PER_TYPE, KickVoice::new),
            snares: VoicePool::new(POLYPHONY_PER_TYPE, SnareVoice::new),
            hats: VoicePool::new(POLYPHONY_PER_TYPE, HatVoice::new),
            noise: NoiseSource::new(0xC0FFEE),
            patch,
        }
    }

    fn apply_event(&mut self, message: &BlockMessage) {
        match message {
            BlockMessage::Midi(Midi2Message::NoteOn { note, velocity, .. }) => {
                let Some(kind) = classify(note.get()) else {
                    return;
                };
                let amp = u16_velocity_to_amplitude(*velocity);
                match kind {
                    DrumKind::Kick => self.kicks.note_on(note.get(), amp),
                    DrumKind::Snare => self.snares.note_on(note.get(), amp),
                    DrumKind::Hat(role) => {
                        // Install the right sub-patch on the slot
                        // we're about to claim. The pool allocates,
                        // then we find the just-claimed voice and
                        // update its patch — same post-allocation
                        // shape as the previous `set_style` flow.
                        let hat_patch = match role {
                            HatRole::Closed => &self.patch.closed_hat,
                            HatRole::Open => &self.patch.open_hat,
                        };
                        self.hats.note_on(note.get(), amp);
                        for v in self.hats.voices_mut() {
                            if v.is_active_recent(note.get()) {
                                v.set_patch(hat_patch);
                                break;
                            }
                        }
                    }
                }
            }
            BlockMessage::Midi(Midi2Message::NoteOff { note, .. }) => {
                // Drums are one-shot — they ignore NoteOff. We still
                // dispatch so the Voice trait contract is respected.
                let Some(kind) = classify(note.get()) else {
                    return;
                };
                match kind {
                    DrumKind::Kick => self.kicks.note_off(note.get()),
                    DrumKind::Snare => self.snares.note_off(note.get()),
                    DrumKind::Hat(_) => self.hats.note_off(note.get()),
                }
            }
            BlockMessage::Param(ParamEvent { path, value }) => {
                self.apply_param(path, *value);
            }
        }
    }

    /// Decode a parameter event and apply it to the runtime patch.
    /// Unrecognized paths `debug_assert!` in debug + no-op in release.
    /// After mutation, propagates the relevant sub-patch into the
    /// matching voice pool so subsequent samples see the change.
    fn apply_param(&mut self, path: &[u8; 8], value: f32) {
        let Some(param) = DrumParam::decode(path) else {
            debug_assert!(false, "DrumSynthNode: unknown ParamEvent path {path:?}");
            return;
        };
        param.apply(&mut self.patch, value);
        self.propagate_patch_to_voices();
    }

    /// Push every voice pool's current sub-patch from the canonical
    /// drum patch. Called by `apply_param` after each mutation and
    /// by `prepare()`. Hats default to the closed sub-patch here;
    /// the per-note dispatcher in `apply_event` overrides to the
    /// open sub-patch on MIDI 46.
    fn propagate_patch_to_voices(&mut self) {
        for v in self.kicks.voices_mut() {
            v.set_patch(&self.patch.kick);
        }
        for v in self.snares.voices_mut() {
            v.set_patch(&self.patch.snare);
        }
        for v in self.hats.voices_mut() {
            v.set_patch(&self.patch.closed_hat);
        }
    }
}

impl Default for DrumSynthNode {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioNode for DrumSynthNode {
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
            while next_event < event_slice.len()
                && (event_slice[next_event].offset_in_block as usize) <= i
            {
                let msg = &event_slice[next_event].message;
                self.apply_event(msg);
                next_event += 1;
            }

            let noise = self.noise.tick();
            let mut sample = 0.0_f32;
            for v in self.kicks.voices_mut() {
                if v.is_active() {
                    sample += v.tick();
                }
            }
            for v in self.snares.voices_mut() {
                if v.is_active() {
                    sample += v.tick(noise);
                }
            }
            for v in self.hats.voices_mut() {
                if v.is_active() {
                    sample += v.tick(noise);
                }
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
        for v in self.kicks.voices_mut() {
            v.prepare(sample_rate, &self.patch.kick);
        }
        for v in self.snares.voices_mut() {
            v.prepare(sample_rate, &self.patch.snare);
        }
        // Hats default to the closed shape at prepare time; per-note
        // dispatch overrides to the open shape on MIDI 46.
        for v in self.hats.voices_mut() {
            v.prepare(sample_rate, &self.patch.closed_hat);
        }
    }
}

fn u16_velocity_to_amplitude(v: U16Velocity) -> f32 {
    // Match the wavetable synth's per-voice headroom — drums need it
    // too once a kick + snare + hat all hit on the same sample.
    (v.get() as f32 / u16::MAX as f32) * 0.5
}

// Helper so the dispatcher can find which hat slot just received the
// note-on (used by the style-setting step). Lives as an inherent
// method on HatVoice rather than the public Voice trait.
trait IsActiveRecent {
    fn is_active_recent(&self, note: u8) -> bool;
}

use rawdaw_dsp::Voice as _;

impl IsActiveRecent for HatVoice {
    fn is_active_recent(&self, note: u8) -> bool {
        self.is_active() && self.note() == note
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_engine::event::BlockEventInBlock;
    use rawdaw_model::{MidiChannel, MidiNote, MusicalTime};

    const SR: u32 = 48_000;
    const BLOCK: usize = 256;

    fn make_node() -> DrumSynthNode {
        let mut node = DrumSynthNode::new();
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
        node: &mut DrumSynthNode,
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
    fn kick_produces_audible_output() {
        let mut node = make_node();
        let mut buf = Vec::new();
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(36, U16Velocity::HALF),
            }],
            &mut buf,
        );
        assert!(
            rms(&buf[..BLOCK]) > 0.01,
            "kick should produce audible signal; got rms {}",
            rms(&buf[..BLOCK]),
        );
    }

    #[test]
    fn snare_produces_audible_output() {
        let mut node = make_node();
        let mut buf = Vec::new();
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(38, U16Velocity::HALF),
            }],
            &mut buf,
        );
        assert!(
            rms(&buf[..BLOCK]) > 0.01,
            "snare should produce audible signal; got rms {}",
            rms(&buf[..BLOCK]),
        );
    }

    #[test]
    fn closed_hat_produces_audible_output() {
        let mut node = make_node();
        let mut buf = Vec::new();
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(42, U16Velocity::HALF),
            }],
            &mut buf,
        );
        assert!(
            rms(&buf[..BLOCK]) > 0.005,
            "closed hat should produce audible signal; got rms {}",
            rms(&buf[..BLOCK]),
        );
    }

    #[test]
    fn unknown_midi_note_is_silent() {
        let mut node = make_node();
        let mut buf = Vec::new();
        // MIDI 60 (middle C) is not in our drum map.
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(60, U16Velocity::HALF),
            }],
            &mut buf,
        );
        assert!(
            buf.iter().all(|s| *s == 0.0),
            "unknown drum note should be silent",
        );
    }

    #[test]
    fn kick_decays_to_silence() {
        let mut node = make_node();
        let mut buf = Vec::new();
        render_block(
            &mut node,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(36, U16Velocity::HALF),
            }],
            &mut buf,
        );
        // Kick decay is ~250 ms; render ~1 second of trailing silence.
        for _ in 0..200 {
            render_block(&mut node, &[], &mut buf);
        }
        let tail_rms = rms(&buf[..BLOCK]);
        assert!(
            tail_rms < 1e-4,
            "kick should fully decay; tail rms = {tail_rms}",
        );
    }

    #[test]
    fn classifier_covers_v0_voices() {
        assert_eq!(classify(36), Some(DrumKind::Kick));
        assert_eq!(classify(38), Some(DrumKind::Snare));
        assert_eq!(classify(42), Some(DrumKind::Hat(HatRole::Closed)));
        assert_eq!(classify(46), Some(DrumKind::Hat(HatRole::Open)));
        assert_eq!(classify(60), None);
    }

    /// A `SnareNoiseMix` parameter event arriving at the node
    /// mutates the patch and propagates to voices — subsequent
    /// snare hits render with the new mix. Pins the full chain:
    /// BlockMessage::Param → apply_event → DrumParam::decode →
    /// apply → propagate.
    #[test]
    fn param_event_changes_snare_noise_mix() {
        // Baseline: snare with default mix (0.7).
        let mut control = make_node();
        let mut control_buf = Vec::new();
        render_block(
            &mut control,
            &[BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(38, U16Velocity::HALF),
            }],
            &mut control_buf,
        );

        // Treatment: same setup but the mix is pushed to 0 (pure
        // body) BEFORE the note triggers, so the snare-allocated
        // voice picks up the new mix at its first sample.
        let mut treatment = make_node();
        let path = DrumParam::SnareNoiseMix.encode();
        let value = 0.0_f32;
        let mut treatment_buf = Vec::new();
        render_block(
            &mut treatment,
            &[
                BlockEventInBlock {
                    offset_in_block: 0,
                    message: BlockMessage::Param(rawdaw_engine::ParamEvent { path, value }),
                },
                BlockEventInBlock {
                    offset_in_block: 1,
                    message: note_on(38, U16Velocity::HALF),
                },
            ],
            &mut treatment_buf,
        );

        let diff_rms = rms(
            &control_buf[..BLOCK]
                .iter()
                .zip(treatment_buf[..BLOCK].iter())
                .map(|(a, b)| a - b)
                .collect::<Vec<_>>(),
        );
        assert!(
            diff_rms > 0.005,
            "SnareNoiseMix @ 0 should audibly change the snare; diff_rms = {diff_rms}",
        );
    }
}
