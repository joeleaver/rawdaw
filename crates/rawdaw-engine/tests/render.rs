//! End-to-end engine tests: build a graph, push events, render offline,
//! assert on output samples.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rawdaw_engine::{
    translate_events, AudioEngine, AudioNode, BlockEvent, BlockMessage, Engine, EngineHandle,
    EventBlock, GraphCommand, ImpulseNode, NodeId, OutputDescriptor, PortAccess, ProcessContext,
    QueueCapacities, SilenceNode, SineNode, TrackRouting,
};
use rawdaw_model::*;

const SAMPLE_RATE: u32 = 48_000;
const MAX_BLOCK: usize = 256;

fn note_on() -> BlockMessage {
    BlockMessage::Midi(Midi2Message::NoteOn {
        channel: MidiChannel::default(),
        note: MidiNote::new(60).unwrap(),
        velocity: U16Velocity::HALF,
    })
}

#[test]
fn rendering_empty_graph_produces_silence() {
    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    // Install a single SilenceNode at id 0 and use it as master.
    let master = NodeId::new(0);
    engine.push_command(GraphCommand::AddNode {
        id: master,
        node: Box::new(SilenceNode::new()),
    });
    let result = engine.render_offline(master, SampleTime::samples(1024), MAX_BLOCK);

    assert_eq!(result.left.len(), 1024);
    assert_eq!(result.right.len(), 1024);
    assert_eq!(result.sample_rate, SAMPLE_RATE);
    assert!(result.left.iter().all(|s| *s == 0.0));
    assert!(result.right.iter().all(|s| *s == 0.0));
}

#[test]
fn impulse_node_writes_one_at_each_note_on() {
    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let master = NodeId::new(0);
    engine.push_command(GraphCommand::AddNode {
        id: master,
        node: Box::new(ImpulseNode::new()),
    });

    // Push two NoteOn events at specific sample offsets.
    engine.push_event(BlockEvent {
        time: SampleTime::samples(0),
        target: master,
        message: note_on(),
    });
    engine.push_event(BlockEvent {
        time: SampleTime::samples(500),
        target: master,
        message: note_on(),
    });

    let result = engine.render_offline(master, SampleTime::samples(1024), MAX_BLOCK);

    // Sample 0 should be 1.0, sample 500 should be 1.0, everything else 0.
    assert_eq!(result.left[0], 1.0);
    assert_eq!(result.right[0], 1.0);
    assert_eq!(result.left[500], 1.0);
    assert_eq!(result.right[500], 1.0);

    let nonzero_left: Vec<usize> = result
        .left
        .iter()
        .enumerate()
        .filter(|(_, s)| **s != 0.0)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        nonzero_left,
        vec![0, 500],
        "exactly two non-zero samples expected on L"
    );
}

#[test]
fn events_outside_block_window_dont_fire() {
    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let master = NodeId::new(0);
    engine.push_command(GraphCommand::AddNode {
        id: master,
        node: Box::new(ImpulseNode::new()),
    });
    // Schedule an event past the render duration.
    engine.push_event(BlockEvent {
        time: SampleTime::samples(2000),
        target: master,
        message: note_on(),
    });
    let result = engine.render_offline(master, SampleTime::samples(1024), MAX_BLOCK);
    assert!(
        result.left.iter().all(|s| *s == 0.0),
        "no events should have fired"
    );
}

#[test]
fn events_dispatch_to_correct_node_only() {
    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let impulse_id = NodeId::new(0);
    let silence_id = NodeId::new(1);
    engine.push_command(GraphCommand::AddNode {
        id: impulse_id,
        node: Box::new(ImpulseNode::new()),
    });
    engine.push_command(GraphCommand::AddNode {
        id: silence_id,
        node: Box::new(SilenceNode::new()),
    });

    // Event targets the silence node (which ignores events).
    engine.push_event(BlockEvent {
        time: SampleTime::samples(100),
        target: silence_id,
        message: note_on(),
    });

    // Render with impulse_id as master. Impulse should see no events, so
    // output stays silent.
    let result = engine.render_offline(impulse_id, SampleTime::samples(512), MAX_BLOCK);
    assert!(
        result.left.iter().all(|s| *s == 0.0),
        "impulse node should not receive events targeted at the silence node"
    );
}

#[test]
fn render_spans_multiple_blocks() {
    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let master = NodeId::new(0);
    engine.push_command(GraphCommand::AddNode {
        id: master,
        node: Box::new(ImpulseNode::new()),
    });
    // Event in the second block (offset 300 from block start 256).
    engine.push_event(BlockEvent {
        time: SampleTime::samples(300),
        target: master,
        message: note_on(),
    });
    let result = engine.render_offline(master, SampleTime::samples(1000), 256);
    assert_eq!(result.left[300], 1.0);
    let nonzero: usize = result.left.iter().filter(|s| **s != 0.0).count();
    assert_eq!(nonzero, 1);
}

#[test]
fn remove_node_routes_to_garbage_for_host_drop() {
    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let id = NodeId::new(0);
    engine.push_command(GraphCommand::AddNode {
        id,
        node: Box::new(SilenceNode::new()),
    });
    // Drain commands by running one short block.
    let result = engine.render_offline(id, SampleTime::samples(64), 64);
    assert_eq!(result.frames(), 64);

    // Remove it; the node should land in garbage on the next process_block.
    engine.push_command(GraphCommand::RemoveNode { id });

    // Install another node so the master is still valid, then process.
    let other = NodeId::new(1);
    engine.push_command(GraphCommand::AddNode {
        id: other,
        node: Box::new(SilenceNode::new()),
    });
    let _ = engine.render_offline(other, SampleTime::samples(64), 64);

    let garbage = engine.drain_garbage();
    assert_eq!(garbage.len(), 1, "removed node should appear in garbage");
}

#[test]
fn sine_renders_audio_during_note_pair_and_silence_after() {
    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let master = NodeId::new(0);
    engine.push_command(GraphCommand::AddNode {
        id: master,
        node: Box::new(SineNode::new()),
    });

    // NoteOn at sample 0, NoteOff at sample 1024. Render 2048 samples.
    let note = MidiNote::new(60).unwrap();
    let channel = MidiChannel::default();
    engine.push_event(BlockEvent {
        time: SampleTime::samples(0),
        target: master,
        message: BlockMessage::Midi(Midi2Message::NoteOn {
            channel,
            note,
            velocity: U16Velocity::HALF,
        }),
    });
    engine.push_event(BlockEvent {
        time: SampleTime::samples(1024),
        target: master,
        message: BlockMessage::Midi(Midi2Message::NoteOff {
            channel,
            note,
            velocity: U16Velocity::MIN,
        }),
    });

    let result = engine.render_offline(master, SampleTime::samples(2048), MAX_BLOCK);

    // Note-active region [0..1024) should carry signal.
    let active_rms = rms(&result.left[..1024]);
    assert!(
        active_rms > 0.01,
        "sine note should produce audible signal, got rms {active_rms}",
    );

    // Note-off region [1024..2048) should be silent.
    let trailing_max =
        result.left[1024..].iter().fold(0.0_f32, |m, s| m.max(s.abs()));
    assert_eq!(
        trailing_max, 0.0,
        "after NoteOff the sine should produce zero samples",
    );
}

#[test]
fn realized_events_drive_sine_through_translator() {
    // Build a minimal Project with one pitched track, one pattern emitting a
    // single Absolute C5 note at the start of one section. Realize → translate
    // → push into engine → verify audio.
    let mut project = Project::new(Scale::major(PitchClass::C));
    let track_id = project.id_allocators.alloc_track();
    let pattern_id = project.id_allocators.alloc_pattern();
    let section_id = project.id_allocators.alloc_section();
    let note_id = project.id_allocators.alloc_note();

    project.tracks.push(Track {
        id: track_id,
        name: "Lead".into(),
        kind: TrackKind::Pitched {
            role: Role::Melodic,
        },
        instrument: InstrumentId::new(0),
        mixer: MixerPlacement::default(),
    });

    let main_variant = VariantId::main();
    let mut variants = BTreeMap::new();
    variants.insert(
        main_variant.clone(),
        vec![PitchedEvent::absolute(
            note_id,
            MusicalTime::beats(0),
            Duration::beats(1),
            U7::clamp(100),
            PitchClass::C,
            Octave(5),
        )],
    );
    project.patterns.insert(
        pattern_id,
        Pattern {
            id: pattern_id,
            name: "single-c".into(),
            default_variant: main_variant.clone(),
            body: PatternBody::Pitched(PitchedPatternBody {
                metadata: PitchedPatternMetadata {
                    length: Duration::bars(1, 4),
                },
                variants,
            }),
        },
    );

    let mut activations = BTreeMap::new();
    activations.insert(
        track_id,
        ActivationEntry {
            id: ActivationEntryId::new(0),
            pattern_ref: Some(pattern_id),
            variant_schedule: Vec::new(),
            realization: RealizationParams::default(),
            per_note_overrides: Vec::new(),
        },
    );
    project.sections.insert(
        section_id,
        Section {
            id: section_id,
            name: "verse".into(),
            base: SectionBody {
                duration_bars: 1,
                scale_override: None,
                chord_loops: Vec::new(),
                activations,
            },
            variants: BTreeMap::new(),
            default_variant: VariantId::base(),
        },
    );
    project.arrangement.sections.push(SectionRef {
        id: project.id_allocators.alloc_section_ref(),
        section: section_id,
        variant: VariantId::base(),
        start: MusicalTime::ZERO,
    });

    let realized = realize(&project, SAMPLE_RATE);
    assert!(
        !realized.is_empty(),
        "fixture project must produce at least one event",
    );

    // Wire that track to a SineNode.
    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let sine_id = NodeId::new(0);
    engine.push_command(GraphCommand::AddNode {
        id: sine_id,
        node: Box::new(SineNode::new()),
    });
    let mut routing = TrackRouting::new();
    routing.insert(track_id, sine_id);
    for ev in translate_events(&realized, &routing).expect("routing covers every event") {
        engine.push_event(ev);
    }

    // The note's NoteOn lands at sample 0; render enough samples to hear it.
    let result = engine.render_offline(sine_id, SampleTime::samples(2048), MAX_BLOCK);
    let active_rms = rms(&result.left[..512]);
    assert!(
        active_rms > 0.01,
        "realized note should reach the sine and produce signal, got rms {active_rms}",
    );
}

fn rms(samples: &[f32]) -> f32 {
    let sumsq: f32 = samples.iter().map(|s| s * s).sum();
    (sumsq / samples.len() as f32).sqrt()
}

#[test]
fn split_engine_round_trip_across_threads() {
    // Construct an Engine, install a SineNode on the audio side, push a
    // note from a separate thread via the handle, then render offline on
    // the audio side. Verifies that AudioEngine + EngineHandle can be
    // moved across threads and the SPSC queues actually carry data
    // between them.
    let engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let (mut audio, handle): (AudioEngine, EngineHandle) = engine.split();
    let sine_id = NodeId::new(0);

    // Host thread pushes commands and events.
    let pusher = std::thread::spawn(move || {
        let mut handle = handle;
        handle
            .push_command(GraphCommand::AddNode {
                id: sine_id,
                node: Box::new(SineNode::new()),
            })
            .expect("command queue had room");
        handle
            .push_event(BlockEvent {
                time: SampleTime::samples(0),
                target: sine_id,
                message: BlockMessage::Midi(Midi2Message::NoteOn {
                    channel: MidiChannel::default(),
                    note: MidiNote::new(60).unwrap(),
                    velocity: U16Velocity::HALF,
                }),
            })
            .expect("event queue had room");
        handle
    });
    let handle = pusher.join().expect("pusher thread");

    let result = audio.render_offline(sine_id, SampleTime::samples(1024), MAX_BLOCK);
    assert!(
        rms(&result.left) > 0.01,
        "sine should produce audio after cross-thread setup",
    );

    // Drop the handle on the main thread; nothing to assert, just
    // verifies it survives the trip.
    drop(handle);
}

#[test]
fn engine_command_queue_overflow_panics() {
    // Construct an engine with a tiny command queue, then exceed it.
    // The Engine convenience wrapper panics on overflow (the explicit
    // EngineHandle returns Result).
    let mut engine = Engine::with_capacities(
        SAMPLE_RATE,
        MAX_BLOCK,
        QueueCapacities {
            commands: 2,
            events: 16,
            garbage: 4,
        },
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for i in 0..16 {
            engine.push_command(GraphCommand::AddNode {
                id: NodeId::new(i),
                node: Box::new(SilenceNode::new()),
            });
        }
    }));
    assert!(
        result.is_err(),
        "pushing past command queue capacity should panic",
    );
}

#[test]
fn engine_handle_returns_pushed_value_on_overflow() {
    // Same scenario but using the EngineHandle directly: overflow
    // returns Err carrying the rejected command.
    let engine = Engine::with_capacities(
        SAMPLE_RATE,
        MAX_BLOCK,
        QueueCapacities {
            commands: 2,
            events: 16,
            garbage: 4,
        },
    );
    let (_audio, mut handle) = engine.split();
    // First two pushes succeed.
    for i in 0..2 {
        handle
            .push_command(GraphCommand::AddNode {
                id: NodeId::new(i),
                node: Box::new(SilenceNode::new()),
            })
            .expect("first two commands fit");
    }
    // Third push overflows. The Err carries the rejected command.
    let err = handle
        .push_command(GraphCommand::AddNode {
            id: NodeId::new(99),
            node: Box::new(SilenceNode::new()),
        })
        .expect_err("third push must overflow");
    match err {
        rawdaw_engine::PushError::Full(GraphCommand::AddNode { id, .. }) => {
            assert_eq!(id, NodeId::new(99));
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn batch_commands_apply_atomically() {
    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let a = NodeId::new(0);
    let b = NodeId::new(1);
    engine.push_command(GraphCommand::Batch(vec![
        GraphCommand::AddNode {
            id: a,
            node: Box::new(SilenceNode::new()),
        },
        GraphCommand::AddNode {
            id: b,
            node: Box::new(SilenceNode::new()),
        },
    ]));
    // First block applies the batch; both nodes installed atomically.
    let _ = engine.render_offline(a, SampleTime::samples(64), 64);
    assert!(engine.graph().has_node(a));
    assert!(engine.graph().has_node(b));
}

#[test]
fn sample_clock_advances_with_render_offline() {
    use std::sync::atomic::Ordering;

    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let master = NodeId::new(0);
    engine.push_command(GraphCommand::AddNode {
        id: master,
        node: Box::new(SilenceNode::new()),
    });

    let clock = engine.sample_clock();
    assert_eq!(clock.load(Ordering::Acquire), 0, "fresh engine reads 0");

    // Render in two batches so we exercise multi-block accumulation.
    let _ = engine.render_offline(master, SampleTime::samples(1000), MAX_BLOCK);
    assert_eq!(
        clock.load(Ordering::Acquire),
        1000,
        "after 1000-sample render the clock points at the next block start",
    );

    let _ = engine.render_offline(master, SampleTime::samples(500), MAX_BLOCK);
    assert_eq!(
        clock.load(Ordering::Acquire),
        500,
        "render_offline restarts absolute_time per call (host-provided ctx); \
         the published clock reflects whatever the caller passed in plus block_size",
    );
}

#[test]
fn sample_clock_handles_are_shared() {
    let engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let a = engine.sample_clock();
    let b = engine.sample_clock();
    // Cloning the Arc — both handles must reference the same atomic.
    a.store(42, std::sync::atomic::Ordering::Release);
    assert_eq!(b.load(std::sync::atomic::Ordering::Acquire), 42);
}

#[test]
fn stopped_transport_silences_output_and_resets_clock() {
    use rawdaw_engine::Transport;
    use std::sync::atomic::Ordering;

    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let master = NodeId::new(0);
    engine.push_command(GraphCommand::AddNode {
        id: master,
        node: Box::new(ImpulseNode::new()),
    });
    // Push events that would otherwise fire on the first block.
    engine.push_event(BlockEvent {
        time: SampleTime::samples(10),
        target: master,
        message: note_on(),
    });

    // Force Stopped, then render — render_offline temporarily flips to
    // Playing for its duration. After it returns, we set Stopped and
    // call process_block via Engine's helper to verify the gate.
    engine.transport_handle().set(Transport::Stopped);
    // Render at Stopped: render_offline forces Playing, so a 256-frame
    // render with the queued NoteOn should produce a one-sample impulse.
    // That's expected and proves render_offline's transport override.
    let result = engine.render_offline(master, SampleTime::samples(256), 256);
    assert_eq!(result.left[10], 1.0, "render_offline forces Playing");

    // Verify the transport state was restored to Stopped (it was the
    // prior state when render_offline started).
    assert_eq!(engine.transport_handle().get(), Transport::Stopped);
    // And the sample_clock was forced back to 0 by the Stopped state's
    // gate on the next would-be process_block — but render_offline
    // already finished, so the clock equals 256 (last published).
    // We exercise the gate directly via process_block instead:
    let mut buf = vec![0.0_f32; 2 * MAX_BLOCK];
    let output = rawdaw_engine::BufferMut::new(&mut buf, 2, 64, MAX_BLOCK);
    let ctx = rawdaw_engine::ProcessContext {
        sample_rate: SAMPLE_RATE,
        block_size: 64,
        absolute_time_samples: 999,
        musical_time: MusicalTime::ZERO,
        bpm: 120.0,
        playing: false,
    };
    engine.process_block(master, output, ctx);
    assert_eq!(
        engine.sample_clock().load(Ordering::Acquire),
        0,
        "Stopped forces sample_clock back to 0",
    );
    assert!(
        buf[..64].iter().all(|s| *s == 0.0),
        "Stopped silences master output",
    );
}

#[test]
fn paused_transport_silences_output_but_holds_clock() {
    use rawdaw_engine::Transport;
    use std::sync::atomic::Ordering;

    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let master = NodeId::new(0);
    engine.push_command(GraphCommand::AddNode {
        id: master,
        node: Box::new(ImpulseNode::new()),
    });

    // Advance the clock to a known value via a Playing render.
    let _ = engine.render_offline(master, SampleTime::samples(500), 250);
    assert_eq!(engine.sample_clock().load(Ordering::Acquire), 500);

    // Flip to Paused. process_block should leave the clock untouched
    // (no Release store) and silence the output. The clock READ should
    // still show 500 — the last published value.
    engine.transport_handle().set(Transport::Paused);
    let mut buf = vec![0.0_f32; 2 * MAX_BLOCK];
    let output = rawdaw_engine::BufferMut::new(&mut buf, 2, 64, MAX_BLOCK);
    let ctx = rawdaw_engine::ProcessContext {
        sample_rate: SAMPLE_RATE,
        block_size: 64,
        absolute_time_samples: 500,
        musical_time: MusicalTime::ZERO,
        bpm: 120.0,
        playing: false,
    };
    engine.process_block(master, output, ctx);
    assert_eq!(
        engine.sample_clock().load(Ordering::Acquire),
        500,
        "Paused doesn't touch the sample_clock",
    );
    assert!(
        buf[..64].iter().all(|s| *s == 0.0),
        "Paused silences master output",
    );
}

#[test]
fn stopped_drains_pending_events() {
    use rawdaw_engine::Transport;

    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let master = NodeId::new(0);
    engine.push_command(GraphCommand::AddNode {
        id: master,
        node: Box::new(ImpulseNode::new()),
    });
    // Queue an event at a far-future timestamp — render_offline runs
    // 256 frames so the event normally stays queued past the render.
    engine.push_event(BlockEvent {
        time: SampleTime::samples(10_000),
        target: master,
        message: note_on(),
    });
    let _ = engine.render_offline(master, SampleTime::samples(256), 256);

    // Now flip to Stopped and process one block; the engine should
    // drain the still-queued event. Verify by re-rendering Playing for
    // 16k frames and asserting no impulses fired (queue is empty).
    engine.transport_handle().set(Transport::Stopped);
    let mut buf = vec![0.0_f32; 2 * MAX_BLOCK];
    let output = rawdaw_engine::BufferMut::new(&mut buf, 2, 64, MAX_BLOCK);
    let ctx = rawdaw_engine::ProcessContext {
        sample_rate: SAMPLE_RATE,
        block_size: 64,
        absolute_time_samples: 0,
        musical_time: MusicalTime::ZERO,
        bpm: 120.0,
        playing: false,
    };
    engine.process_block(master, output, ctx);

    // Engine is back to default Stopped after the explicit set; flip to
    // Playing and render — there should be no impulses anywhere.
    let result = engine.render_offline(master, SampleTime::samples(16_384), 256);
    let impulses: Vec<usize> = result
        .left
        .iter()
        .enumerate()
        .filter(|(_, s)| **s != 0.0)
        .map(|(i, _)| i)
        .collect();
    assert!(
        impulses.is_empty(),
        "Stopped should have drained the event queue; saw impulses at {impulses:?}",
    );
}

// ── U1 parameter event protocol tests ─────────────────────────────────────
//
// The three pins from the U1 plan: (a) Param round-trips through the event
// queue with the same (time, target, path, value); (b) a node that
// silently ignores Param doesn't crash; (c) push_midi and push_param
// preserve push order when their time is equal.
//
// All three use a tiny `RecorderNode` that records every BlockMessage it
// sees into a shared buffer. The recorder ignores Param silently — that's
// the U3+ shape any node opting in to parameter events will eventually
// take, and it lets the same node prove both the round-trip pin and the
// "doesn't crash on unrecognized Param" pin in one fixture.

/// Records every event it receives during `process`. Order preserved.
#[derive(Clone)]
struct RecorderNode {
    received: Arc<Mutex<Vec<(u32, BlockMessage)>>>,
}

impl RecorderNode {
    fn new() -> Self {
        Self {
            received: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl AudioNode for RecorderNode {
    fn process(
        &mut self,
        _ports: &mut PortAccess<'_>,
        events: &EventBlock<'_>,
        _ctx: &ProcessContext,
    ) {
        let mut recv = self.received.lock().expect("recorder mutex");
        for ev in events.iter() {
            recv.push((ev.offset_in_block, ev.message.clone()));
        }
    }

    fn output_descriptors(&self) -> &[OutputDescriptor] {
        const DESCRIPTORS: &[OutputDescriptor] = &[OutputDescriptor {
            name: "main",
            channels: rawdaw_engine::ChannelCount::Stereo,
        }];
        DESCRIPTORS
    }

    fn prepare(&mut self, _sample_rate: u32, _max_block_size: usize) {}
}

#[test]
fn param_event_round_trips_through_engine() {
    // (a) push_param → audio thread → node sees BlockMessage::Param with
    //     the same path bytes and value.
    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let target = NodeId::new(0);
    let recorder = RecorderNode::new();
    let received = Arc::clone(&recorder.received);
    engine.push_command(GraphCommand::AddNode {
        id: target,
        node: Box::new(recorder),
    });

    let path = [7u8, 42, 0, 0, 0, 0, 0, 0];
    let value = -0.625_f32;
    engine.push_param(SampleTime::samples(0), target, path, value);

    let _ = engine.render_offline(target, SampleTime::samples(64), 64);

    let recv = received.lock().expect("recorder mutex");
    assert_eq!(recv.len(), 1, "exactly one event should have been recorded");
    let (offset, message) = &recv[0];
    assert_eq!(*offset, 0);
    let BlockMessage::Param(p) = message else {
        panic!("expected Param variant, got Midi");
    };
    assert_eq!(p.path, path);
    assert_eq!(p.value, value);
}

#[test]
fn unknown_param_path_does_not_crash() {
    // (b) A node that doesn't recognize the param path silently ignores
    //     it. The recorder is a stand-in for any node that opts in to the
    //     parameter event channel; it represents the U3+ steady-state
    //     shape (synth crates' apply_event will silently ignore
    //     unrecognized paths in release builds; debug_assert flags them).
    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let target = NodeId::new(0);
    let recorder = RecorderNode::new();
    engine.push_command(GraphCommand::AddNode {
        id: target,
        node: Box::new(recorder),
    });

    // Bytes that don't correspond to any real synth's encoding.
    engine.push_param(SampleTime::samples(0), target, [0xFF; 8], 999.0);

    // No panic means the queue + dispatch path handled the unknown path
    // gracefully; the render is the load-bearing observation.
    let result = engine.render_offline(target, SampleTime::samples(64), 64);
    assert!(result.left.iter().all(|s| *s == 0.0));
}

#[test]
fn push_midi_and_push_param_interleave_in_push_order() {
    // (c) push_param and push_midi at the same time arrive at the node in
    //     push order. rtrb is FIFO; the engine's per-block partitioner
    //     iterates the queue once and pushes into the per-target Vec in
    //     order, so push order ≡ delivery order at the same target.
    let mut engine = Engine::new(SAMPLE_RATE, MAX_BLOCK);
    let target = NodeId::new(0);
    let recorder = RecorderNode::new();
    let received = Arc::clone(&recorder.received);
    engine.push_command(GraphCommand::AddNode {
        id: target,
        node: Box::new(recorder),
    });

    // All three at sample 0 — interleaved order tests the FIFO contract.
    engine.push_param(SampleTime::samples(0), target, [1; 8], 0.1);
    engine.push_midi(
        SampleTime::samples(0),
        target,
        Midi2Message::NoteOn {
            channel: MidiChannel::default(),
            note: MidiNote::new(60).unwrap(),
            velocity: U16Velocity::HALF,
        },
    );
    engine.push_param(SampleTime::samples(0), target, [2; 8], 0.2);

    let _ = engine.render_offline(target, SampleTime::samples(64), 64);

    let recv = received.lock().expect("recorder mutex");
    assert_eq!(recv.len(), 3);
    assert!(matches!(recv[0].1, BlockMessage::Param(_)));
    assert!(matches!(recv[1].1, BlockMessage::Midi(_)));
    assert!(matches!(recv[2].1, BlockMessage::Param(_)));
    if let BlockMessage::Param(p) = &recv[0].1 {
        assert_eq!(p.path[0], 1);
    }
    if let BlockMessage::Param(p) = &recv[2].1 {
        assert_eq!(p.path[0], 2);
    }
}
