//! Host-side handle for an `Engine`.
//!
//! Owns the producer ends of the host → audio queues (graph commands and
//! MIDI events) plus the consumer end of the audio → host garbage return
//! queue. The audio thread never touches an `EngineHandle`; the handle is
//! the user's only sanctioned way to talk to a running engine.
//!
//! After `Engine::split`, an `EngineHandle` typically stays on whatever
//! thread runs the UI / realization pass while the matching `AudioEngine`
//! moves into the cpal callback. Both types are `Send`.

use rtrb::{Consumer, Producer, PushError};

use rawdaw_model::{Midi2Message, SampleTime};

use crate::command::GraphCommand;
use crate::event::{BlockEvent, BlockMessage, ParamEvent};
use crate::graph::NodeId;
use crate::node::AudioNode;

pub struct EngineHandle {
    pub(crate) command_tx: Producer<GraphCommand>,
    pub(crate) event_tx: Producer<BlockEvent>,
    pub(crate) garbage_rx: Consumer<Box<dyn AudioNode>>,
}

/// Host-side handle for pushing MIDI events into the engine *from a
/// different thread than the one that owns [`EngineHandle`]*.
///
/// Owns a second SPSC producer dedicated to MIDI input from external
/// sources (typically a `midir` callback). The audio thread drains
/// both this queue and the [`EngineHandle::event_tx`] queue at the
/// start of every `process_block` and merges them into the per-block
/// event partition.
///
/// Why a second queue and not a shared `Mutex<EngineHandle>`: rtrb
/// SPSC stays single-producer, no lock contention on the audio path,
/// and the host-side push from the UI thread remains independent of
/// any MIDI input thread. See `docs/midi-input-plan.md` K0 design
/// notes for the trade-off.
///
/// `Send + !Sync`. The owning thread is typically `midir`'s callback
/// thread; midir's `MidiInputConnection<T>::close()` returns the
/// captured `T` when the connection closes, so the handle can be
/// recovered and moved into a new connection on device switch (see
/// `crates/rawdaw-app/src/midi_input/` for the production wiring).
pub struct MidiInputHandle {
    pub(crate) event_tx: Producer<BlockEvent>,
}

impl MidiInputHandle {
    /// Enqueue a MIDI message scheduled at `time` aimed at `target`.
    /// Returns `Err(PushError::Full(event))` on overflow.
    ///
    /// External MIDI input typically schedules at `sample_clock + 1`
    /// so the event lands in the next block — see the K0 design
    /// note on sample-accurate scheduling.
    pub fn push_midi(
        &mut self,
        time: SampleTime,
        target: NodeId,
        message: Midi2Message,
    ) -> Result<(), PushError<BlockEvent>> {
        self.event_tx.push(BlockEvent {
            time,
            target,
            message: BlockMessage::Midi(message),
        })
    }
}

impl EngineHandle {
    /// Enqueue a graph mutation. The audio thread applies it at the top
    /// of the next `process_block`.
    ///
    /// Returns `Err(PushError::Full(cmd))` if the command queue is full —
    /// the caller gets the rejected command back and can retry after the
    /// audio thread has consumed.
    pub fn push_command(&mut self, cmd: GraphCommand) -> Result<(), PushError<GraphCommand>> {
        self.command_tx.push(cmd)
    }

    /// Enqueue any [`BlockEvent`]. Events delivered for a block's window
    /// are partitioned by target and passed to each node's `process()`.
    /// Returns `Err(PushError::Full(event))` on overflow.
    ///
    /// Prefer [`Self::push_midi`] / [`Self::push_param`] for the common
    /// cases — they assemble the `BlockEvent` for you. This raw method
    /// stays for translation paths that already have a `BlockEvent` in
    /// hand (e.g. `translate_events`'s output).
    pub fn push_event(&mut self, event: BlockEvent) -> Result<(), PushError<BlockEvent>> {
        self.event_tx.push(event)
    }

    /// Enqueue a MIDI message at `time` aimed at `target`.
    pub fn push_midi(
        &mut self,
        time: SampleTime,
        target: NodeId,
        message: Midi2Message,
    ) -> Result<(), PushError<BlockEvent>> {
        self.push_event(BlockEvent {
            time,
            target,
            message: BlockMessage::Midi(message),
        })
    }

    /// Enqueue a parameter change at `time` aimed at `target`.
    ///
    /// `path` is an opaque, synth-defined 8-byte address — see the
    /// [`ParamEvent`](crate::event::ParamEvent) docs. The engine never
    /// inspects it; the receiving node decodes its own `ParamPath`.
    pub fn push_param(
        &mut self,
        time: SampleTime,
        target: NodeId,
        path: [u8; 8],
        value: f32,
    ) -> Result<(), PushError<BlockEvent>> {
        self.push_event(BlockEvent {
            time,
            target,
            message: BlockMessage::Param(ParamEvent { path, value }),
        })
    }

    /// Drain every removed node sitting in the audio → host garbage queue
    /// so the host can drop them off the audio thread.
    ///
    /// This allocates a fresh `Vec` and is host-side only; never call from
    /// an audio callback.
    pub fn drain_garbage(&mut self) -> Vec<Box<dyn AudioNode>> {
        let mut out = Vec::new();
        while let Ok(node) = self.garbage_rx.pop() {
            out.push(node);
        }
        out
    }
}
