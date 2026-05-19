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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub use param::{patch_to_param_events as drum_patch_to_param_events, DrumParam};
pub use patch::{DrumPatch, HatPatch, KickPatch, SnarePatch};
pub use voices::{HatVoice, KickVoice, SnareVoice};

/// Audio-thread → host publishers for one [`DrumSynthNode`].
///
/// Parallel to
/// [`WavetablePublishers`](rawdaw_synth_wavetable::WavetablePublishers).
/// The audio thread bumps `version` + writes `snapshot` on every
/// successful `ParamEvent` apply; the host's `DrumPoller` reads both
/// to mirror the live patch into a UI signal.
///
/// `Clone` is cheap — both inner fields are `Arc`. The host keeps a
/// clone after passing one into the synth node so it can subscribe
/// to patch changes after the node has moved into the engine.
///
/// RT trade-off: the audio thread takes a brief `lock()` to write
/// the snapshot on each Param event (Mutex<DrumPatch> contains a
/// `Copy` payload of a few hundred bytes — the critical section is
/// a memcpy). Triple-buffer / `arc-swap` cleanup is a known future
/// optimization; measurement should justify it before we add a
/// dependency.
#[derive(Clone)]
pub struct DrumPublishers {
    pub version: Arc<AtomicU64>,
    pub snapshot: Arc<Mutex<DrumPatch>>,
}

impl DrumPublishers {
    /// Build a fresh publishers pair seeded with `initial`. Version
    /// starts at 0; the snapshot starts at the initial patch so the
    /// host can read the boot state without a `prepare()` callback
    /// having fired.
    pub fn new(initial: DrumPatch) -> Self {
        Self {
            version: Arc::new(AtomicU64::new(0)),
            snapshot: Arc::new(Mutex::new(initial)),
        }
    }
}

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
/// noise source, the canonical [`DrumPatch`] that every voice copies
/// from at `prepare()` and at per-note patch install (hats), plus
/// patch publishers (`patch_version`, `patch_snapshot`) for host-side
/// UI subscription. See [`DrumPublishers`].
pub struct DrumSynthNode {
    kicks: VoicePool<KickVoice>,
    snares: VoicePool<SnareVoice>,
    hats: VoicePool<HatVoice>,
    noise: NoiseSource,
    patch: DrumPatch,
    /// Bumped on every successful `ParamEvent` apply. Pollers watch
    /// this to know when the snapshot is fresh.
    patch_version: Arc<AtomicU64>,
    /// Latest post-apply patch — mirrors `self.patch` after every
    /// `ParamEvent`. Host reads via `lock()`.
    patch_snapshot: Arc<Mutex<DrumPatch>>,
}

impl DrumSynthNode {
    /// Build with the v0 default drum patch. Publishers are created
    /// internally and dropped when this node drops; tests and
    /// historical callers that don't need them stay terse.
    pub fn new() -> Self {
        Self::with_patch(DrumPatch::default())
    }

    /// Build with a specific runtime patch. Publishers are created
    /// internally — callers that need to observe the patch from the
    /// host side use [`Self::with_patch_publishers`] instead.
    pub fn with_patch(patch: DrumPatch) -> Self {
        let pubs = DrumPublishers::new(patch);
        Self::with_patch_publishers(patch, pubs)
    }

    /// Build with patch + host-owned publishers. The host keeps a
    /// clone of `publishers` so it can subscribe to patch changes
    /// after the node moves into the engine. The audio thread writes
    /// to `publishers.snapshot` + bumps `publishers.version` on every
    /// successful `ParamEvent` apply.
    pub fn with_patch_publishers(patch: DrumPatch, publishers: DrumPublishers) -> Self {
        Self {
            kicks: VoicePool::new(POLYPHONY_PER_TYPE, KickVoice::new),
            snares: VoicePool::new(POLYPHONY_PER_TYPE, SnareVoice::new),
            hats: VoicePool::new(POLYPHONY_PER_TYPE, HatVoice::new),
            noise: NoiseSource::new(0xC0FFEE),
            patch,
            patch_version: publishers.version,
            patch_snapshot: publishers.snapshot,
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
            // Drums are unpitched one-shots — pitch wheel has no
            // meaningful effect, and sustain pedal would just hold
            // already-completed envelopes. Silently ignore both at
            // K4. (K5 may route CCs to drum-side mod-matrix-equivalents
            // when those exist.)
            BlockMessage::Midi(Midi2Message::ControlChange { .. })
            | BlockMessage::Midi(Midi2Message::PitchBend { .. }) => {}
            BlockMessage::Param(ParamEvent { path, value }) => {
                self.apply_param(path, *value);
            }
        }
    }

    /// Decode a parameter event and apply it to the runtime patch.
    /// Unrecognized paths `debug_assert!` in debug + no-op in
    /// release. After mutation:
    ///
    /// 1. Propagate the relevant sub-patch into the matching voice
    ///    pool so subsequent samples see the change.
    /// 2. Write the new patch into `patch_snapshot` and bump
    ///    `patch_version` so host pollers can re-snapshot.
    fn apply_param(&mut self, path: &[u8; 8], value: f32) {
        let Some(param) = DrumParam::decode(path) else {
            debug_assert!(false, "DrumSynthNode: unknown ParamEvent path {path:?}");
            return;
        };
        param.apply(&mut self.patch, value);
        self.propagate_patch_to_voices();
        self.publish_patch();
    }

    /// Write the current `self.patch` into `patch_snapshot` and bump
    /// `patch_version` so host pollers see a fresh version. Brief
    /// `lock()` — `DrumPatch` is `Copy`, so the critical section is
    /// a memcpy. Version bump is `Release` so a host reader doing an
    /// `Acquire` load is guaranteed to see the snapshot write.
    fn publish_patch(&self) {
        if let Ok(mut slot) = self.patch_snapshot.lock() {
            *slot = self.patch;
        }
        self.patch_version.fetch_add(1, Ordering::Release);
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

    // ── U7 publishers contract ───────────────────────────────────

    fn make_node_with_publishers(patch: DrumPatch) -> (DrumSynthNode, DrumPublishers) {
        let pubs = DrumPublishers::new(patch);
        let mut node = DrumSynthNode::with_patch_publishers(patch, pubs.clone());
        node.prepare(SR, BLOCK);
        (node, pubs)
    }

    fn param_event(param: DrumParam, value: f32) -> BlockEventInBlock {
        BlockEventInBlock {
            offset_in_block: 0,
            message: BlockMessage::Param(rawdaw_engine::ParamEvent {
                path: param.encode(),
                value,
            }),
        }
    }

    #[test]
    fn drum_publishers_seed_to_initial_patch_and_zero_version() {
        let patch = DrumPatch::default();
        let (_node, pubs) = make_node_with_publishers(patch);
        assert_eq!(pubs.version.load(Ordering::Acquire), 0);
        let snapshot = pubs.snapshot.lock().expect("snapshot lock");
        assert_eq!(snapshot.kick.start_hz, patch.kick.start_hz);
        assert_eq!(snapshot.snare.noise_mix, patch.snare.noise_mix);
    }

    #[test]
    fn drum_param_apply_bumps_version_and_updates_snapshot() {
        let patch = DrumPatch::default();
        let (mut node, pubs) = make_node_with_publishers(patch);

        let events = [param_event(DrumParam::KickStartHz, 200.0)];
        let mut out = Vec::new();
        render_block(&mut node, &events, &mut out);

        assert!(pubs.version.load(Ordering::Acquire) >= 1);
        let snapshot = pubs.snapshot.lock().expect("snapshot lock");
        assert_eq!(snapshot.kick.start_hz, 200.0);
    }

    #[test]
    fn drum_read_from_is_inverse_of_apply() {
        // U9 audio→UI bind pin: DrumParam::read_from(patch) is the
        // inverse of DrumParam::apply(patch, value). Spot-check
        // one variant per voice/category.
        let test_cases: Vec<(DrumParam, f32)> = vec![
            (DrumParam::KickStartHz, 130.0),
            (DrumParam::KickAmpDecayS, 0.42),
            (DrumParam::SnareNoiseMix, 0.25),
            (DrumParam::SnareAmpAttackS, 0.003),
            (DrumParam::ClosedHatHpHz, 7500.0),
            (DrumParam::ClosedHatAmpDecayS, 0.07),
            (DrumParam::OpenHatHpHz, 4500.0),
            (DrumParam::OpenHatAmpRelease, 0.05),
        ];
        for (param, value) in test_cases {
            let mut patch = DrumPatch::default();
            param.apply(&mut patch, value);
            let read = param.read_from(&patch);
            assert!(
                (read - value).abs() < 0.001,
                "{param:?} round trip failed: applied {value}, read {read}",
            );
        }
    }

    #[test]
    fn drum_patch_to_param_events_round_trips_through_apply() {
        // Mirror of the wavetable round-trip pin: every (param,
        // value) emitted by drum_patch_to_param_events applied to a
        // default patch must reproduce the source patch.
        use crate::drum_patch_to_param_events;

        let mut source = DrumPatch::default();
        source.kick.start_hz = 150.0;
        source.snare.noise_mix = 0.25;
        source.closed_hat.hp_hz = 8500.0;
        source.open_hat.amp.decay_s = 0.8;

        let events = drum_patch_to_param_events(&source);
        assert_eq!(events.len(), 29, "every drum-param variant must appear");

        let mut target = DrumPatch::default();
        for (param, value) in events {
            param.apply(&mut target, value);
        }

        assert_eq!(target.kick.start_hz, 150.0);
        assert_eq!(target.snare.noise_mix, 0.25);
        assert_eq!(target.closed_hat.hp_hz, 8500.0);
        assert_eq!(target.open_hat.amp.decay_s, 0.8);
    }

    #[test]
    fn drum_multiple_param_events_in_one_block_bump_version_multiply() {
        let patch = DrumPatch::default();
        let (mut node, pubs) = make_node_with_publishers(patch);

        let events = [
            param_event(DrumParam::KickStartHz, 150.0),
            param_event(DrumParam::SnareNoiseMix, 0.3),
            param_event(DrumParam::ClosedHatHpHz, 5000.0),
        ];
        let mut out = Vec::new();
        render_block(&mut node, &events, &mut out);

        assert_eq!(pubs.version.load(Ordering::Acquire), 3);
        let snapshot = pubs.snapshot.lock().expect("snapshot lock");
        assert_eq!(snapshot.kick.start_hz, 150.0);
        assert_eq!(snapshot.snare.noise_mix, 0.3);
        assert_eq!(snapshot.closed_hat.hp_hz, 5000.0);
    }
}
