//! Per-block event delivery.
//!
//! Events flow from the host thread into the engine via an SPSC queue.
//! At the top of every `process_block`, the engine drains events whose
//! `time` falls within the upcoming block, partitions them by target
//! `NodeId`, and passes each node its slice in the `process()` call.
//!
//! Within a block, events are sorted by `offset_in_block` ascending.
//!
//! ## Event message variants
//!
//! [`BlockMessage`] carries either a MIDI message or a parameter change.
//! Both variants share the same delivery channel — the engine drains a
//! single SPSC queue, partitions by target node, and passes the
//! `(offset_in_block, message)` slice to `AudioNode::process`. This
//! keeps MIDI and parameter events sample-accurate against each other
//! at the same target.

use rawdaw_model::{Midi2Message, SampleTime};

use crate::graph::NodeId;

/// One message that flows from host to audio thread, addressed at a
/// specific [`NodeId`] at a specific [`SampleTime`].
///
/// MIDI carries note/cc messages aimed at the synth voice manager;
/// `Param` carries an opaque per-synth parameter path + a new value.
/// Each synth crate defines its own `ParamPath` enum and encodes it
/// into the 8-byte `path` array; the engine treats the bytes as
/// opaque and lets the receiving node decode.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockMessage {
    Midi(Midi2Message),
    Param(ParamEvent),
}

impl BlockMessage {
    /// True if this message is a `Param` variant.
    pub fn is_param(&self) -> bool {
        matches!(self, Self::Param(_))
    }

    /// True if this message is a `Midi` variant.
    pub fn is_midi(&self) -> bool {
        matches!(self, Self::Midi(_))
    }
}

/// An opaque per-synth parameter change.
///
/// The 8-byte `path` is encoded by the sender (the host-side UI or
/// automation engine) and decoded by the receiving node. Eight bytes
/// is generous: a typical encoding is one tag byte plus one slot-
/// index byte, leaving six bytes of headroom for future destinations
/// (e.g. matrix-slot fields that need both a slot index and a sub-
/// field selector).
///
/// The engine itself never inspects `path`; it just routes the event
/// to the targeted node, which calls its own decoder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamEvent {
    pub path: [u8; 8],
    pub value: f32,
}

/// An event scheduled at an absolute sample time, targeting a specific node.
///
/// This is the engine's internal representation. The host translates the
/// model layer's `TimedEvent` (which targets a `TrackId`) into a `BlockEvent`
/// by looking up the `NodeId` that backs that track's instrument.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockEvent {
    pub time: SampleTime,
    pub target: NodeId,
    pub message: BlockMessage,
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
#[derive(Debug, Clone, PartialEq)]
pub struct BlockEventInBlock {
    pub offset_in_block: u32,
    pub message: BlockMessage,
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

    fn note_on() -> BlockMessage {
        BlockMessage::Midi(Midi2Message::NoteOn {
            channel: MidiChannel::default(),
            note: MidiNote::new(60).unwrap(),
            velocity: U16Velocity::HALF,
        })
    }

    fn param(path_byte: u8, value: f32) -> BlockMessage {
        let mut path = [0u8; 8];
        path[0] = path_byte;
        BlockMessage::Param(ParamEvent { path, value })
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

    #[test]
    fn midi_and_param_can_share_a_block() {
        // A `Param` event sits next to a `Midi` event at a later offset
        // in the same block. The two variants don't collide, both
        // survive the iterator round-trip, and ordering by
        // `offset_in_block` is preserved (the engine never re-sorts).
        let events = vec![
            BlockEventInBlock {
                offset_in_block: 0,
                message: param(7, 0.5),
            },
            BlockEventInBlock {
                offset_in_block: 32,
                message: note_on(),
            },
        ];
        let block = EventBlock::new(&events);
        let mut iter = block.iter();
        let first = iter.next().unwrap();
        assert_eq!(first.offset_in_block, 0);
        assert!(first.message.is_param());
        let second = iter.next().unwrap();
        assert_eq!(second.offset_in_block, 32);
        assert!(second.message.is_midi());
        assert!(iter.next().is_none());
    }

    #[test]
    fn param_event_carries_path_and_value() {
        let mut path = [0u8; 8];
        path[0] = 3;
        path[1] = 42;
        let p = ParamEvent { path, value: -1.25 };
        assert_eq!(p.path[0], 3);
        assert_eq!(p.path[1], 42);
        assert_eq!(p.value, -1.25);
    }
}
