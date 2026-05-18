//! Translate model-layer realized events into engine-layer block events.
//!
//! The realization pass in `rawdaw-model` emits `TimedEvent`s that target
//! `TrackId`s. The engine works in `NodeId`s. This module bridges the two:
//! given a routing table mapping each `TrackId` to the `NodeId` of the
//! instrument node that should receive its events, it rewrites every
//! `TimedEvent` into a `BlockEvent`.
//!
//! This is **host-side glue**, not part of the audio path. It runs once
//! per realization, on whatever thread owns the model/UI. The engine
//! never sees `TrackId` directly, and the model never sees `NodeId`.
//!
//! When `rawdaw-app` lands, this module may move out of the engine
//! crate. It lives here for now because the engine crate already depends
//! on `rawdaw-model` (for `Midi2Message`, `SampleTime`, etc.) and there
//! is no host crate yet.

use std::collections::BTreeMap;

use rawdaw_model::{TimedEvent, TrackId};

use crate::event::{BlockEvent, BlockMessage};
use crate::graph::NodeId;

/// Map from each model-layer track to the engine-layer instrument node
/// that should receive its events.
///
/// The host owns this table and updates it as instruments are added,
/// removed, or reassigned. `BTreeMap` (not `HashMap`) for the same
/// reason the rest of the project does: deterministic iteration when
/// the table needs to be inspected or serialized.
pub type TrackRouting = BTreeMap<TrackId, NodeId>;

/// Error returned when realization produced events for a track that
/// isn't in the routing table.
///
/// The engine treats this as a programming error rather than a silent
/// drop — the host is expected to provide a complete routing before
/// translation. If you genuinely want lossy translation, filter the
/// `TimedEvent` stream before calling `translate_events`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslateError {
    /// The named track has no entry in the routing table.
    UnroutedTrack(TrackId),
}

impl core::fmt::Display for TranslateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnroutedTrack(t) => write!(
                f,
                "no engine NodeId routed for TrackId({})",
                t.get(),
            ),
        }
    }
}

impl std::error::Error for TranslateError {}

/// Translate every `TimedEvent` to a `BlockEvent` using `routing`.
///
/// On success the output preserves the input order, including any
/// existing time-sort. The function does not re-sort.
///
/// Returns `Err(TranslateError::UnroutedTrack(_))` on the first event
/// whose target track is missing from the routing.
pub fn translate_events(
    events: &[TimedEvent],
    routing: &TrackRouting,
) -> Result<Vec<BlockEvent>, TranslateError> {
    let mut out = Vec::with_capacity(events.len());
    for ev in events {
        let target = routing
            .get(&ev.target)
            .copied()
            .ok_or(TranslateError::UnroutedTrack(ev.target))?;
        out.push(BlockEvent {
            time: ev.time,
            target,
            message: BlockMessage::Midi(ev.message.clone()),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawdaw_model::{
        Midi2Message, MidiChannel, MidiNote, NoteId, PatternId, Provenance, SampleTime,
        SectionRefId, U16Velocity, VariantId,
    };

    fn provenance() -> Provenance {
        Provenance {
            pattern: PatternId::new(1),
            variant: VariantId::main(),
            event_note_id: NoteId::new(1),
            override_applied: None,
            section: SectionRefId::new(1),
        }
    }

    fn note_on(track: TrackId, time: u64, n: u8) -> TimedEvent {
        TimedEvent {
            time: SampleTime::samples(time),
            target: track,
            message: Midi2Message::NoteOn {
                channel: MidiChannel::default(),
                note: MidiNote::new(n).unwrap(),
                velocity: U16Velocity::HALF,
            },
            provenance: provenance(),
        }
    }

    #[test]
    fn translates_single_event() {
        let track = TrackId::new(42);
        let node = NodeId::new(7);
        let mut routing = TrackRouting::new();
        routing.insert(track, node);

        let events = vec![note_on(track, 100, 60)];
        let out = translate_events(&events, &routing).unwrap();

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].time, SampleTime::samples(100));
        assert_eq!(out[0].target, node);
        assert!(matches!(
            out[0].message,
            BlockMessage::Midi(Midi2Message::NoteOn { .. })
        ));
    }

    #[test]
    fn translates_multi_track_events_with_correct_node_ids() {
        let track_a = TrackId::new(1);
        let track_b = TrackId::new(2);
        let node_a = NodeId::new(10);
        let node_b = NodeId::new(20);
        let mut routing = TrackRouting::new();
        routing.insert(track_a, node_a);
        routing.insert(track_b, node_b);

        let events = vec![
            note_on(track_a, 0, 60),
            note_on(track_b, 100, 64),
            note_on(track_a, 200, 67),
        ];
        let out = translate_events(&events, &routing).unwrap();

        let targets: Vec<NodeId> = out.iter().map(|e| e.target).collect();
        assert_eq!(targets, vec![node_a, node_b, node_a]);
    }

    #[test]
    fn unrouted_track_returns_error() {
        let routed = TrackId::new(1);
        let unrouted = TrackId::new(99);
        let mut routing = TrackRouting::new();
        routing.insert(routed, NodeId::new(0));

        let events = vec![note_on(routed, 0, 60), note_on(unrouted, 100, 64)];
        let err = translate_events(&events, &routing).unwrap_err();
        assert_eq!(err, TranslateError::UnroutedTrack(unrouted));
    }

    #[test]
    fn empty_event_stream_yields_empty_output() {
        let routing = TrackRouting::new();
        let out = translate_events(&[], &routing).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn input_order_is_preserved() {
        let track = TrackId::new(1);
        let mut routing = TrackRouting::new();
        routing.insert(track, NodeId::new(0));

        let events = vec![
            note_on(track, 500, 60),
            note_on(track, 100, 64),
            note_on(track, 200, 67),
        ];
        let out = translate_events(&events, &routing).unwrap();
        let times: Vec<u64> = out.iter().map(|e| e.time.as_samples()).collect();
        assert_eq!(times, vec![500, 100, 200], "translator must not re-sort");
    }
}
