//! Audio engine wiring for rawdaw-app.
//!
//! Phase E3 of the engine-wiring milestone — builds an
//! [`rawdaw_engine::Engine`] at app launch, installs a per-track
//! [`SineNode`] feeding into a [`MixerNode`] master, runs
//! [`rawdaw_model::realize::realize`] over the project, translates the
//! resulting event stream through a [`TrackRouting`] map, and pushes
//! every [`BlockEvent`] into the engine's event queue.
//!
//! ## No audio thread yet
//!
//! Nothing calls [`rawdaw_engine::Engine::process_block`]. The engine is
//! armed and ready; phase E4 spins up the cpal callback. For E3 the
//! `Engine` lives inside an `Rc<RefCell>` so the rinch store contract
//! (`Clone + 'static`) is satisfied; phase E4 will split() the engine
//! and replace this with an `EngineHandle` shared via channels.
//!
//! ## Node layout
//!
//! - `master_node = NodeId(0)` is a [`MixerNode`] with `tracks.len()`
//!   stereo inputs, one per project track.
//! - Per track `i`, the sine instrument is `NodeId(i + 1)`. Its single
//!   stereo output port (port `0`) is wired to mixer input port `i`.
//!
//! All graph commands flow through a single
//! [`GraphCommand::Batch`] so the topo order recomputes once.

use std::cell::{RefCell, RefMut};
use std::rc::Rc;

use rawdaw_engine::{
    translate_events, Edge, Engine, GraphCommand, MixerNode, NodeId, NodePort, SineNode,
    TrackRouting,
};
use rawdaw_model::fixtures::build_round1_project;
use rawdaw_model::realize::realize;
use rawdaw_model::project::Project;

/// Engine sample rate for phase E3. Matches the engine-side render
/// tests. Phase E4 picks this up from the cpal device's actual sample
/// rate at stream-open time.
pub const SAMPLE_RATE: u32 = 48_000;

/// Engine max block size for phase E3. Matches the engine-side render
/// tests; cpal-typical block sizes (128–1024) fit under this. Phase E4
/// will revisit when the audio device is actually opened.
pub const MAX_BLOCK: usize = 256;

/// Shared host-side handle on the audio engine and its topology.
///
/// Wrapped in `Rc<RefCell>` so the type satisfies the rinch
/// `create_store<T: Clone + 'static>` contract. Single-threaded UI
/// thread for now; the audio-thread split moves the engine off the
/// `RefCell` in phase E4.
///
/// All fields are read by tests and by future phases (E4 cpal wiring,
/// E5 playhead, E6 transport); the in-process bin doesn't consume
/// them yet, hence the explicit dead-code allow.
#[derive(Clone)]
#[allow(dead_code)]
pub struct AudioResources {
    engine: Rc<RefCell<Engine>>,
    /// Mixer master at `NodeId(0)`. `render_offline` / `process_block`
    /// callers pass this as the output root.
    pub master: NodeId,
    /// Project track id → sine instrument node id. Immutable after
    /// `build`; wrapped in `Rc` so the store stays cheap to clone.
    pub routing: Rc<TrackRouting>,
    /// Number of events pushed into the engine at build time. Equal
    /// to `realize(project).len()` for a project whose routing covers
    /// every active track. Surfaced for the engine-side cross-check
    /// test the milestone's done-when requires.
    pub initial_event_count: usize,
}

impl AudioResources {
    /// Build the engine for the round-1 fixture. Calls
    /// `build_round1_project()` and forwards to [`Self::build_from_project`].
    pub fn build() -> Self {
        let (project, _keys) = build_round1_project();
        Self::build_from_project(&project)
    }

    /// Build the engine for a specific project. Useful for tests that
    /// want to feed a known event stream and verify counts.
    pub fn build_from_project(project: &Project) -> Self {
        let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);

        let track_count = project.tracks.len();
        let master_id = NodeId::new(0);
        let mut routing = TrackRouting::new();

        // 1 master + 1 sine per track + 1 connect per track. Batch
        // them so the graph's topo-order recomputes once after the
        // whole reconfiguration.
        let mut commands: Vec<GraphCommand> = Vec::with_capacity(2 * track_count + 1);
        commands.push(GraphCommand::AddNode {
            id: master_id,
            node: Box::new(MixerNode::new(track_count)),
        });
        for (i, track) in project.tracks.iter().enumerate() {
            let sine_id = NodeId::new((i + 1) as u32);
            commands.push(GraphCommand::AddNode {
                id: sine_id,
                node: Box::new(SineNode::new()),
            });
            commands.push(GraphCommand::Connect {
                edge: Edge {
                    from: NodePort::new(sine_id, 0),
                    to: NodePort::new(master_id, i as u8),
                },
            });
            routing.insert(track.id, sine_id);
        }
        engine.push_command(GraphCommand::Batch(commands));

        // Realize → translate → push events. `translate_events` errors
        // if any realized event targets a track not in the routing;
        // for round-1 every track has a sine, so the error path is
        // unreachable. Panic if it ever fires — the host built the
        // routing wrong.
        let realized = realize(project, SAMPLE_RATE);
        let block_events = translate_events(&realized, &routing)
            .expect("AudioResources routing must cover every realized track");
        let initial_event_count = block_events.len();
        for ev in block_events {
            engine.push_event(ev);
        }

        Self {
            engine: Rc::new(RefCell::new(engine)),
            master: master_id,
            routing: Rc::new(routing),
            initial_event_count,
        }
    }

    /// Borrow the engine for the duration of one call. Phase E3 callers
    /// only need this for offline rendering smoke checks; phase E4
    /// replaces this with a split + cpal callback wiring.
    #[allow(dead_code)]
    pub fn engine(&self) -> RefMut<'_, Engine> {
        self.engine.borrow_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_1_pushes_one_block_event_per_realized_event() {
        let (project, _) = build_round1_project();
        let realized = realize(&project, SAMPLE_RATE);
        let resources = AudioResources::build();
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
        let resources = AudioResources::build();
        let (project, _) = build_round1_project();
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
    fn master_is_node_zero_and_other_nodes_follow() {
        let resources = AudioResources::build();
        assert_eq!(resources.master, NodeId::new(0));
        // Each routed sine gets a NodeId(1..) — verify the count.
        let mut sine_ids: Vec<NodeId> = resources.routing.values().copied().collect();
        sine_ids.sort_by_key(|n| n.get());
        let expected: Vec<NodeId> = (1..=sine_ids.len() as u32).map(NodeId::new).collect();
        assert_eq!(sine_ids, expected);
    }

    #[test]
    fn built_engine_renders_offline_without_panicking() {
        // Sanity check on the graph topology + queue setup. The
        // realized events drive sine output through the mixer, but
        // we don't assert on the audio content yet — the done-when
        // for phase E3 is "engine builds without panics + event
        // count matches realize()". Audible verification lives with
        // phase E4 once cpal is wired.
        let resources = AudioResources::build();
        let mut engine = resources.engine();
        let result = engine.render_offline(
            resources.master,
            rawdaw_model::SampleTime::samples(SAMPLE_RATE as u64 / 4),
            MAX_BLOCK,
        );
        assert_eq!(result.left.len(), SAMPLE_RATE as usize / 4);
        assert_eq!(result.right.len(), SAMPLE_RATE as usize / 4);
    }
}
