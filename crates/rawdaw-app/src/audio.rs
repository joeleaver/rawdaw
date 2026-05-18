//! Audio engine wiring for rawdaw-app.
//!
//! Phases E3–E4 of the engine-wiring milestone — at app launch we
//!
//! 1. probe the default cpal output device for its sample rate;
//! 2. build a [`rawdaw_engine::Engine`] at that rate;
//! 3. install a per-track [`SineNode`] feeding a [`MixerNode`] master;
//! 4. realize the round-1 [`Project`], translate the events through a
//!    [`TrackRouting`] map, and push every [`BlockEvent`] into the
//!    queue;
//! 5. `engine.split()` into an [`AudioEngine`] + [`EngineHandle`], move
//!    the audio side into a [`CpalDriver`], and start the stream.
//!
//! After that the audio thread runs continuously, producing silence
//! while the engine has no pending NoteOn events. Once phase E6 wires
//! the transport buttons, hitting play will start consuming the
//! pre-pushed event stream and produce sine output.
//!
//! ## Failure modes
//!
//! `build()` is best-effort. If no cpal output device is available, or
//! the device's sample format isn't f32, or the stream fails to open,
//! the [`AudioResources`] is still constructed with a usable
//! [`EngineHandle`] — the UI just runs silent. This lets the binary
//! launch in headless test environments and on systems without audio
//! development packages installed, both of which the engine's own
//! `cpal-driver` Cargo feature was designed to accommodate.
//!
//! Non-fatal cpal stream errors raised during playback (device
//! disconnect, backend hiccups) flow through an mpsc channel; the host
//! polls via [`AudioResources::next_stream_error`]. The audio thread
//! never blocks: the channel's `Sender::send` is wait-free.
//!
//! ## Node layout
//!
//! - `master_node = NodeId(0)` is a [`MixerNode`] with `tracks.len()`
//!   stereo inputs, one per project track.
//! - Per track `i`, the sine instrument is `NodeId(i + 1)`. Its single
//!   stereo output port (port `0`) is wired to mixer input port `i`.

use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};

use rawdaw_engine::cpal_driver::{CpalDriver, StreamError};
use rawdaw_engine::{
    translate_events, Edge, Engine, EngineHandle, GraphCommand, MixerNode, NodeId, NodePort,
    SineNode, TrackRouting,
};
use rawdaw_model::fixtures::build_round1_project;
use rawdaw_model::project::Project;
use rawdaw_model::realize::realize;

/// Fallback engine sample rate used when no cpal output device can be
/// probed. Matches the engine-side render tests so unit tests that
/// build `AudioResources` without an audio device get a deterministic
/// rate.
pub const FALLBACK_SAMPLE_RATE: u32 = 48_000;

/// Engine max block size. Holds across phases E3+; cpal's per-callback
/// block sizes (128–1024 typical) fit under this, and the engine
/// allocates scratch sized to this at construction.
pub const MAX_BLOCK: usize = 256;

/// Shared host-side handle on the audio engine and its topology.
///
/// Wrapped in `Rc<RefCell>` so the type satisfies the rinch
/// `create_store<T: Clone + 'static>` contract. The `RefCell` only
/// guards UI-thread access to the host-side [`EngineHandle`]; the
/// audio thread is reached entirely through the rtrb SPSC queues
/// that the handle's `push_command` / `push_event` methods drive.
#[derive(Clone)]
#[allow(dead_code)] // fields read by future phases (E5 playhead, E6 transport)
pub struct AudioResources {
    handle: Rc<RefCell<EngineHandle>>,
    /// The cpal driver owns the running output stream. Held here for
    /// the app's lifetime so dropping `AudioResources` tears the
    /// stream down. `None` when no audio device was available at
    /// build time — the UI runs silent in that case.
    driver: Option<Rc<CpalDriver>>,
    /// Receiver for non-fatal stream errors. The host polls via
    /// `next_stream_error`; future UI work surfaces these as a
    /// non-blocking banner.
    stream_errors: Rc<Receiver<String>>,
    /// Mixer master at `NodeId(0)`.
    pub master: NodeId,
    /// Project track id → sine instrument node id. Immutable after
    /// `build`.
    pub routing: Rc<TrackRouting>,
    /// Number of events pushed into the engine at build time.
    pub initial_event_count: usize,
    /// Sample rate the engine was built with — either the cpal device's
    /// reported rate or [`FALLBACK_SAMPLE_RATE`] when probe failed.
    pub sample_rate: u32,
}

impl AudioResources {
    /// Build the round-1 project's audio stack. Probes the cpal device,
    /// constructs the engine, installs the per-track sine graph, pushes
    /// the realized events, opens the stream, and starts playback. See
    /// the module doc for the failure-mode contract.
    pub fn build() -> Self {
        let sample_rate =
            CpalDriver::probe_default_sample_rate().unwrap_or(FALLBACK_SAMPLE_RATE);
        let (project, _keys) = build_round1_project();
        Self::build_from_project_and_rate(&project, sample_rate)
    }

    /// Build for a specific project at a specific sample rate. Used
    /// directly by tests; production callers go through [`Self::build`].
    pub fn build_from_project_and_rate(project: &Project, sample_rate: u32) -> Self {
        let mut engine = Engine::new(sample_rate, MAX_BLOCK);
        let (master, routing, initial_event_count) = configure_graph(&mut engine, project, sample_rate);
        let (audio_engine, handle) = engine.split();

        let (err_tx, err_rx) = mpsc::channel::<String>();
        let driver = match CpalDriver::new(audio_engine, master, move |err: StreamError| {
            // The cpal error callback runs on a cpal-managed thread,
            // not the audio callback thread — `Sender::send` is
            // wait-free here. The receive end is single-threaded on
            // the UI side.
            let _ = err_tx.send(err.to_string());
        }) {
            Ok(d) => Some(Rc::new(d)),
            Err(e) => {
                // Print once on stderr so the failure mode is visible
                // when launched from a terminal. The UI still works;
                // the engine is armed but silent.
                eprintln!("audio: opening cpal stream failed ({e}); UI will run silent");
                None
            }
        };

        // The cpal stream is paused at construction — `CpalDriver::play`
        // hasn't been called. Audio remains silent until phase E6
        // wires the transport buttons to `AudioResources::play`. That
        // matches the milestone plan's E4 done-when ("audio thread
        // armed but outputs silence") without an engine-side transport
        // gate.

        Self {
            handle: Rc::new(RefCell::new(handle)),
            driver,
            stream_errors: Rc::new(err_rx),
            master,
            routing: Rc::new(routing),
            initial_event_count,
            sample_rate,
        }
    }

    /// Borrow the host-side [`EngineHandle`] for the duration of one
    /// call. Future phases push commands and transport events through
    /// this borrow.
    #[allow(dead_code)]
    pub fn handle(&self) -> RefMut<'_, EngineHandle> {
        self.handle.borrow_mut()
    }

    /// Drain the next pending stream error, if any. Non-blocking; safe
    /// to call from any UI handler. Returns `None` when the queue is
    /// empty or the audio thread has been torn down.
    #[allow(dead_code)]
    pub fn next_stream_error(&self) -> Option<String> {
        self.stream_errors.try_recv().ok()
    }

    /// Whether the cpal output stream opened successfully at build
    /// time. `false` means the UI is running with the engine armed but
    /// no audio output — useful for surfacing a status indicator.
    #[allow(dead_code)]
    pub fn audio_enabled(&self) -> bool {
        self.driver.is_some()
    }

    /// Start the audio callback. No-op when the driver didn't open
    /// (audio-disabled launch). Phase E6 hooks this to the play
    /// button.
    #[allow(dead_code)]
    pub fn play(&self) -> Result<(), String> {
        let Some(driver) = self.driver.as_ref() else {
            return Ok(());
        };
        driver.play().map_err(|e| e.to_string())
    }

    /// Pause the audio callback. No-op when the driver didn't open.
    #[allow(dead_code)]
    pub fn pause(&self) -> Result<(), String> {
        let Some(driver) = self.driver.as_ref() else {
            return Ok(());
        };
        driver.pause().map_err(|e| e.to_string())
    }
}

fn configure_graph(
    engine: &mut Engine,
    project: &Project,
    sample_rate: u32,
) -> (NodeId, TrackRouting, usize) {
    let track_count = project.tracks.len();
    let master_id = NodeId::new(0);
    let mut routing = TrackRouting::new();

    // One master + one sine per track + one Connect per track,
    // batched so the engine recomputes topo order once after the
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

    // Realize → translate → push events. `translate_events` errors if
    // any realized event targets a track not in the routing; for
    // round-1 every track has a sine, so this is unreachable.
    let realized = realize(project, sample_rate);
    let block_events = translate_events(&realized, &routing)
        .expect("AudioResources routing must cover every realized track");
    let count = block_events.len();
    for ev in block_events {
        engine.push_event(ev);
    }
    (master_id, routing, count)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn master_is_node_zero_and_other_nodes_follow() {
        let (project, _) = build_round1_project();
        let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
        assert_eq!(resources.master, NodeId::new(0));
        let mut sine_ids: Vec<NodeId> = resources.routing.values().copied().collect();
        sine_ids.sort_by_key(|n| n.get());
        let expected: Vec<NodeId> = (1..=sine_ids.len() as u32).map(NodeId::new).collect();
        assert_eq!(sine_ids, expected);
    }

    #[test]
    fn handle_is_usable_after_build() {
        // The handle should accept commands even when no audio device is
        // available — the host stays free to mutate the (silent) graph.
        let (project, _) = build_round1_project();
        let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
        // A noop batch is the cheapest exercise of the command path.
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
}
