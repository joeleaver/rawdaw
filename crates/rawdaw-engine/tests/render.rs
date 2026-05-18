//! End-to-end engine tests: build a graph, push events, render offline,
//! assert on output samples.

use std::collections::BTreeMap;

use rawdaw_engine::{
    translate_events, AudioEngine, BlockEvent, Engine, EngineHandle, GraphCommand, ImpulseNode,
    NodeId, QueueCapacities, SilenceNode, SineNode, TrackRouting,
};
use rawdaw_model::*;

const SAMPLE_RATE: u32 = 48_000;
const MAX_BLOCK: usize = 256;

fn note_on() -> Midi2Message {
    Midi2Message::NoteOn {
        channel: MidiChannel::default(),
        note: MidiNote::new(60).unwrap(),
        velocity: U16Velocity::HALF,
    }
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
        message: Midi2Message::NoteOn {
            channel,
            note,
            velocity: U16Velocity::HALF,
        },
    });
    engine.push_event(BlockEvent {
        time: SampleTime::samples(1024),
        target: master,
        message: Midi2Message::NoteOff {
            channel,
            note,
            velocity: U16Velocity::MIN,
        },
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
                message: Midi2Message::NoteOn {
                    channel: MidiChannel::default(),
                    note: MidiNote::new(60).unwrap(),
                    velocity: U16Velocity::HALF,
                },
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
