//! Per-block event delivery.
//!
//! Events flow from the host thread into the engine via an SPSC queue.
//! At the top of every `process_block`, the engine drains events whose
//! `time` falls within the upcoming block, partitions them by target
//! `NodeId`, and passes each node its slice in the `process()` call.
//!
//! Within a block, events are sorted by `offset_in_block` ascending.

use rawdaw_model::{Midi2Message, SampleTime};

use crate::graph::NodeId;

/// An event scheduled at an absolute sample time, targeting a specific node.
///
/// This is the engine's internal representation. The host translates the
/// model layer's `TimedEvent` (which targets a `TrackId`) into a `BlockEvent`
/// by looking up the `NodeId` that backs that track's instrument.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockEvent {
    pub time: SampleTime,
    pub target: NodeId,
    pub message: Midi2Message,
}

/// The slice of events visible to one node during one `process()` call.
///
/// All events here target the same node; `offset_in_block` is the sample
/// offset from the start of the current block (`0..block_size`), already
/// adjusted from the absolute `BlockEvent::time`.
pub struct EventBlock<'a> {
    events: &'a [BlockEventInBlock],
}

/// An event already converted to block-relative time. Built by the engine
/// per block; nodes only see this form (not raw `BlockEvent`s).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockEventInBlock {
    pub offset_in_block: u32,
    pub message: Midi2Message,
}

impl<'a> EventBlock<'a> {
    pub fn new(events: &'a [BlockEventInBlock]) -> Self {
        Self { events }
    }

    /// An empty event block (no events for this node this block).
    pub const fn empty() -> Self {
        Self { events: &[] }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &BlockEventInBlock> {
        self.events.iter()
    }

    /// Borrow the underlying slice. Useful for nodes that need indexed access
    /// to walk events alongside per-sample DSP without allocating an iterator
    /// state on the stack — e.g. the sine voice manager.
    pub fn as_slice(&self) -> &[BlockEventInBlock] {
        self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_model::{MidiChannel, MidiNote, U16Velocity};

    fn note_on() -> Midi2Message {
        Midi2Message::NoteOn {
            channel: MidiChannel::default(),
            note: MidiNote::new(60).unwrap(),
            velocity: U16Velocity::HALF,
        }
    }

    #[test]
    fn event_block_iterates_in_order() {
        let events = vec![
            BlockEventInBlock {
                offset_in_block: 0,
                message: note_on(),
            },
            BlockEventInBlock {
                offset_in_block: 64,
                message: note_on(),
            },
        ];
        let block = EventBlock::new(&events);
        let offsets: Vec<u32> = block.iter().map(|e| e.offset_in_block).collect();
        assert_eq!(offsets, vec![0, 64]);
    }

    #[test]
    fn empty_block_is_empty() {
        let block = EventBlock::empty();
        assert!(block.is_empty());
        assert_eq!(block.len(), 0);
    }
}
