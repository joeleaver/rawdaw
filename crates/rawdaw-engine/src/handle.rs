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

use crate::command::GraphCommand;
use crate::event::BlockEvent;
use crate::node::AudioNode;

pub struct EngineHandle {
    pub(crate) command_tx: Producer<GraphCommand>,
    pub(crate) event_tx: Producer<BlockEvent>,
    pub(crate) garbage_rx: Consumer<Box<dyn AudioNode>>,
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

    /// Enqueue a MIDI event for some target node. Events delivered for a
    /// block's window are partitioned by target and passed to each node's
    /// `process()`. Returns `Err(PushError::Full(event))` on overflow.
    pub fn push_event(&mut self, event: BlockEvent) -> Result<(), PushError<BlockEvent>> {
        self.event_tx.push(event)
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
