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

mod edit_pump;
mod graph;
mod midi;
mod synth_ops;

use std::cell::{RefCell, RefMut};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use rinch::core::reactive::{poll_signal, PollRate};
use rinch::prelude::Signal;

use midi::first_pitched_track_node_id;
use graph::{
    attach_drum_poll_signals, attach_wavetable_poll_signals, build_drum_handles,
    build_wavetable_handles, configure_graph, extract_drum_publishers, extract_publishers,
    ConfiguredGraph,
};

use rawdaw_engine::cpal_driver::{CpalDriver, StreamError};
use rawdaw_engine::{
    BlockEvent, Engine, EngineHandle, MidiInputHandle, NodeId, TrackRouting, Transport,
    TransportHandle,
};
use crate::initial_project;
use rawdaw_model::project::Project;
use rawdaw_model::tempo::TempoMap;
use rawdaw_synth_drum::DrumPublishers;
use rawdaw_synth_wavetable::WavetablePublishers;

use crate::midi_input::MidiInputBridge;
use midir::MidiInputConnection;

pub use graph::{DrumEditorHandle, WavetableEditorHandle};

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

/// Playhead poll rate. ~60 Hz target — visually smooth without
/// burning a core to mirror a single `u64`. Tuned in pairs with the
/// engine's per-block publication: at 48 kHz / 256 frames per block
/// the audio thread publishes the clock every ~5 ms, so 16 ms
/// polling skips roughly three publications per visible update.
const PLAYHEAD_POLL_RATE: PollRate = PollRate::Hz(60);

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
    /// Project tempo map cached for UI sample → bars/beats conversion.
    /// Updated by [`Self::apply_project_edit`] whenever the project's
    /// tempo changes; read via [`Self::tempo_map`].
    ///
    /// `Rc<RefCell<_>>` for interior mutability — `AudioResources` is
    /// `Clone` (rinch store contract) and the edit pump runs from UI
    /// click handlers (`&self`), so the cell is the way to mutate
    /// without restructuring every consumer.
    tempo_map: Rc<RefCell<TempoMap>>,
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
    ///
    /// C2 wraps the `Rc<Vec<_>>` in an `Rc<RefCell<_>>` so the edit
    /// pump can replace it wholesale after re-realize — readers
    /// clone the inner `Rc` and the swap is one Vec-pointer write.
    realized_events: Rc<RefCell<Rc<Vec<BlockEvent>>>>,
    /// Owned snapshot of the project the engine was built from. UI
    /// components read this (via [`Self::project`]) to surface track
    /// names, kinds, and per-track synth assignments —
    /// `tracks[idx].synth` drives the U4 synth-editor dispatcher.
    /// `Rc<RefCell<Rc<Project>>>` so the edit pump can swap the inner
    /// `Rc` from `&self`; readers clone the Rc through the accessor.
    /// The per-parameter live patch state lives on the audio thread
    /// and is mirrored to the UI via [`WavetableEditorHandle`] — this
    /// snapshot tracks the as-edited project structure (notes,
    /// chords, tempo) and is intentionally distinct from per-synth
    /// patch state.
    project: Rc<RefCell<Rc<Project>>>,
    /// Per-pitched-track editor handles, keyed by `project.tracks`
    /// index. Each handle carries the synth's NodeId (for addressing
    /// `ParamEvent`s back at the right node) and a reactive
    /// `Signal<WavetablePatch>` mirroring the audio-thread's
    /// patch state. Populated for `TrackKind::Pitched` tracks
    /// (Wavetable synth) only; drum tracks land their own handle
    /// type in U7. `Rc` because the table is keyed by `usize` and
    /// the values clone cheaply (Arcs + Signal).
    pub wavetable_handles: Rc<BTreeMap<usize, WavetableEditorHandle>>,
    /// Audio-thread publishers paired with each Wavetable handle.
    /// Kept around so `build()` can register the per-track
    /// `poll_signal` mirror (test path leaves no polls attached).
    /// Private because the publishers are an implementation detail
    /// — UI callers go through [`WavetableEditorHandle`].
    wavetable_publishers: Rc<BTreeMap<usize, WavetablePublishers>>,
    /// Per-drum-track editor handles, keyed by `project.tracks` index.
    /// Each handle carries the synth's NodeId and a reactive
    /// `Signal<DrumPatch>` mirroring the audio-thread's drum patch.
    /// Populated for `TrackKind::Drum` tracks only.
    pub drum_handles: Rc<BTreeMap<usize, DrumEditorHandle>>,
    /// Audio-thread publishers paired with each Drum handle. Kept
    /// alongside the handles for the same reasons as the wavetable
    /// publishers field above.
    drum_publishers: Rc<BTreeMap<usize, DrumPublishers>>,
    // The PlayheadPoller / WavetablePoller / DrumPoller fields (each
    // an `Rc<std::thread>` + AtomicBool stop flag) were all removed
    // when we adopted `rinch::core::reactive::poll_signal` (rinch
    // issue [#28](https://github.com/joeleaver/rinch/issues/28)).
    // The patch + playhead Signals now drive off the runtime's
    // per-frame poll drain — no threads, no Drop dances. See
    // `attach_wavetable_poll_signals` / `attach_drum_poll_signals`
    // in `graph.rs` for the version-atomic-gated side-effect
    // closures.
    /// MIDI input handle for the engine's dedicated MIDI input SPSC
    /// queue. `Some(_)` after `build_from_project_and_rate`; `take()`n
    /// by [`Self::build`] when it opens a midir input connection.
    /// `None` afterwards (the handle moves into midir's callback for
    /// the lifetime of the connection). Tests that don't go through
    /// `build()` drop the handle when AudioResources drops.
    ///
    /// `Rc<RefCell<>>` because AudioResources is `Clone` (rinch store
    /// contract) and the handle is `!Sync + !Clone`.
    pub(super) midi_input_handle: Rc<RefCell<Option<MidiInputHandle>>>,
    /// Active midir input connection, when a device is open. Held to
    /// keep the connection alive — when this drops, midir closes the
    /// port. `None` when no device was found at auto-pick (the host
    /// runs without MIDI input silently). K2's UI picker
    /// `take()`s and replaces this on device switch.
    pub(super) _midi_connection: Rc<RefCell<Option<MidiInputConnection<MidiInputBridge>>>>,
    /// MIDI input routing target — the NodeId of the synth that
    /// receives incoming MIDI events. Shared `Arc` with the
    /// [`MidiInputBridge`] inside midir's callback. K3 writes to this
    /// atomic from the host (via [`Self::set_midi_target_track`])
    /// whenever the user picks a different track; the bridge reads
    /// on every event. Initialized to the first Pitched track's
    /// NodeId by `build_from_project_and_rate`.
    pub(super) midi_target: Arc<AtomicU32>,
    /// Reactive mirror of the currently-open MIDI input device's
    /// name. `None` when nothing's open (no device found at boot,
    /// or user disconnected via K2's picker). The MidiPicker
    /// component reads this to render the dropdown's selected
    /// option; [`Self::set_midi_device`] updates it.
    pub current_midi_device: Signal<Option<String>>,
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
        // Boot from the shared initial-project factory; the overlay is
        // produced too but only the model side is fed into the audio
        // graph. AppState consumes the overlay separately in
        // `app::main_window`.
        let (project, _overlay) = initial_project::build_initial();
        let mut resources = Self::build_from_project_and_rate(&project, sample_rate);
        // Replace the placeholder Signal::new(0) from
        // build_from_project_and_rate with a poll_signal-backed one
        // that the rinch runtime drives once per frame. UI bindings
        // happen on the AudioResources returned from `build()`, so
        // the swap lands before anything subscribes.
        let clock = Arc::clone(&resources.sample_clock);
        resources.playhead_samples =
            poll_signal(move || clock.load(Ordering::Acquire), PLAYHEAD_POLL_RATE);
        // Register one `poll_signal` per pitched / drum track. Lives
        // in `build()` rather than `build_from_project_and_rate`
        // because `poll_signal` asserts it's called on the main
        // thread (which `build()`'s caller has, but tests haven't).
        attach_wavetable_poll_signals(
            &resources.wavetable_publishers,
            &resources.wavetable_handles,
        );
        attach_drum_poll_signals(
            &resources.drum_publishers,
            &resources.drum_handles,
        );
        resources.open_default_midi_input();
        resources
    }

    /// Auto-pick the first available MIDI input device and open a
    /// connection routed to the first Pitched track in the project.
    /// Silently no-ops when no device is connected (`auto_pick_input`
    /// returns `None`) or when midir reports an error (printed to
    /// stderr; the UI still works). K2 will surface device selection
    /// in the UI; K3 makes the routing target dynamic.
    /// Build for a specific project at a specific sample rate, *without*
    /// the playhead polling thread. Used directly by tests (which run
    /// outside the rinch runtime); production callers go through
    /// [`Self::build`].
    pub fn build_from_project_and_rate(project: &Project, sample_rate: u32) -> Self {
        let mut engine = Engine::new(sample_rate, MAX_BLOCK);
        let ConfiguredGraph {
            master,
            routing,
            realized_events,
            wavetable_publishers,
            drum_publishers,
        } = configure_graph(&mut engine, project, sample_rate);
        let initial_event_count = realized_events.len();
        let sample_clock = engine.sample_clock();
        let transport = engine.transport_handle();
        let (audio_engine, handle, midi_input_handle) = engine.split();

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
            tempo_map: Rc::new(RefCell::new(project.tempo_map.clone())),
            transport,
            transport_state: Signal::new(Transport::default()),
            realized_events: Rc::new(RefCell::new(Rc::new(realized_events))),
            project: Rc::new(RefCell::new(Rc::new(project.clone()))),
            wavetable_handles: Rc::new(build_wavetable_handles(&wavetable_publishers)),
            wavetable_publishers: Rc::new(extract_publishers(&wavetable_publishers)),
            drum_handles: Rc::new(build_drum_handles(&drum_publishers)),
            drum_publishers: Rc::new(extract_drum_publishers(&drum_publishers)),
            // No `poll_signal` registrations in the test path —
            // they assert main-thread and unit tests run outside the
            // runtime. `build()` attaches the polls after this fn
            // returns.
            midi_input_handle: Rc::new(RefCell::new(Some(midi_input_handle))),
            _midi_connection: Rc::new(RefCell::new(None)),
            // Seed the routing atomic with the first Pitched track's
            // NodeId so MIDI input has a sensible default target
            // before the user picks a track. K3's
            // `set_midi_target_track` overwrites this when the user
            // selects a different track; the bridge's
            // `Arc::clone(&self.midi_target)` keeps the host and
            // midir-thread views in sync.
            midi_target: Arc::new(AtomicU32::new(
                first_pitched_track_node_id(project).map(|n| n.0).unwrap_or(0),
            )),
            current_midi_device: Signal::new(None),
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
        let events = self.realized_events.borrow().clone();
        let mut handle = self.handle.borrow_mut();
        for ev in events.iter() {
            handle
                .push_event(ev.clone())
                .map_err(|e| format!("event queue overflowed while re-arming: {e:?}"))?;
        }
        Ok(())
    }

    /// Snapshot of the live project as a cheap `Rc<Project>` clone.
    /// Components read this to surface track names, kinds, and
    /// per-track synth assignments. The returned `Rc` is a snapshot
    /// at call time; the edit pump may swap the inner `Rc` from
    /// underneath, so callers that need a stable view across multiple
    /// reads should bind the result to a local.
    pub fn project(&self) -> Rc<Project> {
        self.project.borrow().clone()
    }

    /// Snapshot of the project's tempo map. Used by
    /// [`Self::playhead_position`] for sample → bars/beats
    /// conversion. Kept in sync with [`Self::project`] by the edit
    /// pump.
    pub fn tempo_map(&self) -> TempoMap {
        self.tempo_map.borrow().clone()
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
        let tempo_map = self.tempo_map();
        let mt = tempo_map
            .sample_to_musical(rawdaw_model::SampleTime::samples(samples), self.sample_rate);
        let beats_per_bar = tempo_map.beats_per_bar_at(rawdaw_model::MusicalTime::ZERO);
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

// `configure_graph` + its helpers + `WavetableEditorHandle` live in
// `audio/graph.rs`; unit tests live in `audio/tests.rs` — both
// split out under the workspace 700-line cap.

#[cfg(test)]
mod tests;
