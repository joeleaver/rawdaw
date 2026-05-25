//! Unit tests for the audio module.
//!
//! Split out of `mod.rs` so the module stays under the workspace
//! 700-line cap. Every test calls
//! [`AudioResources::build_from_project_and_rate`] (the test-facing
//! entry that skips poller spawning) — the production
//! [`AudioResources::build`] requires the rinch runtime's
//! cross-thread dispatcher, which isn't available in `cargo test`.

use rawdaw_engine::{GraphCommand, NodeId, Transport};
use rawdaw_model::fixtures::build_round1_project;
use rawdaw_model::patch::SynthAssignment;
use rawdaw_model::realize::realize;

use super::{AudioResources, FALLBACK_SAMPLE_RATE};

#[test]
fn round_1_pushes_one_block_event_per_realized_event() {
    let (project, _) = build_round1_project();
    let realized = realize(&project, FALLBACK_SAMPLE_RATE);
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    assert_eq!(
        resources.initial_event_count,
        realized.len(),
        "every realized TimedEvent should translate 1:1 to a BlockEvent"
    );
    assert!(
        !realized.is_empty(),
        "the round-1 fixture must produce at least one realized event"
    );
}

#[test]
fn routing_covers_every_project_track() {
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    assert_eq!(resources.routing.len(), project.tracks.len());
    for track in &project.tracks {
        assert!(
            resources.routing.contains_key(&track.id),
            "track {:?} must have a routing entry",
            track.id
        );
    }
}

#[test]
fn wavetable_handles_exist_for_every_pitched_track() {
    // Round-1 has three pitched tracks (bass, lead, pad) and one
    // drum track. Wavetable handles must be present for exactly the
    // pitched indices — drum tracks land their own handle type in
    // U7. Each handle's NodeId must match the routing entry for
    // that track.
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);

    let pitched_indices: Vec<usize> = project
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t.synth, SynthAssignment::Wavetable(_)))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        resources.wavetable_handles.len(),
        pitched_indices.len(),
        "one handle per pitched track",
    );
    for idx in &pitched_indices {
        let handle = resources
            .wavetable_handles
            .get(idx)
            .unwrap_or_else(|| panic!("track {idx} should have a wavetable handle"));
        let track = &project.tracks[*idx];
        let routed = resources
            .routing
            .get(&track.id)
            .copied()
            .expect("routing covers every track");
        assert_eq!(
            handle.node_id, routed,
            "handle node_id must match routing entry"
        );
    }
}

#[test]
fn push_wavetable_param_succeeds_for_pitched_track_and_errors_for_drum() {
    // The lead track (round-1 idx 1) is Pitched/Wavetable; the
    // drums track (idx 2) is Drum. push_wavetable_param must accept
    // the former and reject the latter — the dispatcher surfaces
    // "no wavetable handle" so callers don't silently drop events
    // targeting the wrong synth.
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    use rawdaw_synth_wavetable::WavetableParam;

    let ok = resources.push_wavetable_param(1, WavetableParam::FilterCutoffHz, 1234.0);
    assert!(ok.is_ok(), "lead track must accept param: {ok:?}");

    let err = resources.push_wavetable_param(2, WavetableParam::FilterCutoffHz, 1234.0);
    assert!(err.is_err(), "drum track must reject wavetable param");
    let msg = err.unwrap_err();
    assert!(
        msg.contains("no wavetable handle"),
        "error must explain why ({msg})"
    );
}

#[test]
fn drum_handles_exist_for_every_drum_track() {
    // Round-1 has one drum track (drums). Drum handles must be
    // present for exactly the drum indices; pitched tracks use the
    // wavetable handles tested above. Each handle's NodeId must
    // match the routing entry for that track.
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);

    let drum_indices: Vec<usize> = project
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t.synth, SynthAssignment::Drum(_)))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        resources.drum_handles.len(),
        drum_indices.len(),
        "one handle per drum track",
    );
    for idx in &drum_indices {
        let handle = resources
            .drum_handles
            .get(idx)
            .unwrap_or_else(|| panic!("track {idx} should have a drum handle"));
        let track = &project.tracks[*idx];
        let routed = resources
            .routing
            .get(&track.id)
            .copied()
            .expect("routing covers every track");
        assert_eq!(
            handle.node_id, routed,
            "handle node_id must match routing entry"
        );
    }
}

#[test]
fn push_drum_param_succeeds_for_drum_track_and_errors_for_pitched() {
    // Mirror of the wavetable test: drums (idx 2 in round-1)
    // accepts a drum param; lead (idx 1) rejects it with "no drum
    // handle".
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    use rawdaw_synth_drum::DrumParam;

    let ok = resources.push_drum_param(2, DrumParam::KickStartHz, 200.0);
    assert!(ok.is_ok(), "drums track must accept param: {ok:?}");

    let err = resources.push_drum_param(1, DrumParam::KickStartHz, 200.0);
    assert!(err.is_err(), "lead track must reject drum param");
    let msg = err.unwrap_err();
    assert!(
        msg.contains("no drum handle"),
        "error must explain why ({msg})"
    );
}

#[test]
fn apply_wavetable_preset_ok_for_pitched_err_for_drum() {
    // Pitched (lead, idx 1) accepts the preset; drum (idx 2)
    // rejects with "no wavetable handle". The bell preset has 72
    // events; rejecting after the first ensures we don't half-
    // apply when targeting the wrong synth.
    use crate::presets::wavetable_preset_by_name;
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    let bell = wavetable_preset_by_name("bell").expect("bell preset present");

    let ok = resources.apply_wavetable_preset(1, bell.data);
    assert!(ok.is_ok(), "lead must accept preset: {ok:?}");

    let err = resources.apply_wavetable_preset(2, bell.data);
    assert!(err.is_err(), "drums must reject wavetable preset");
    assert!(err.unwrap_err().contains("no wavetable handle"));
}

#[test]
fn apply_drum_preset_ok_for_drum_err_for_pitched() {
    use crate::presets::drum_preset_by_name;
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    let acoustic = drum_preset_by_name("acoustic").expect("acoustic preset present");

    let ok = resources.apply_drum_preset(2, acoustic.data);
    assert!(ok.is_ok(), "drums must accept preset: {ok:?}");

    let err = resources.apply_drum_preset(1, acoustic.data);
    assert!(err.is_err(), "lead must reject drum preset");
    assert!(err.unwrap_err().contains("no drum handle"));
}

#[test]
fn push_wavetable_param_targets_correct_node_id() {
    // The handle's node_id is the audio-graph address for that
    // synth. push_wavetable_param routes through that address —
    // verified here by reading the handle's node_id and confirming
    // it matches the track's routing entry (since push_param won't
    // return anything we can inspect; the contract is that
    // handle.node_id == routing[track.id]).
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    let lead_id = project.tracks[1].id;
    let routed = resources.routing.get(&lead_id).copied().unwrap();
    let handle_node = resources
        .wavetable_handles
        .get(&1)
        .expect("lead has handle")
        .node_id;
    assert_eq!(handle_node, routed);
}

#[test]
fn node_layout_has_mixer_then_instruments_then_master_gain() {
    // Round-1 has an empty master-FX chain after a manual clear,
    // so the cpal-source NodeId resolves back to the master gain
    // node (the X3-introduced chain sits between gain and cpal
    // only when non-empty).
    let (mut project, _) = build_round1_project();
    project.master_chain.fx.clear();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    let n = project.tracks.len();
    // Instrument NodeIds are 1..=n.
    let mut instrument_ids: Vec<NodeId> = resources.routing.values().copied().collect();
    instrument_ids.sort_by_key(|n| n.get());
    let expected: Vec<NodeId> = (1..=n as u32).map(NodeId::new).collect();
    assert_eq!(instrument_ids, expected);
    // Mixer sits at NodeId(0) (not in routing — it's the bus, not
    // an instrument), and the master GainNode sits at NodeId(n+1).
    // With an empty FX chain the master gain is the cpal output.
    assert_eq!(resources.master, NodeId::new((n + 1) as u32));
}

#[test]
fn handle_is_usable_after_build() {
    // The handle should accept commands even when no audio device
    // is available — the host stays free to mutate the (silent)
    // graph.
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    let result = resources
        .handle()
        .push_command(GraphCommand::Batch(Vec::new()));
    assert!(result.is_ok(), "empty command batch should not overflow");
}

#[test]
fn stream_errors_queue_is_empty_at_startup() {
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    // Whether the driver opened or not, the error queue is empty
    // at startup — errors only arrive in response to a running
    // stream's mishaps.
    assert!(resources.next_stream_error().is_none());
}

#[test]
fn sample_clock_starts_at_zero() {
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    assert_eq!(
        resources
            .sample_clock
            .load(std::sync::atomic::Ordering::Acquire),
        0,
    );
}

#[test]
fn tempo_map_matches_project() {
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    assert_eq!(resources.tempo_map(), project.tempo_map);
}

#[test]
fn build_from_project_and_rate_does_not_register_poll_signal() {
    // Unit tests run outside the rinch runtime — calling
    // `poll_signal` would panic on the `is_main_thread` assert.
    // Guarantee that the test-only entry leaves `playhead_samples`
    // as a plain `Signal::new(0u64)` placeholder. The poll-signal
    // bridge is wired up in `build()`, not here.
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    assert_eq!(resources.playhead_samples.get(), 0u64);
}

#[test]
fn transport_starts_stopped_and_walks_the_state_machine() {
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);

    assert_eq!(resources.transport.get(), Transport::Stopped);

    resources.play().expect("play succeeds");
    assert_eq!(resources.transport.get(), Transport::Playing);

    resources.pause().expect("pause succeeds");
    assert_eq!(resources.transport.get(), Transport::Paused);

    resources.play().expect("play resumes");
    assert_eq!(resources.transport.get(), Transport::Playing);

    resources.stop().expect("stop succeeds");
    assert_eq!(resources.transport.get(), Transport::Stopped);
}

#[test]
fn realized_events_are_cached_for_replay() {
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    let cached = resources.realized_events.borrow().clone();
    assert_eq!(cached.len(), resources.initial_event_count);
    assert!(!cached.is_empty());
}

// ─── X3: master-FX chain wiring ────────────────────────────────────

#[test]
fn default_master_chain_routes_through_softclip_node() {
    // The round-1 fixture inherits the safety-net chain from
    // `MasterChainData::default` (single SoftClip). configure_graph
    // must allocate one FX slot after the master gain, install a
    // SoftClipNode there, and report `master = soft_clip_id`
    // (not the master-gain id) so cpal reads the shaped signal.
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);

    assert_eq!(resources.master_fx_handles.len(), 1);
    assert_eq!(
        resources.master_fx_handles[0].kind,
        super::MasterFxKind::SoftClip
    );

    let track_count = project.tracks.len() as u32;
    let master_gain_id = NodeId::new(track_count + 1);
    let soft_clip_id = NodeId::new(track_count + 2);
    assert_eq!(resources.master_fx_handles[0].node_id, soft_clip_id);
    assert_eq!(
        resources.master, soft_clip_id,
        "cpal must read from the last chain node, not the master gain"
    );
    // Sanity: master gain still lives at its historical NodeId so the
    // chain hangs off the right place.
    assert_ne!(resources.master, master_gain_id);
}

#[test]
fn empty_master_chain_keeps_master_gain_as_cpal_source() {
    // A project with `master_chain.fx = vec![]` (user explicitly
    // cleared the safety net) round-trips to a graph where cpal
    // reads from the master gain — preserves round-1 behavior
    // exactly when no FX is configured.
    let (mut project, _) = build_round1_project();
    project.master_chain.fx.clear();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);

    assert!(resources.master_fx_handles.is_empty());
    let track_count = project.tracks.len() as u32;
    let master_gain_id = NodeId::new(track_count + 1);
    assert_eq!(
        resources.master, master_gain_id,
        "with empty chain, cpal reads from master gain"
    );
}

#[test]
fn multi_entry_master_chain_routes_through_last_node() {
    // A two-entry chain (two SoftClips for synthetic v1 — future
    // EQ→SoftClip etc. use the same wiring). Validates that
    // NodeIds increment per slot, handles match positions, and
    // `master` resolves to the *last* slot's NodeId.
    use rawdaw_model::master_fx::{MasterFxData, SoftClipData, MASTER_CHAIN_FORMAT_VERSION,
        SOFT_CLIP_FORMAT_VERSION};
    let (mut project, _) = build_round1_project();
    project.master_chain = rawdaw_model::master_fx::MasterChainData {
        format_version: MASTER_CHAIN_FORMAT_VERSION,
        fx: vec![
            MasterFxData::SoftClip(SoftClipData {
                format_version: SOFT_CLIP_FORMAT_VERSION,
                threshold: 0.5,
            }),
            MasterFxData::SoftClip(SoftClipData {
                format_version: SOFT_CLIP_FORMAT_VERSION,
                threshold: 0.8,
            }),
        ],
    };
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);

    assert_eq!(resources.master_fx_handles.len(), 2);
    let track_count = project.tracks.len() as u32;
    let fx0_id = NodeId::new(track_count + 2);
    let fx1_id = NodeId::new(track_count + 3);
    assert_eq!(resources.master_fx_handles[0].node_id, fx0_id);
    assert_eq!(resources.master_fx_handles[1].node_id, fx1_id);
    assert_eq!(
        resources.master, fx1_id,
        "cpal must read from the final chain slot"
    );
}

#[test]
fn push_master_fx_param_succeeds_for_in_range_slot() {
    // X4: the push helper resolves slot → NodeId via the
    // master_fx_handles table and routes the Param event there.
    // Round-1's default chain has one slot, so slot 0 must
    // succeed.
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    use rawdaw_fx::SoftClipParam;
    let ok = resources.push_master_fx_param(0, SoftClipParam::Threshold, 0.4);
    assert!(ok.is_ok(), "slot 0 push must succeed: {ok:?}");
}

#[test]
fn push_master_fx_param_errors_for_out_of_range_slot() {
    // X4: an out-of-range slot returns Err rather than silently
    // dropping the event. Surfaces host-side bugs (e.g., stale
    // slot index after a chain reconfigure) fast.
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    use rawdaw_fx::SoftClipParam;
    let err = resources.push_master_fx_param(99, SoftClipParam::Threshold, 0.4);
    assert!(err.is_err(), "out-of-range slot must error");
    let msg = err.unwrap_err();
    assert!(msg.contains("slot 99"), "error message must name the slot: {msg}");
}

#[test]
fn master_fx_publisher_contract_is_observable_via_cloned_handle() {
    // X4: the publishers under the audio thread's
    // `with_patch_publishers` constructor are also held by
    // AudioResources via the `master_fx_publishers` field, so the
    // host side can lock the snapshot + read the version
    // independently of the audio thread. This pins the contract
    // that powers the X4 poll: a parameter change written by the
    // audio thread is observable through the publishers without
    // needing to reach into the node.
    //
    // The test simulates the audio thread by directly mutating
    // the publisher's snapshot + bumping the version, then asserts
    // the host can observe both transitions through its kept-clone
    // publisher.
    use rawdaw_fx::{SoftClipPatch, MIN_THRESHOLD};
    use std::sync::atomic::Ordering;
    let (project, _) = build_round1_project();
    let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
    let (_node_id, _kind, pubs) = &resources.master_fx_publishers[0];

    // Initial state: version 0, snapshot at default threshold.
    match pubs {
        super::MasterFxPublishers::SoftClip(p) => {
            let initial_version = p.version.load(Ordering::Acquire);
            let initial_threshold = p.snapshot.lock().unwrap().threshold;
            // Audio thread simulates a Param apply: write snapshot,
            // bump version. Mirrors `SoftClipNode::publish_patch`.
            {
                let mut guard = p.snapshot.lock().unwrap();
                *guard = SoftClipPatch::new(MIN_THRESHOLD);
            }
            p.version.fetch_add(1, Ordering::Release);
            // Host observes both transitions through its cloned
            // publisher (the same `Arc`s).
            assert_eq!(p.version.load(Ordering::Acquire), initial_version + 1);
            assert_eq!(p.snapshot.lock().unwrap().threshold, MIN_THRESHOLD);
            assert_ne!(p.snapshot.lock().unwrap().threshold, initial_threshold);
        }
    }
}

#[test]
fn master_fx_handle_count_always_matches_project_chain_length() {
    // Pinning the debug_assert contract in `build_from_project_and_rate`:
    // handle count must equal `project.master_chain.fx.len()`. Tested
    // across the three chain shapes (empty / single / multi).
    use rawdaw_model::master_fx::{MasterFxData, SoftClipData, MASTER_CHAIN_FORMAT_VERSION,
        SOFT_CLIP_FORMAT_VERSION};
    for chain_len in [0usize, 1, 3, 5] {
        let (mut project, _) = build_round1_project();
        project.master_chain = rawdaw_model::master_fx::MasterChainData {
            format_version: MASTER_CHAIN_FORMAT_VERSION,
            fx: (0..chain_len)
                .map(|_| {
                    MasterFxData::SoftClip(SoftClipData {
                        format_version: SOFT_CLIP_FORMAT_VERSION,
                        threshold: 0.7,
                    })
                })
                .collect(),
        };
        let resources =
            AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
        assert_eq!(
            resources.master_fx_handles.len(),
            chain_len,
            "handles must match chain_len={chain_len}",
        );
    }
}
