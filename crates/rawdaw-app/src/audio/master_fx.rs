//! Master-FX chain plumbing for the audio graph.
//!
//! Mirrors the per-synth pattern in [`super::graph`]: a runtime
//! patch enum, a publisher enum (each variant wraps the
//! kind-specific `XxxPublishers`), a kind tag, and an editor
//! handle that pairs the slot's NodeId with a reactive
//! `Signal<MasterFxPatch>` the X6 UI surface will read.
//!
//! v1 has one FX kind ([`MasterFxKind::SoftClip`]). New kinds
//! extend each enum non-breakingly — the `build_master_fx_node`
//! match arm and the X6 UI dispatch grow one arm per kind.

use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use rinch::core::reactive::{poll_signal, PollRate};
use rinch::prelude::Signal;

use rawdaw_engine::node::AudioNode;
use rawdaw_engine::NodeId;
use rawdaw_fx::{SoftClipNode, SoftClipPatch, SoftClipPublishers};
use rawdaw_model::master_fx::MasterFxData;

/// Poll rate for master-FX patch mirrors. Matches the per-track
/// PATCH_POLL_RATE in `graph.rs` — 20 Hz, version-atomic-gated so
/// idle ticks are an atomic load + integer compare.
const MASTER_FX_POLL_RATE: PollRate = PollRate::Hz(20);

/// The runtime tag for a master-chain slot. Mirrors the v1
/// [`MasterFxData`] variants without the payload — the variant
/// alone is enough for the X4 push-helper to pick the right
/// `Param` encoder and for the X6 UI to pick the right editor
/// component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasterFxKind {
    SoftClip,
}

/// Runtime patch state for one slot. The X4 poller updates this
/// signal from the audio thread's publishers; the X6 UI reads it.
/// Enum so the per-slot [`MasterFxEditorHandle`] can carry one
/// concrete `Signal<MasterFxPatch>` type regardless of which
/// FX kind sits in the slot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MasterFxPatch {
    SoftClip(SoftClipPatch),
}

impl MasterFxPatch {
    /// FX-kind tag for this patch. Wired by X6 to pick the right
    /// editor component per slot.
    #[allow(dead_code)] // X6 dispatcher
    pub fn kind(&self) -> MasterFxKind {
        match self {
            Self::SoftClip(_) => MasterFxKind::SoftClip,
        }
    }
}

/// Audio-thread → host publisher pair for one slot. Cloned by the
/// graph builder so the host keeps a copy after the node moves
/// into the engine. The X4 poller reads `version` + `snapshot`
/// from this enum's inner publisher and side-loads into the
/// matching [`MasterFxEditorHandle::patch_signal`].
#[derive(Clone)]
pub enum MasterFxPublishers {
    SoftClip(SoftClipPublishers),
}

impl MasterFxPublishers {
    pub fn kind(&self) -> MasterFxKind {
        match self {
            Self::SoftClip(_) => MasterFxKind::SoftClip,
        }
    }
}

/// Editor handle for one master-FX slot. The host installs one of
/// these per chain entry in [`super::AudioResources`]'s
/// `master_fx_handles` table; the X6 `MasterFxEditor` reads
/// `patch_signal` to drive its sliders' initial + reactive values
/// and addresses `ParamEvent`s at `node_id`.
///
/// `Clone` is cheap — `Signal` is `Copy`, `NodeId` is `Copy`.
#[derive(Clone)]
#[allow(dead_code)] // fields read by X4 poller + X6 editor
pub struct MasterFxEditorHandle {
    /// NodeId of the FX node in the engine graph. The X6 editor
    /// builds `BlockEvent { target: node_id, ... }` to address
    /// Param events back at this slot.
    pub node_id: NodeId,
    /// Tag for kind-aware editor dispatch. Pulled from the
    /// underlying [`MasterFxData`] variant at construction time;
    /// changes only if the user mutates the chain itself (which
    /// causes a full re-`configure_graph`).
    pub kind: MasterFxKind,
    /// Reactive mirror of the audio-thread patch state for this
    /// slot. Updated by the X4 poller from
    /// [`MasterFxPublishers`] on every version advance.
    pub patch_signal: Signal<MasterFxPatch>,
}

/// Output of [`build_master_fx_node`] — bundled so the graph
/// builder can destructure into its `AddNode`/`Connect` command
/// list plus the publishers Vec without juggling three tuples.
pub struct MasterFxBuild {
    pub node: Box<dyn AudioNode>,
    pub kind: MasterFxKind,
    pub publishers: MasterFxPublishers,
}

/// Construct one FX node + its publishers from a serialized
/// [`MasterFxData`] entry. Caller assigns the NodeId and wires
/// the `AddNode` + chain `Connect` commands.
pub fn build_master_fx_node(data: &MasterFxData) -> MasterFxBuild {
    match data {
        MasterFxData::SoftClip(soft_clip_data) => {
            let patch = SoftClipPatch::from(*soft_clip_data);
            let pubs = SoftClipPublishers::new(patch);
            let node = SoftClipNode::with_patch_publishers(patch, pubs.clone());
            MasterFxBuild {
                node: Box::new(node),
                kind: MasterFxKind::SoftClip,
                publishers: MasterFxPublishers::SoftClip(pubs),
            }
        }
    }
}

/// Build the per-slot editor handle table from the audio-thread
/// publishers. Mirror of `build_wavetable_handles` /
/// `build_drum_handles` but the patch signal is enum-tagged so
/// the table is a `Vec<Handle>` (dense, indexed by chain slot)
/// rather than a `BTreeMap<usize, Handle>` (sparse, only Pitched
/// or Drum tracks present).
///
/// The signal seeds with the publisher's initial snapshot so the
/// X6 editor opens with the right values even when no poller has
/// run yet (test paths + first render).
pub fn build_master_fx_handles(
    publishers: &[(NodeId, MasterFxPublishers)],
) -> Rc<Vec<MasterFxEditorHandle>> {
    let handles = publishers
        .iter()
        .map(|(node_id, pubs)| {
            let kind = pubs.kind();
            let initial = match pubs {
                MasterFxPublishers::SoftClip(p) => {
                    let snap = p
                        .snapshot
                        .lock()
                        .map(|g| *g)
                        .unwrap_or_else(|_| SoftClipPatch::default());
                    MasterFxPatch::SoftClip(snap)
                }
            };
            MasterFxEditorHandle {
                node_id: *node_id,
                kind,
                patch_signal: Signal::new(initial),
            }
        })
        .collect::<Vec<_>>();
    Rc::new(handles)
}

/// Register one `poll_signal` per master-FX slot that mirrors the
/// audio-thread patch into the slot's [`MasterFxEditorHandle::patch_signal`].
/// Dispatch is enum-arm-based: each slot's
/// [`MasterFxPublishers`] variant carries the kind-specific
/// publisher type, and the closure wraps the resulting snapshot
/// back into the matching [`MasterFxPatch`] variant.
///
/// Returns nothing — the polls live for the lifetime of the app,
/// owned by rinch's main-thread registry. Same shape as
/// [`super::graph::attach_wavetable_poll_signals`].
///
/// `u64::MAX` is the "never seen" sentinel so a first version of
/// `0` (the audio thread's apply counter starts at 0) still
/// triggers an initial snapshot read. Matches the wavetable /
/// drum idiom exactly.
pub fn attach_master_fx_poll_signals(
    publishers: &[(NodeId, MasterFxKind, MasterFxPublishers)],
    handles: &[MasterFxEditorHandle],
) {
    debug_assert_eq!(
        publishers.len(),
        handles.len(),
        "attach_master_fx_poll_signals: publishers and handles must align by slot",
    );
    for (slot, (_node_id, _kind, pubs)) in publishers.iter().enumerate() {
        let Some(handle) = handles.get(slot) else {
            continue;
        };
        let target = handle.patch_signal;
        match pubs {
            MasterFxPublishers::SoftClip(soft_clip_pubs) => {
                let version = Arc::clone(&soft_clip_pubs.version);
                let snapshot = Arc::clone(&soft_clip_pubs.snapshot);
                let mut last_version: u64 = u64::MAX;
                let _: Signal<()> = poll_signal(
                    move || {
                        let now = version.load(Ordering::Acquire);
                        if now != last_version {
                            last_version = now;
                            if let Ok(guard) = snapshot.lock() {
                                target.set(MasterFxPatch::SoftClip(*guard));
                            }
                        }
                    },
                    MASTER_FX_POLL_RATE,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_model::master_fx::SoftClipData;

    #[test]
    fn build_master_fx_node_softclip_uses_data_threshold() {
        let data = MasterFxData::SoftClip(SoftClipData {
            format_version: 1,
            threshold: 0.4,
        });
        let built = build_master_fx_node(&data);
        assert_eq!(built.kind, MasterFxKind::SoftClip);
        // Reach through the publisher to verify the patch threshold
        // landed (the node itself doesn't expose its patch outside
        // of its publisher snapshot).
        match built.publishers {
            MasterFxPublishers::SoftClip(pubs) => {
                let snap = pubs.snapshot.lock().unwrap();
                assert_eq!(snap.threshold, 0.4);
            }
        }
    }

    #[test]
    fn build_master_fx_handles_seeds_signal_from_snapshot() {
        let data = MasterFxData::SoftClip(SoftClipData {
            format_version: 1,
            threshold: 0.5,
        });
        let built = build_master_fx_node(&data);
        let node_id = NodeId::new(42);
        let publishers = vec![(node_id, built.publishers)];
        let handles = build_master_fx_handles(&publishers);
        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].node_id, node_id);
        assert_eq!(handles[0].kind, MasterFxKind::SoftClip);
        match handles[0].patch_signal.get() {
            MasterFxPatch::SoftClip(p) => assert_eq!(p.threshold, 0.5),
        }
    }

    #[test]
    fn build_master_fx_handles_empty_for_empty_chain() {
        let handles = build_master_fx_handles(&[]);
        assert!(handles.is_empty());
    }
}
