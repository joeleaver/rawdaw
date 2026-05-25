//! Audio-graph configuration + per-track editor plumbing.
//!
//! [`configure_graph`] takes a fresh [`Engine`] and a [`Project`]
//! and lays down the round-1 graph topology:
//!
//! ```text
//!   NodeId(0)       MixerNode (sums all instrument outputs)
//!   NodeId(1..=N)   per-track instrument nodes
//!   NodeId(N+1)     master GainNode (cpal reads here)
//! ```
//!
//! Per-track instrument picking is a stub for the round-3
//! instrument-assignment UI. For now: every `Pitched` track gets
//! the v0 wavetable synth; every `Drum` track gets the v0 drum
//! synth. The physical modeller will land on a per-role basis later
//! (Bass / Pad / Voicing will likely switch over).
//!
//! For each Pitched track, [`configure_graph`] also creates a paired
//! [`WavetablePublishers`] *before* constructing the synth so the
//! host can keep a clone — [`build_wavetable_handles`] uses those
//! clones to seed the per-track [`WavetableEditorHandle`] that
//! drives the UI's slider state. [`attach_wavetable_poll_signals`]
//! (live only on the production `AudioResources::build` path, not
//! the test-facing `build_from_project_and_rate`) registers one
//! `poll_signal` per handle that side-loads patch snapshots into
//! the existing `patch_signal` whenever the version atomic advances.

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use rinch::core::reactive::{poll_signal, PollRate};
use rinch::prelude::Signal;

use rawdaw_engine::node::AudioNode;
use rawdaw_engine::{
    translate_events, BlockEvent, Edge, Engine, GraphCommand, MixerNode, NodeId, NodePort,
    TrackRouting,
};
use rawdaw_fx::GainNode;
use rawdaw_model::patch::SynthAssignment;
use rawdaw_model::project::Project;
use rawdaw_model::realize::realize;
use rawdaw_synth_drum::{DrumPatch, DrumPublishers, DrumSynthNode};
use rawdaw_synth_wavetable::{WavetablePatch, WavetablePublishers, WavetableSynthNode};

use super::master_fx::{build_master_fx_node, MasterFxKind, MasterFxPublishers};
use super::MASTER_GAIN;

/// Poll rate for the per-track patch mirrors. 20 Hz keeps the
/// external-source latency under one frame at 30 fps with negligible
/// CPU — the source closure is one atomic load and (only when the
/// version atomic advanced since last tick) a brief mutex critical
/// section.
const PATCH_POLL_RATE: PollRate = PollRate::Hz(20);

/// Editor handle for one Pitched track — surfaces the live patch
/// state + the NodeId needed to push `ParamEvent`s back at the
/// synth.
///
/// The host installs these in [`configure_graph`] (one per Pitched
/// track) and stores them on
/// [`AudioResources::wavetable_handles`](super::AudioResources::wavetable_handles).
/// The UI's `WavetableEditor` reads `patch_signal` to drive its
/// sliders' initial + reactive values and addresses Param events at
/// `node_id`. `Clone` is cheap — the inner fields are all `Arc` /
/// `Signal` (which is `Copy`).
#[derive(Clone)]
pub struct WavetableEditorHandle {
    /// NodeId of the wavetable synth in the engine graph. The U5
    /// editor builds `BlockEvent { target: node_id, ... }` to
    /// address Param events back at this synth.
    pub node_id: NodeId,
    /// Reactive mirror of the audio-thread patch state. Updated by
    /// the per-track
    /// [`WavetablePoller`](super::wavetable_poller::WavetablePoller)
    /// background thread on every version advance.
    pub patch_signal: Signal<WavetablePatch>,
}

/// Editor handle for one Drum track. Parallel to
/// [`WavetableEditorHandle`] — the host installs these in
/// [`configure_graph`] (one per Drum track) and stores them on
/// [`AudioResources::drum_handles`](super::AudioResources::drum_handles).
/// The U7 `DrumEditor` reads `patch_signal` for its sliders' initial
/// values and addresses Param events at `node_id`.
#[derive(Clone)]
pub struct DrumEditorHandle {
    /// NodeId of the drum synth in the engine graph. The U7 editor
    /// addresses Param events here.
    #[allow(dead_code)] // wired by U7-3
    pub node_id: NodeId,
    /// Reactive mirror of the audio-thread drum patch. Updated by
    /// the per-track
    /// [`DrumPoller`](super::drum_poller::DrumPoller) background
    /// thread on every version advance.
    pub patch_signal: Signal<DrumPatch>,
}

/// Output of [`configure_graph`] — bundled so the caller (which is
/// `AudioResources::build_from_project_and_rate`) can destructure
/// once and assign each field where it belongs.
pub struct ConfiguredGraph {
    /// NodeId that cpal reads from. With an empty master chain
    /// this is the master `GainNode` (round-1 behavior); with a
    /// non-empty chain this is the *last* FX node in the chain so
    /// the chain's shaping is what reaches the device.
    pub master: NodeId,
    pub routing: TrackRouting,
    pub realized_events: Vec<BlockEvent>,
    /// Captured at construction time so AudioResources can spawn one
    /// poller per Wavetable track without re-traversing the graph.
    pub wavetable_publishers: BTreeMap<usize, (NodeId, WavetablePublishers)>,
    /// Same shape as [`Self::wavetable_publishers`], for drum tracks.
    pub drum_publishers: BTreeMap<usize, (NodeId, DrumPublishers)>,
    /// Per-slot master-FX publishers, dense by chain index. Each
    /// entry pairs the slot's NodeId with the FX-kind tag and the
    /// kind-specific publishers wrapped in
    /// [`MasterFxPublishers`]. AudioResources clones each into a
    /// [`super::master_fx::MasterFxEditorHandle`] for the X6 UI
    /// (and X4 will register polls against them). Empty when
    /// `project.master_chain.fx` is empty.
    pub master_fx_publishers: Vec<(NodeId, MasterFxKind, MasterFxPublishers)>,
}

pub fn configure_graph(
    engine: &mut Engine,
    project: &Project,
    sample_rate: u32,
) -> ConfiguredGraph {
    let track_count = project.tracks.len();
    let chain_len = project.master_chain.fx.len();
    let mixer_id = NodeId::new(0);
    let master_gain_id = NodeId::new((track_count + 1) as u32);
    let mut routing = TrackRouting::new();
    let mut wavetable_publishers: BTreeMap<usize, (NodeId, WavetablePublishers)> = BTreeMap::new();
    let mut drum_publishers: BTreeMap<usize, (NodeId, DrumPublishers)> = BTreeMap::new();
    let mut master_fx_publishers: Vec<(NodeId, MasterFxKind, MasterFxPublishers)> =
        Vec::with_capacity(chain_len);

    // One mixer + one instrument per track + one Connect per track +
    // master GainNode + Connect mixer → master + (per master-FX
    // slot: one AddNode + one Connect). Batched so the engine
    // recomputes topo order once after the whole reconfiguration.
    let mut commands: Vec<GraphCommand> =
        Vec::with_capacity(2 * track_count + 3 + 2 * chain_len);
    commands.push(GraphCommand::AddNode {
        id: mixer_id,
        node: Box::new(MixerNode::new(track_count)),
    });
    for (i, track) in project.tracks.iter().enumerate() {
        // (kind, synth) is paired correctly by Track::new; this
        // asserts the invariant at the audio-graph boundary so any
        // hand-mutated track surfaces in debug builds.
        debug_assert!(
            track.kind_matches_synth(),
            "track {:?} has kind/synth mismatch (kind = {:?})",
            track.id,
            track.kind,
        );
        let instrument_id = NodeId::new((i + 1) as u32);
        let node: Box<dyn AudioNode> = match &track.synth {
            SynthAssignment::Wavetable(data) => {
                let patch = WavetablePatch::from(*data);
                // Create publishers BEFORE constructing the node so
                // the host can keep a clone for its editor handle.
                let pubs = WavetablePublishers::new(patch);
                wavetable_publishers.insert(i, (instrument_id, pubs.clone()));
                Box::new(WavetableSynthNode::with_patch_publishers(patch, pubs))
            }
            SynthAssignment::Drum(data) => {
                let patch = DrumPatch::from(*data);
                let pubs = DrumPublishers::new(patch);
                drum_publishers.insert(i, (instrument_id, pubs.clone()));
                Box::new(DrumSynthNode::with_patch_publishers(patch, pubs))
            }
        };
        commands.push(GraphCommand::AddNode {
            id: instrument_id,
            node,
        });
        commands.push(GraphCommand::Connect {
            edge: Edge {
                from: NodePort::new(instrument_id, 0),
                to: NodePort::new(mixer_id, i as u8),
            },
        });
        routing.insert(track.id, instrument_id);
    }
    // Master GainNode after the mixer.
    commands.push(GraphCommand::AddNode {
        id: master_gain_id,
        node: Box::new(GainNode::new(MASTER_GAIN)),
    });
    commands.push(GraphCommand::Connect {
        edge: Edge {
            from: NodePort::new(mixer_id, 0),
            to: NodePort::new(master_gain_id, 0),
        },
    });

    // Master-FX chain. NodeIds N+2..=N+1+chain_len; first node hangs
    // off the master gain; each subsequent node hangs off its
    // predecessor. cpal reads from the final node in the chain
    // (resolved into `master_output_id` below) — or from the master
    // gain directly when the chain is empty (round-1 behavior).
    let master_fx_base_id = (track_count + 2) as u32;
    let mut prev_out_id = master_gain_id;
    for (slot, data) in project.master_chain.fx.iter().enumerate() {
        let fx_id = NodeId::new(master_fx_base_id + slot as u32);
        let built = build_master_fx_node(data);
        master_fx_publishers.push((fx_id, built.kind, built.publishers));
        commands.push(GraphCommand::AddNode {
            id: fx_id,
            node: built.node,
        });
        commands.push(GraphCommand::Connect {
            edge: Edge {
                from: NodePort::new(prev_out_id, 0),
                to: NodePort::new(fx_id, 0),
            },
        });
        prev_out_id = fx_id;
    }
    let master_output_id = prev_out_id;
    engine.push_command(GraphCommand::Batch(commands));

    // Realize → translate → push events. `translate_events` errors
    // if any realized event targets a track not in the routing;
    // for round-1 every track has a synth, so this is unreachable.
    let realized = realize(project, sample_rate);
    let realized_events = translate_events(&realized, &routing)
        .expect("AudioResources routing must cover every realized track");
    for ev in &realized_events {
        engine.push_event(ev.clone());
    }
    ConfiguredGraph {
        master: master_output_id,
        routing,
        realized_events,
        wavetable_publishers,
        drum_publishers,
        master_fx_publishers,
    }
}

/// Build the per-track editor handle table. One entry per Pitched
/// track; the inner [`Signal`] is seeded from the audio thread's
/// boot snapshot so the editor opens with the right values even
/// when no poller has run yet (tests; first-render before the
/// poller's interval elapses).
pub fn build_wavetable_handles(
    publishers: &BTreeMap<usize, (NodeId, WavetablePublishers)>,
) -> BTreeMap<usize, WavetableEditorHandle> {
    publishers
        .iter()
        .map(|(idx, (node_id, pubs))| {
            let initial = pubs
                .snapshot
                .lock()
                .map(|g| *g)
                .unwrap_or_else(|_| WavetablePatch::default());
            let handle = WavetableEditorHandle {
                node_id: *node_id,
                patch_signal: Signal::new(initial),
            };
            (*idx, handle)
        })
        .collect()
}

/// Strip the [`NodeId`] off each entry — the publishers are kept
/// separately for poller construction. Tracks stay keyed in the
/// right order via BTreeMap.
pub fn extract_publishers(
    src: &BTreeMap<usize, (NodeId, WavetablePublishers)>,
) -> BTreeMap<usize, WavetablePublishers> {
    src.iter()
        .map(|(idx, (_, pubs))| (*idx, pubs.clone()))
        .collect()
}

/// Register one `poll_signal` per (publisher, handle) pair. Each
/// poll closure watches the publisher's version atomic on the rinch
/// main-loop ticker; the mutex-guarded patch snapshot is only locked
/// when the version actually advanced since the last tick, so the
/// audio thread's RT determinism is preserved (idle ticks are an
/// atomic load + integer compare).
///
/// Returns nothing — the polls live for the lifetime of the app,
/// owned by rinch's main-thread registry. Mirror of the original
/// `WavetablePoller`-per-handle shape but with no `std::thread`,
/// no JoinHandle bookkeeping, no Drop dance.
pub fn attach_wavetable_poll_signals(
    publishers: &BTreeMap<usize, WavetablePublishers>,
    handles: &BTreeMap<usize, WavetableEditorHandle>,
) {
    for (idx, pubs) in publishers {
        let Some(handle) = handles.get(idx) else {
            continue;
        };
        let version = Arc::clone(&pubs.version);
        let snapshot = Arc::clone(&pubs.snapshot);
        let target: Signal<WavetablePatch> = handle.patch_signal;
        // `u64::MAX` is the "never seen" sentinel so a first version
        // of `0` (the audio thread's apply counter starts at 0) still
        // triggers an initial snapshot read. Matches the prior
        // WavetablePoller idiom.
        let mut last_version: u64 = u64::MAX;
        // Side-effect closure: poll_signal's returned `Signal<()>` is
        // discarded; the load-bearing update is `target.set(...)`
        // inside the closure when version advances.
        let _: Signal<()> = poll_signal(
            move || {
                let now = version.load(Ordering::Acquire);
                if now != last_version {
                    last_version = now;
                    if let Ok(guard) = snapshot.lock() {
                        target.set(*guard);
                    }
                }
            },
            PATCH_POLL_RATE,
        );
    }
}

// ── Drum equivalents ──────────────────────────────────────────────

/// Build the per-track drum editor handle table. One entry per Drum
/// track. Same seeding contract as
/// [`build_wavetable_handles`].
pub fn build_drum_handles(
    publishers: &BTreeMap<usize, (NodeId, DrumPublishers)>,
) -> BTreeMap<usize, DrumEditorHandle> {
    publishers
        .iter()
        .map(|(idx, (node_id, pubs))| {
            let initial = pubs
                .snapshot
                .lock()
                .map(|g| *g)
                .unwrap_or_else(|_| DrumPatch::default());
            let handle = DrumEditorHandle {
                node_id: *node_id,
                patch_signal: Signal::new(initial),
            };
            (*idx, handle)
        })
        .collect()
}

/// Strip [`NodeId`] off each drum entry — the publishers are kept
/// separately for poller construction.
pub fn extract_drum_publishers(
    src: &BTreeMap<usize, (NodeId, DrumPublishers)>,
) -> BTreeMap<usize, DrumPublishers> {
    src.iter()
        .map(|(idx, (_, pubs))| (*idx, pubs.clone()))
        .collect()
}

/// Drum mirror of [`attach_wavetable_poll_signals`]. Identical
/// version-atomic-gated side-effect-closure shape.
pub fn attach_drum_poll_signals(
    publishers: &BTreeMap<usize, DrumPublishers>,
    handles: &BTreeMap<usize, DrumEditorHandle>,
) {
    for (idx, pubs) in publishers {
        let Some(handle) = handles.get(idx) else {
            continue;
        };
        let version = Arc::clone(&pubs.version);
        let snapshot = Arc::clone(&pubs.snapshot);
        let target: Signal<DrumPatch> = handle.patch_signal;
        let mut last_version: u64 = u64::MAX;
        let _: Signal<()> = poll_signal(
            move || {
                let now = version.load(Ordering::Acquire);
                if now != last_version {
                    last_version = now;
                    if let Ok(guard) = snapshot.lock() {
                        target.set(*guard);
                    }
                }
            },
            PATCH_POLL_RATE,
        );
    }
}
