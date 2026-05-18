//! Audio engine wiring for rawdaw-app.
//!
//! Phases E3–E5 of the engine-wiring milestone — at app launch we
//!
//! 1. probe the default cpal output device for its sample rate;
//! 2. build a [`rawdaw_engine::Engine`] at that rate;
//! 3. install a per-track [`SineNode`] feeding a [`MixerNode`] master;
//! 4. realize the round-1 [`Project`], translate the events through a
//!    [`TrackRouting`] map, and push every [`BlockEvent`] into the
//!    queue;
//! 5. `engine.split()` into an [`AudioEngine`] + [`EngineHandle`], move
//!    the audio side into a [`CpalDriver`], and start the stream;
//! 6. (Phase E5) clone the engine's `Arc<AtomicU64>` sample clock,
//!    pair it with a UI-facing `Signal<u64>`, and spawn a background
//!    poller that mirrors atomic → signal at ~60 Hz so the
//!    arrangement playhead tracks the engine's transport position.
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
//! ## Playhead polling thread (E5)
//!
//! This is a **known anti-pattern** noted in the engine-wiring plan: we
//! poll an atomic from a background `std::thread` and `Signal::send`
//! the value into the UI. The rinch framework may grow a native
//! audio-thread → signal bridge later, at which point this can be
//! replaced. Until then, the thread:
//!
//! - sleeps `PLAYHEAD_POLL_INTERVAL_MS` between reads (target ~60 Hz);
//! - only dispatches a `Signal::send` when the atomic actually changed;
//! - is owned by an `Rc<PlayheadPoller>` whose `Drop` flips a stop flag
//!   and joins the thread, so the last [`AudioResources`] clone going
//!   out of scope cleans up the poller.
//!
//! The poller is only attached by the public [`AudioResources::build`]
//! entry; the test-facing [`AudioResources::build_from_project_and_rate`]
//! leaves `_poller = None` because unit tests don't run inside the
//! rinch runtime, so `Signal::send` from a background thread would
//! panic without a registered cross-thread dispatcher.
//!
//! ## Node layout
//!
//! - `master_node = NodeId(0)` is a [`MixerNode`] with `tracks.len()`
//!   stereo inputs, one per project track.
//! - Per track `i`, the sine instrument is `NodeId(i + 1)`. Its single
//!   stereo output port (port `0`) is wired to mixer input port `i`.

mod poller;

use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use rinch::prelude::Signal;

use poller::PlayheadPoller;

use rawdaw_engine::cpal_driver::{CpalDriver, StreamError};
use rawdaw_engine::node::AudioNode;
use rawdaw_engine::{
    translate_events, BlockEvent, Edge, Engine, EngineHandle, GraphCommand, MixerNode, NodeId,
    NodePort, TrackRouting, Transport, TransportHandle,
};
use rawdaw_fx::GainNode;
use rawdaw_model::fixtures::build_round1_project;
use rawdaw_model::project::Project;
use rawdaw_model::realize::realize;
use rawdaw_model::tempo::TempoMap;
use rawdaw_model::track::TrackKind;
use rawdaw_synth_drum::DrumSynthNode;
use rawdaw_synth_wavetable::WavetableSynthNode;

/// Fallback engine sample rate used when no cpal output device can be
/// probed. Matches the engine-side render tests so unit tests that
/// build `AudioResources` without an audio device get a deterministic
/// rate.
pub const FALLBACK_SAMPLE_RATE: u32 = 48_000;

/// Engine max block size. Holds across phases E3+; cpal's per-callback
/// block sizes (128–1024 typical) fit under this, and the engine
/// allocates scratch sized to this at construction.
pub const MAX_BLOCK: usize = 256;

/// Master output gain. Roughly -12 dB. Multi-voice synth chords +
/// the still-sine drum bursts comfortably exceed unity at the
/// mixer; this gives the cpal device clean headroom without a
/// soft-clipper. Replace with a real master-channel strip + a
/// soft-clipper once `rawdaw-fx` grows more nodes.
const MASTER_GAIN: f32 = 0.25;

/// Playhead poller sleep interval. ~60 Hz target — the smallest delay
/// that still produces visually-smooth scrubbing without burning a
/// core to update a single u64. Tuned in pairs with the engine's
/// per-block publication: at 48 kHz / 256 frames per block the audio
/// thread publishes the clock every ~5 ms, so 16 ms polling skips
/// roughly three publications per visible update.
const PLAYHEAD_POLL_INTERVAL_MS: u64 = 16;

/// Shared host-side handle on the audio engine and its topology.
///
/// Wrapped in `Rc<RefCell>` so the type satisfies the rinch
/// `create_store<T: Clone + 'static>` contract. The `RefCell` only
/// guards UI-thread access to the host-side [`EngineHandle`]; the
/// audio thread is reached entirely through the rtrb SPSC queues
/// that the handle's `push_command` / `push_event` methods drive.
#[derive(Clone)]
#[allow(dead_code)] // fields read by future phases (E6 transport)
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
    /// Engine sample clock — the absolute sample index at which the
    /// audio thread will start processing the *next* block. Updated
    /// with `Release` ordering at the end of every `process_block`.
    /// Hosts can `load(Acquire)` from any thread; the polling thread
    /// (when attached) mirrors this into [`Self::playhead_samples`].
    pub sample_clock: Arc<AtomicU64>,
    /// UI-facing reactive playhead position in samples. Components
    /// read `.get()` inside rsx attribute closures to subscribe.
    /// Starts at 0; the polling thread `Signal::send`s updates from
    /// a background thread (rinch routes them to the main thread
    /// via the registered cross-thread dispatcher).
    pub playhead_samples: Signal<u64>,
    /// Project tempo map cloned at build time. Used by the UI to
    /// convert `playhead_samples` → bars/beats for display. Constant
    /// today; the round-3 tempo-editor work will update this when the
    /// host edits the project.
    pub tempo_map: TempoMap,
    /// Transport state handle — Playing / Paused / Stopped. The host
    /// writes via [`Self::play`] / [`Self::pause`] / [`Self::stop`];
    /// the audio thread reads at the top of every `process_block`.
    pub transport: TransportHandle,
    /// UI-facing reactive mirror of [`Self::transport`]. Updated by
    /// the public play/pause/stop methods alongside the atomic. The
    /// rinch reactivity tracker only subscribes to `Signal` reads,
    /// not atomic loads — so any rsx attribute closure or `match`
    /// scrutinee that needs to re-render on transport changes must
    /// read this signal, not the [`TransportHandle`].
    pub transport_state: Signal<Transport>,
    /// Cached realized events. Pushed at build (E3) and re-pushed on
    /// every Stop → Play transition: the engine drains the event
    /// queue when entering Stopped, so a clean replay needs the host
    /// to refill the queue. Stored as `Rc<Vec<_>>` so cloning
    /// `AudioResources` for the rinch store doesn't deep-copy the
    /// realized stream every UI handler. `BlockEvent: Clone`, so the
    /// per-push iteration clones each event into the SPSC queue.
    realized_events: Rc<Vec<BlockEvent>>,
    /// Optional background poller. `Some` when [`Self::build`] was
    /// called inside a rinch runtime; `None` for unit tests. Dropping
    /// the last `Rc` clone stops the thread.
    _poller: Option<Rc<PlayheadPoller>>,
}

impl AudioResources {
    /// Build the round-1 project's audio stack. Probes the cpal device,
    /// constructs the engine, installs the per-track sine graph, pushes
    /// the realized events, opens the stream, and starts playback.
    /// Attaches a background playhead poller so the UI's
    /// [`Self::playhead_samples`] signal mirrors the engine's transport
    /// position. See the module doc for the failure-mode contract.
    ///
    /// Must be called inside a rinch runtime — the poller's
    /// `Signal::send` from a background thread requires the runtime's
    /// cross-thread dispatcher to be registered. Tests that don't
    /// initialize the runtime should call
    /// [`Self::build_from_project_and_rate`] directly instead.
    pub fn build() -> Self {
        let sample_rate =
            CpalDriver::probe_default_sample_rate().unwrap_or(FALLBACK_SAMPLE_RATE);
        let (project, _keys) = build_round1_project();
        let mut resources = Self::build_from_project_and_rate(&project, sample_rate);
        resources._poller = Some(Rc::new(PlayheadPoller::spawn(
            Arc::clone(&resources.sample_clock),
            resources.playhead_samples,
            PLAYHEAD_POLL_INTERVAL_MS,
        )));
        resources
    }

    /// Build for a specific project at a specific sample rate, *without*
    /// the playhead polling thread. Used directly by tests (which run
    /// outside the rinch runtime); production callers go through
    /// [`Self::build`].
    pub fn build_from_project_and_rate(project: &Project, sample_rate: u32) -> Self {
        let mut engine = Engine::new(sample_rate, MAX_BLOCK);
        let (master, routing, realized_events) = configure_graph(&mut engine, project, sample_rate);
        let initial_event_count = realized_events.len();
        let sample_clock = engine.sample_clock();
        let transport = engine.transport_handle();
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
            sample_clock,
            playhead_samples: Signal::new(0u64),
            tempo_map: project.tempo_map.clone(),
            transport,
            transport_state: Signal::new(Transport::default()),
            realized_events: Rc::new(realized_events),
            _poller: None,
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

    /// Transition to Playing.
    ///
    /// If currently Stopped, re-arms the event queue first — the
    /// engine's Stopped state drains every queued event so a clean
    /// replay needs the host to refill from `realized_events`. Then
    /// flips the transport atomic to Playing and (idempotently)
    /// starts the cpal stream so callbacks run.
    ///
    /// No-op for the audio-disabled launch (`driver = None`); the
    /// transport state still updates so the UI can render
    /// consistently.
    pub fn play(&self) -> Result<(), String> {
        if self.transport.get() == Transport::Stopped {
            self.rearm_events()?;
        }
        self.set_transport(Transport::Playing);
        let Some(driver) = self.driver.as_ref() else {
            return Ok(());
        };
        driver.play().map_err(|e| e.to_string())
    }

    /// Transition to Paused.
    ///
    /// The cpal stream keeps running so the audio thread can drain
    /// commands while paused — the engine sees `Transport::Paused`
    /// and silences output without draining events or advancing the
    /// sample clock. Pressing Play later resumes from the frozen
    /// position.
    pub fn pause(&self) -> Result<(), String> {
        self.set_transport(Transport::Paused);
        Ok(())
    }

    /// Transition to Stopped — silence, drain queued events, snap
    /// the playhead back to bar 1.
    ///
    /// The engine's Stopped state handles the actual drain on the
    /// audio thread; the host's responsibility is just to flip the
    /// atomic. Re-arming on the next [`Self::play`] handles the
    /// "play after stop" case.
    pub fn stop(&self) -> Result<(), String> {
        self.set_transport(Transport::Stopped);
        Ok(())
    }

    /// Write both the audio-thread atomic and the UI signal in one
    /// place. Keeps the two views of transport in lockstep so the
    /// reactive Play/Pause glyph (and any future transport-aware UI)
    /// never disagrees with what the audio thread is actually doing.
    fn set_transport(&self, state: Transport) {
        self.transport.set(state);
        self.transport_state.set(state);
    }

    /// Push every cached realized event back into the engine's event
    /// queue. Called from [`Self::play`] on a Stopped → Playing
    /// transition. Errors if the queue overflows mid-push — the host
    /// is the producer so this is a host-side capacity bug, not a
    /// race condition.
    fn rearm_events(&self) -> Result<(), String> {
        let mut handle = self.handle.borrow_mut();
        for ev in self.realized_events.iter() {
            handle
                .push_event(ev.clone())
                .map_err(|e| format!("event queue overflowed while re-arming: {e:?}"))?;
        }
        Ok(())
    }

    /// Current playhead position derived from [`Self::playhead_samples`]
    /// and the project's tempo map. Reads the signal — callers inside
    /// an rsx attribute closure will re-evaluate when the engine
    /// publishes a new sample-clock value.
    ///
    /// The returned `bar` and `beat` are 1-indexed for display (the
    /// project starts at "Bar 1 · Beat 1"). `bars_f64` is the raw
    /// fractional bar count from `MusicalTime`; arrangement code uses
    /// it for sub-bar percent positioning, the top-bar readout uses
    /// `(bar, beat)`.
    pub fn playhead_position(&self) -> PlayheadPosition {
        let samples = self.playhead_samples.get();
        let mt = self
            .tempo_map
            .sample_to_musical(rawdaw_model::SampleTime::samples(samples), self.sample_rate);
        let beats_per_bar = self.tempo_map.beats_per_bar_at(rawdaw_model::MusicalTime::ZERO);
        let total_beats = mt.as_beats_f64();
        let bars_f64 = total_beats / beats_per_bar as f64;
        let bar_idx = bars_f64.floor().max(0.0) as u32;
        let beat_in_bar = (total_beats - (bar_idx as f64) * beats_per_bar as f64)
            .floor()
            .max(0.0) as u32;
        PlayheadPosition {
            bar: bar_idx + 1,
            beat: beat_in_bar + 1,
            bars_f64,
        }
    }
}

/// Reactive playhead position, derived from the engine sample clock.
///
/// See [`AudioResources::playhead_position`]. `bar` and `beat` are
/// 1-indexed for display; `bars_f64` is the raw fractional position.
#[derive(Debug, Clone, Copy)]
pub struct PlayheadPosition {
    pub bar: u32,
    pub beat: u32,
    pub bars_f64: f64,
}

fn configure_graph(
    engine: &mut Engine,
    project: &Project,
    sample_rate: u32,
) -> (NodeId, TrackRouting, Vec<BlockEvent>) {
    let track_count = project.tracks.len();
    // Graph layout:
    //   NodeId(0)         — MixerNode (sums all instrument outputs).
    //   NodeId(1..=N)     — per-track instrument nodes.
    //   NodeId(N+1)       — master GainNode. cpal reads from here.
    // The mixer is no longer the master itself; the gain node sits
    // between mixer and output so multi-voice chords don't pre-clip.
    let mixer_id = NodeId::new(0);
    let master_id = NodeId::new((track_count + 1) as u32);
    let mut routing = TrackRouting::new();

    // One mixer + one instrument per track + one Connect per track +
    // master GainNode + Connect mixer → master. Batched so the
    // engine recomputes topo order once after the whole
    // reconfiguration.
    //
    // Per-track instrument picking is a stub for the round-3
    // instrument-assignment UI. For now: every Pitched track gets the
    // v0 wavetable synth, every Drum track gets the v0 drum synth.
    // The physical modeller will land on a per-role basis later
    // (Bass / Pad / Voicing will likely switch over).
    let mut commands: Vec<GraphCommand> = Vec::with_capacity(2 * track_count + 3);
    commands.push(GraphCommand::AddNode {
        id: mixer_id,
        node: Box::new(MixerNode::new(track_count)),
    });
    for (i, track) in project.tracks.iter().enumerate() {
        // (kind, synth) is paired correctly by Track::new; this
        // asserts the invariant at the audio-graph boundary so any
        // hand-mutated track surfaces in debug builds (release does
        // nothing — U3 will read track.synth here regardless of the
        // invariant; configure_graph trusts the track's `kind` as the
        // source of truth for node selection).
        debug_assert!(
            track.kind_matches_synth(),
            "track {:?} has kind/synth mismatch (kind = {:?})",
            track.id,
            track.kind,
        );
        let instrument_id = NodeId::new((i + 1) as u32);
        let node: Box<dyn AudioNode> = match &track.kind {
            TrackKind::Pitched { .. } => Box::new(WavetableSynthNode::new()),
            TrackKind::Drum { .. } => Box::new(DrumSynthNode::new()),
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

    // Realize → translate → push events. `translate_events` errors if
    // any realized event targets a track not in the routing; for
    // round-1 every track has a sine, so this is unreachable.
    let realized = realize(project, sample_rate);
    let block_events = translate_events(&realized, &routing)
        .expect("AudioResources routing must cover every realized track");
    for ev in &block_events {
        engine.push_event(ev.clone());
    }
    (master_id, routing, block_events)
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
    fn node_layout_has_mixer_then_instruments_then_master_gain() {
        let (project, _) = build_round1_project();
        let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
        let n = project.tracks.len();
        // Instrument NodeIds are 1..=n.
        let mut instrument_ids: Vec<NodeId> = resources.routing.values().copied().collect();
        instrument_ids.sort_by_key(|n| n.get());
        let expected: Vec<NodeId> = (1..=n as u32).map(NodeId::new).collect();
        assert_eq!(instrument_ids, expected);
        // Mixer sits at NodeId(0) (not in routing — it's the bus, not
        // an instrument), and the master GainNode sits at NodeId(n+1)
        // and is the cpal output read.
        assert_eq!(resources.master, NodeId::new((n + 1) as u32));
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
        assert_eq!(resources.tempo_map, project.tempo_map);
    }

    #[test]
    fn build_from_project_and_rate_does_not_spawn_poller() {
        // Unit tests run outside the rinch runtime — spawning the
        // poller would mean a future Signal::send panic. Guarantee
        // that the test entry leaves the field None.
        let (project, _) = build_round1_project();
        let resources = AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
        assert!(resources._poller.is_none());
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
        assert_eq!(resources.realized_events.len(), resources.initial_event_count);
        assert!(!resources.realized_events.is_empty());
    }
}
