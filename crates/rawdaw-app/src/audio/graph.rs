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
//! drives the UI's slider state. [`build_wavetable_pollers`] (live
//! only on the production `AudioResources::build` path, not the
//! test-facing `build_from_project_and_rate`) spawns one
//! [`WavetablePoller`](super::wavetable_poller::WavetablePoller)
//! per handle.

use std::collections::BTreeMap;
use std::sync::Arc;

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

use super::drum_poller::{DrumPoller, DRUM_PATCH_POLL_INTERVAL_MS};
use super::wavetable_poller::{WavetablePoller, WAVETABLE_PATCH_POLL_INTERVAL_MS};
use super::MASTER_GAIN;

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
    pub master: NodeId,
    pub routing: TrackRouting,
    pub realized_events: Vec<BlockEvent>,
    /// Captured at construction time so AudioResources can spawn one
    /// poller per Wavetable track without re-traversing the graph.
    pub wavetable_publishers: BTreeMap<usize, (NodeId, WavetablePublishers)>,
    /// Same shape as [`Self::wavetable_publishers`], for drum tracks.
    pub drum_publishers: BTreeMap<usize, (NodeId, DrumPublishers)>,
}

pub fn configure_graph(
    engine: &mut Engine,
    project: &Project,
    sample_rate: u32,
) -> ConfiguredGraph {
    let track_count = project.tracks.len();
    let mixer_id = NodeId::new(0);
    let master_id = NodeId::new((track_count + 1) as u32);
    let mut routing = TrackRouting::new();
    let mut wavetable_publishers: BTreeMap<usize, (NodeId, WavetablePublishers)> = BTreeMap::new();
    let mut drum_publishers: BTreeMap<usize, (NodeId, DrumPublishers)> = BTreeMap::new();

    // One mixer + one instrument per track + one Connect per track +
    // master GainNode + Connect mixer → master. Batched so the
    // engine recomputes topo order once after the whole
    // reconfiguration.
    let mut commands: Vec<GraphCommand> = Vec::with_capacity(2 * track_count + 3);
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
        id: master_id,
        node: Box::new(GainNode::new(MASTER_GAIN)),
    });
    commands.push(GraphCommand::Connect {
        edge: Edge {
            from: NodePort::new(mixer_id, 0),
            to: NodePort::new(master_id, 0),
        },
    });
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
        master: master_id,
        routing,
        realized_events,
        wavetable_publishers,
        drum_publishers,
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

/// Spawn one [`WavetablePoller`] per (publisher, handle) pair. Each
/// poller watches the publisher's atomic and pushes patch snapshots
/// into the matching handle's `Signal`.
pub fn build_wavetable_pollers(
    publishers: &BTreeMap<usize, WavetablePublishers>,
    handles: &BTreeMap<usize, WavetableEditorHandle>,
) -> Vec<WavetablePoller> {
    publishers
        .iter()
        .filter_map(|(idx, pubs)| {
            let handle = handles.get(idx)?;
            Some(WavetablePoller::spawn(
                Arc::clone(&pubs.version),
                Arc::clone(&pubs.snapshot),
                handle.patch_signal,
                WAVETABLE_PATCH_POLL_INTERVAL_MS,
            ))
        })
        .collect()
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

/// Spawn one [`DrumPoller`] per (publisher, handle) pair.
pub fn build_drum_pollers(
    publishers: &BTreeMap<usize, DrumPublishers>,
    handles: &BTreeMap<usize, DrumEditorHandle>,
) -> Vec<DrumPoller> {
    publishers
        .iter()
        .filter_map(|(idx, pubs)| {
            let handle = handles.get(idx)?;
            Some(DrumPoller::spawn(
                Arc::clone(&pubs.version),
                Arc::clone(&pubs.snapshot),
                handle.patch_signal,
                DRUM_PATCH_POLL_INTERVAL_MS,
            ))
        })
        .collect()
}
