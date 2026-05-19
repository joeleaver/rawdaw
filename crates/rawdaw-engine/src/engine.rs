//! `Engine`: the convenience wrapper that bundles an [`AudioEngine`] and
//! an [`EngineHandle`] together.
//!
//! For single-thread use (offline rendering, unit tests) the `Engine`
//! exposes the legacy combined API: `push_command`, `push_event`,
//! `process_block`, `render_offline`, `drain_garbage`, `graph`. These
//! delegate to the appropriate half so callers don't have to deal with
//! two types.
//!
//! For multi-thread use (cpal playback) call `engine.split()` to consume
//! the bundle and get the two halves separately. Move the
//! [`AudioEngine`] into the audio thread / cpal callback; keep the
//! [`EngineHandle`] on the host thread to push commands and events.
//!
//! ## Queue capacities
//!
//! The SPSC ring buffers are sized at construction. Defaults are chosen
//! to comfortably handle the v1 use cases (a fully-realized few-minute
//! song fits in `EVENT_QUEUE_CAP`); use `with_capacities` to override
//! when offline-rendering very long projects or constraining memory.
//!
//! Push operations return `Result` so callers can react to overflow
//! deterministically instead of silently dropping; an overflow is
//! always a host-side bug or capacity-sizing miscalculation.

use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use rtrb::RingBuffer;

use rawdaw_model::{Midi2Message, SampleTime};

use crate::audio_engine::{AudioEngine, RenderResult};
use crate::buffer::BufferMut;
use crate::command::GraphCommand;
use crate::context::ProcessContext;
use crate::event::BlockEvent;
use crate::graph::{Graph, NodeId};
use crate::handle::{EngineHandle, MidiInputHandle};
use crate::node::AudioNode;
use crate::transport::TransportHandle;

/// Default command queue capacity. Graph mutations are infrequent;
/// 1024 covers very chunky batch reconfigurations.
pub const DEFAULT_COMMAND_QUEUE_CAPACITY: usize = 1024;

/// Default event queue capacity. 16,384 events fits the realized output
/// of a multi-minute song with dense MIDI; the engine returns a
/// `PushError` if the host outruns the audio thread, surfacing the
/// problem early instead of silently dropping.
pub const DEFAULT_EVENT_QUEUE_CAPACITY: usize = 16_384;

/// Default audio → host garbage capacity. Removed nodes flow through
/// this so the host can drop them off the audio thread; 256 is far
/// beyond realistic peak.
pub const DEFAULT_GARBAGE_QUEUE_CAPACITY: usize = 256;

/// Tunable queue capacities. `Default` returns the constants above.
#[derive(Debug, Clone, Copy)]
pub struct QueueCapacities {
    pub commands: usize,
    pub events: usize,
    pub garbage: usize,
}

impl Default for QueueCapacities {
    fn default() -> Self {
        Self {
            commands: DEFAULT_COMMAND_QUEUE_CAPACITY,
            events: DEFAULT_EVENT_QUEUE_CAPACITY,
            garbage: DEFAULT_GARBAGE_QUEUE_CAPACITY,
        }
    }
}

/// An audio engine plus its host-side handle, bundled for single-thread
/// use. Call [`Engine::split`] to break them apart for multi-threaded
/// playback.
pub struct Engine {
    audio: AudioEngine,
    host: EngineHandle,
    /// MIDI input handle. Owned here pre-split so [`Engine::split`]
    /// can hand it out to the MIDI input thread (typically a `midir`
    /// callback). Single-thread callers can leave it unused.
    midi_input: MidiInputHandle,
}

impl Engine {
    pub fn new(sample_rate: u32, max_block_size: usize) -> Self {
        Self::with_capacities(sample_rate, max_block_size, QueueCapacities::default())
    }

    pub fn with_capacities(
        sample_rate: u32,
        max_block_size: usize,
        caps: QueueCapacities,
    ) -> Self {
        let (cmd_tx, cmd_rx) = RingBuffer::<GraphCommand>::new(caps.commands);
        let (ev_tx, ev_rx) = RingBuffer::<BlockEvent>::new(caps.events);
        let (midi_ev_tx, midi_ev_rx) = RingBuffer::<BlockEvent>::new(caps.events);
        let (host_ev_tx, host_ev_rx) = RingBuffer::<BlockEvent>::new(caps.events);
        let (gar_tx, gar_rx) = RingBuffer::<Box<dyn AudioNode>>::new(caps.garbage);

        let audio = AudioEngine::new(
            sample_rate,
            max_block_size,
            cmd_rx,
            ev_rx,
            midi_ev_rx,
            host_ev_rx,
            gar_tx,
        );
        let host = EngineHandle {
            command_tx: cmd_tx,
            event_tx: ev_tx,
            host_event_tx: host_ev_tx,
            garbage_rx: gar_rx,
        };
        let midi_input = MidiInputHandle {
            event_tx: midi_ev_tx,
        };
        Self {
            audio,
            host,
            midi_input,
        }
    }

    /// Consume the bundle and return the three halves separately:
    /// the [`AudioEngine`] for the audio thread, the [`EngineHandle`]
    /// for the host thread, and the [`MidiInputHandle`] for whichever
    /// thread will be pushing external MIDI input (typically `midir`'s
    /// callback thread).
    ///
    /// All three are independent; a host that doesn't need live MIDI
    /// can simply drop the [`MidiInputHandle`] and the engine still
    /// works (the audio thread harmlessly drains an empty queue every
    /// block).
    pub fn split(self) -> (AudioEngine, EngineHandle, MidiInputHandle) {
        (self.audio, self.host, self.midi_input)
    }

    // ---------- Convenience delegators (single-thread use) ----------
    //
    // These panic on queue overflow rather than returning `Result`. A
    // single-thread caller can't usefully react to overflow at the call
    // site — they need to resize the queue at construction. For
    // multi-thread use where the host might genuinely want to handle
    // backpressure, call `split()` and use the `EngineHandle` directly;
    // its push methods return `Result<(), PushError<T>>`.

    pub fn push_command(&mut self, cmd: GraphCommand) {
        self.host
            .push_command(cmd)
            .expect("engine command queue overflowed; increase QueueCapacities.commands");
    }

    pub fn push_event(&mut self, event: BlockEvent) {
        self.host
            .push_event(event)
            .expect("engine event queue overflowed; increase QueueCapacities.events");
    }

    /// Single-thread convenience for [`EngineHandle::push_midi`].
    pub fn push_midi(&mut self, time: SampleTime, target: NodeId, message: Midi2Message) {
        self.host
            .push_midi(time, target, message)
            .expect("engine event queue overflowed; increase QueueCapacities.events");
    }

    /// Single-thread convenience for [`EngineHandle::push_param`].
    pub fn push_param(&mut self, time: SampleTime, target: NodeId, path: [u8; 8], value: f32) {
        self.host
            .push_param(time, target, path, value)
            .expect("engine event queue overflowed; increase QueueCapacities.events");
    }

    pub fn drain_garbage(&mut self) -> Vec<Box<dyn AudioNode>> {
        self.host.drain_garbage()
    }

    pub fn process_block(
        &mut self,
        master: NodeId,
        output: BufferMut<'_>,
        ctx: ProcessContext,
    ) {
        self.audio.process_block(master, output, ctx);
    }

    pub fn render_offline(
        &mut self,
        master: NodeId,
        duration_samples: SampleTime,
        block_size: usize,
    ) -> RenderResult {
        self.audio.render_offline(master, duration_samples, block_size)
    }

    pub fn graph(&self) -> &Graph {
        self.audio.graph()
    }

    /// Clone the shared sample-clock handle so the host can observe the
    /// audio thread's transport position. Safe to call before or after
    /// [`Self::split`]; the same atomic backs both halves.
    pub fn sample_clock(&self) -> Arc<AtomicU64> {
        self.audio.sample_clock()
    }

    /// Clone the shared transport handle so the host can drive
    /// Playing / Paused / Stopped transitions. Safe to call before or
    /// after [`Self::split`]; the same atomic backs both halves.
    pub fn transport_handle(&self) -> TransportHandle {
        self.audio.transport_handle()
    }
}
