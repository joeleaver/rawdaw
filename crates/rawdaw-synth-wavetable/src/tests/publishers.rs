//! Tests for [`WavetablePublishers`] — the audio thread → host
//! patch-snapshot mechanism introduced in U5. Every successful
//! `ParamEvent` apply must bump the version counter and refresh
//! the snapshot lock; the host-side
//! [`WavetablePoller`](crate doc reference) reads both to mirror
//! the live patch into a UI signal.
//!
//! The poller itself lives in `rawdaw-app` (it needs the rinch
//! runtime); these tests verify the publisher contract that
//! makes the poll meaningful.
//!
//! Tests in this file send Param events directly through the
//! synth node's `process` loop (mirroring how the engine drives
//! `apply_event` at the right offset) so the assertions exercise
//! the same code path the audio thread uses.

use std::sync::atomic::Ordering;

use rawdaw_engine::event::{BlockEventInBlock, ParamEvent};

use crate::{BlockMessage, WavetableParam, WavetablePatch, WavetablePublishers, WavetableSynthNode};

use super::{ctx, render_block};

/// Helper: build a synth node with a fresh patch + paired publishers,
/// returning both so the test can assert against the publishers.
fn make_node_with_publishers(patch: WavetablePatch) -> (WavetableSynthNode, WavetablePublishers) {
    let pubs = WavetablePublishers::new(patch);
    let mut node = WavetableSynthNode::with_patch_publishers(patch, pubs.clone());
    // `prepare` is the engine-side init; mirror that here so the
    // node is in the same state it'd be inside the engine.
    use rawdaw_engine::node::AudioNode;
    node.prepare(super::SR, super::BLOCK);
    (node, pubs)
}

/// Send a Param event at sample offset 0 in a render block. The
/// audio thread will drain it before the first sample tick.
fn param_event(param: WavetableParam, value: f32) -> BlockEventInBlock {
    BlockEventInBlock {
        offset_in_block: 0,
        message: BlockMessage::Param(ParamEvent {
            path: param.encode(),
            value,
        }),
    }
}

#[test]
fn version_starts_at_zero_and_snapshot_seeds_to_initial_patch() {
    let patch = WavetablePatch::default();
    let (_node, pubs) = make_node_with_publishers(patch);
    assert_eq!(pubs.version.load(Ordering::Acquire), 0);
    let snapshot = pubs.snapshot.lock().expect("snapshot lock");
    assert_eq!(snapshot.filter_cutoff_hz, patch.filter_cutoff_hz);
    assert_eq!(snapshot.lfo_rate_hz, patch.lfo_rate_hz);
}

#[test]
fn param_apply_bumps_version_and_updates_snapshot() {
    let patch = WavetablePatch::default();
    let (mut node, pubs) = make_node_with_publishers(patch);

    let events = [param_event(WavetableParam::FilterCutoffHz, 4321.0)];
    let mut out = Vec::new();
    render_block(&mut node, &events, &mut out);

    let _ = ctx(); // pin the ctx helper as used
    assert!(
        pubs.version.load(Ordering::Acquire) >= 1,
        "version must bump on Param apply"
    );
    let snapshot = pubs.snapshot.lock().expect("snapshot lock");
    assert_eq!(
        snapshot.filter_cutoff_hz, 4321.0,
        "snapshot must reflect post-apply patch state"
    );
}

#[test]
fn multiple_param_events_in_one_block_bump_version_multiply() {
    // Three Param events in one block should produce three version
    // bumps — pollers shouldn't lose intermediate values, even if
    // they only ever observe the final snapshot.
    let patch = WavetablePatch::default();
    let (mut node, pubs) = make_node_with_publishers(patch);

    let events = [
        param_event(WavetableParam::FilterCutoffHz, 1000.0),
        param_event(WavetableParam::FilterResonance, 3.5),
        param_event(WavetableParam::LfoRateHz, 5.0),
    ];
    let mut out = Vec::new();
    render_block(&mut node, &events, &mut out);

    assert_eq!(
        pubs.version.load(Ordering::Acquire),
        3,
        "one bump per Param event"
    );
    let snapshot = pubs.snapshot.lock().expect("snapshot lock");
    assert_eq!(snapshot.filter_cutoff_hz, 1000.0);
    assert_eq!(snapshot.filter_resonance, 3.5);
    assert_eq!(snapshot.lfo_rate_hz, 5.0);
}

#[test]
fn read_from_is_inverse_of_apply_for_every_variant() {
    // U9 audio→UI bind pin: WavetableParam::read_from(patch) is
    // the inverse of WavetableParam::apply(patch, value). For
    // every variant, applying a value then reading it back must
    // yield the same value (within clamp). The matrix source +
    // destination ordinals round-trip via encode_mod_*.
    let test_cases: Vec<(WavetableParam, f32)> = vec![
        (WavetableParam::OscTune(0), 7.0),
        (WavetableParam::OscFineCents(1), -30.0),
        (WavetableParam::OscLevel(2), 0.42),
        (WavetableParam::EnvAttackS(0), 0.123),
        (WavetableParam::EnvDecayS(1), 0.456),
        (WavetableParam::EnvSustain(2), 0.789),
        (WavetableParam::EnvReleaseS(0), 1.234),
        (WavetableParam::LfoRateHz, 6.5),
        (WavetableParam::FilterCutoffHz, 4321.0),
        (WavetableParam::FilterResonance, 1.7),
        (WavetableParam::MatrixAmount(7), 0.33),
    ];
    for (param, value) in test_cases {
        let mut patch = WavetablePatch::default();
        param.apply(&mut patch, value);
        let read = param.read_from(&patch);
        assert!(
            (read - value).abs() < 0.001,
            "{param:?} round trip failed: applied {value}, read {read}",
        );
    }
}

#[test]
fn patch_to_param_events_round_trips_through_apply() {
    // U8 preset path pin: applying every (param, value) emitted by
    // patch_to_param_events to a default patch must reproduce the
    // source patch field-by-field. If a future patch field is
    // added without updating the flattener, this test catches it.
    use crate::wavetable_patch_to_param_events;

    let mut source = WavetablePatch::default();
    // Make the source distinguishable from the default so any
    // mismatch surfaces. The bell-like tweak below covers every
    // category of field (osc, env, filter, lfo, matrix).
    source.osc_params[1].tune_semitones = 5;
    source.osc_params[2].level = 0.42;
    source.env_params[0].decay_s = 1.5;
    source.lfo_rate_hz = 6.5;
    source.filter_cutoff_hz = 4321.0;
    source.filter_resonance = 1.7;
    source.matrix[7].amount = 0.33;

    let events = wavetable_patch_to_param_events(&source);
    assert_eq!(events.len(), 72, "every patch field must contribute");

    let mut target = WavetablePatch::default();
    for (param, value) in events {
        param.apply(&mut target, value);
    }

    // Float equality is fine here — the values were never run
    // through DSP, just round-tripped through the apply clamps.
    assert_eq!(target.osc_params[1].tune_semitones, 5);
    assert_eq!(target.osc_params[2].level, 0.42);
    assert_eq!(target.env_params[0].decay_s, 1.5);
    assert_eq!(target.lfo_rate_hz, 6.5);
    assert_eq!(target.filter_cutoff_hz, 4321.0);
    assert_eq!(target.filter_resonance, 1.7);
    assert_eq!(target.matrix[7].amount, 0.33);
}

#[test]
fn shared_publishers_reflect_node_state_for_external_holders() {
    // The host clones `WavetablePublishers` before handing it to
    // the synth — both clones share the same Arc<AtomicU64> and
    // Arc<Mutex<_>>. This pins the contract: a change on the audio
    // thread is visible through the host's clone.
    let patch = WavetablePatch::default();
    let pubs = WavetablePublishers::new(patch);
    let host_clone = pubs.clone();
    let mut node = WavetableSynthNode::with_patch_publishers(patch, pubs);
    use rawdaw_engine::node::AudioNode;
    node.prepare(super::SR, super::BLOCK);

    let events = [param_event(WavetableParam::OscLevel(1), 0.42)];
    let mut out = Vec::new();
    render_block(&mut node, &events, &mut out);

    assert!(host_clone.version.load(Ordering::Acquire) >= 1);
    let snapshot = host_clone.snapshot.lock().expect("snapshot lock");
    assert_eq!(snapshot.osc_params[1].level, 0.42);
}
