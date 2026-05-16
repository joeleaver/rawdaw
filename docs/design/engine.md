# Audio Engine

The engine consumes the model layer's output (sample-timed MIDI 2.0 events from
the realization pass) and produces audio samples. It owns a DAG of audio nodes,
processes them in topological order each block, and writes the master output
into a sink — either an offline buffer (for tests and bouncing) or a real-time
audio backend (cpal).

Scope of this doc: everything inside the engine crate, plus its boundaries with
the model layer above and audio backends below.

## Layering

```
                    rawdaw-model
                  (Project, realize)
                          │
                          │ Vec<TimedEvent>   (and graph commands)
                          ▼
                    rawdaw-engine
            (Graph, AudioNode, processing)
                          │
                          │ stereo f32 samples
                          ▼
              ┌───────────┴───────────┐
              │                       │
        offline sink             cpal driver
       (Vec<f32> buffer)      (rt audio callback)
```

The engine is **agnostic to its driver**. The same engine core renders offline
for tests and runs in a cpal callback for playback. cpal is a deployment
detail; it does not appear in the engine's core types.

## Threading model (v1)

Two threads:

- **Host thread (non-RT).** Runs the realization pass, builds graph mutations,
  pushes events and commands into the engine's queues. May allocate, lock, and
  syscall freely.
- **Audio thread (RT).** Calls `Engine::process_block()`. Drains the command
  and event queues at the top of each block; runs every node in topo order;
  writes to the output sink. Forbidden: allocation, locks, syscalls, I/O.

Communication is **two single-producer-single-consumer lock-free queues**, both
host → audio:

1. **Command queue** — `GraphCommand` enum. Adds/removes nodes, connects/
   disconnects edges, sets parameters. Applied at the top of each block.
2. **Event queue** — `BlockEvent` items (effectively `TimedEvent`s with the
   engine-internal `NodeId` resolved). Drained per block; partitioned by
   target node; passed to each node in its `process()`.

No worker pool, no parallel scheduling across the DAG. Serial topo-order
processing on a single audio thread. Parallel scheduling is a future
optimization, not v1.

The audio thread does not push anything back to the host yet. Observability
(playhead position, peak meters, node-internal state) goes through atomic
state cells in a later iteration; v1 has none.

## `AudioNode` trait

```rust
pub trait AudioNode: Send {
    /// One block of audio processing.
    ///
    /// `inputs` and `outputs` are pre-sized: `inputs.len()` matches
    /// `input_descriptors().len()`, ditto outputs. Each buffer has channels
    /// matching its descriptor's channel count. All buffers are `block_size`
    /// frames long.
    ///
    /// `events` contains only events targeted at this node, sorted by
    /// `offset_in_block` ascending.
    ///
    /// MUST be RT-safe: no allocation, no locks, no syscalls.
    fn process(
        &mut self,
        inputs: &[BufferRef<'_>],
        outputs: &mut [BufferMut<'_>],
        events: &EventBlock<'_>,
        ctx: &ProcessContext,
    );

    /// What audio inputs this node accepts. Default: none (a source).
    fn input_descriptors(&self) -> &[InputDescriptor] {
        &[]
    }

    /// What audio outputs this node produces. Must return the same slice
    /// every call; the graph caches this.
    fn output_descriptors(&self) -> &[OutputDescriptor];

    /// Sample latency the node adds. Used for PDC (deferred to a later
    /// iteration). Default: zero.
    fn latency_samples(&self) -> usize {
        0
    }

    /// Called once when the node is added to a running graph, on the
    /// non-RT thread. Allows the node to allocate its internal state based
    /// on the actual sample rate and max block size.
    fn prepare(&mut self, sample_rate: u32, max_block_size: usize);
}

pub struct OutputDescriptor {
    pub name: &'static str,        // e.g. "main", "kick", "snare", "hats"
    pub channels: ChannelCount,
}

pub struct InputDescriptor {
    pub name: &'static str,
    pub channels: ChannelCount,
}

pub enum ChannelCount {
    Mono,
    Stereo,
}
```

v1 supports mono and stereo only. Surround / Atmos is not on the roadmap.

## Buffers

A buffer is a fixed-length f32 sample array per channel, exposed as a typed
wrapper:

```rust
pub struct BufferRef<'a> {
    channels: &'a [&'a [f32]],     // outer slice indexed by channel
}

pub struct BufferMut<'a> {
    channels: &'a mut [&'a mut [f32]],
}
```

Channel layout is per-channel slices (planar), not interleaved. Mono =
one slice; stereo = two slices (L, R).

**Ownership rule:** each node owns its output buffers. The graph allocates
them at the time the node is added (sized from `output_descriptors()` plus
the host's `max_block_size`). Upstream node's output buffers are passed to
downstream nodes as `BufferRef<'a>` in `process()`.

The engine guarantees buffer disjointness within a single `process()` call:
input slices and output slices never alias. This is enforced by processing
nodes in topo order and by the graph's edge structure (no cycles).

**Sizing rule:** `prepare()` is called once at graph-build time with
`max_block_size`. Buffers are allocated for that size. The actual block
size at process time may be less (handled by passing `block_size <=
max_block_size` in `ProcessContext`). Buffers are never resized at
process time.

## `ProcessContext`

```rust
pub struct ProcessContext {
    pub sample_rate: u32,
    pub block_size: usize,           // <= max_block_size
    pub absolute_time_samples: u64,  // sample index of this block's first frame
    pub musical_time: MusicalTime,   // musical position at block start
    pub bpm: f64,                    // current tempo
    pub playing: bool,
}
```

Each node sees the same `ProcessContext` per block. Per-event musical-time
information is derived from `offset_in_block` and the block's musical
boundaries by the node itself if needed.

## Graph

```rust
pub struct Graph {
    nodes: Vec<NodeSlot>,            // indexed by NodeId
    edges: Vec<Edge>,
    topo_order: Vec<NodeId>,         // recomputed when graph mutates
    sample_rate: u32,
    max_block_size: usize,
}

pub struct NodeId(u32);

struct NodeSlot {
    node: Box<dyn AudioNode>,
    output_buffers: Vec<Vec<Vec<f32>>>,  // [output_index][channel_index][sample]
    input_descriptors: &'static [InputDescriptor],
    output_descriptors: &'static [OutputDescriptor],
}

pub struct Edge {
    from: NodePort,    // (NodeId, output_index)
    to: NodePort,      // (NodeId, input_index)
}

pub struct NodePort {
    pub node: NodeId,
    pub port: u8,
}
```

`NodeId` is a dense `u32` index; `Graph` reuses indices when nodes are
removed (via a free-list). When a `GraphCommand::AddNode` is applied, the
engine assigns the next available `NodeId` and returns it... wait, the
audio thread can't return values to the host. Either:

- The host allocates the `NodeId` from a shared counter before pushing the
  command (simpler; the audio thread just installs the node at that index).
- The host pushes the command and waits for the audio thread to acknowledge
  (more complex; requires a return channel).

v1 uses the first approach: the host owns `NodeId` allocation and is
responsible for not reusing an ID until the audio thread has applied a
`RemoveNode` command. (This is the standard pattern; JUCE does it similarly.)

### Topological sort

After any structural mutation (add/remove node, connect/disconnect edge),
the `topo_order` is dirty and must be recomputed before the next
`process_block`. Recomputation is a Kahn's-algorithm DFS over `edges`; it
runs once per mutation batch, not per block.

The audio thread is responsible for keeping `topo_order` fresh after applying
the per-block command batch. A mutation that introduces a cycle is a
programmer error and is treated as a panic in debug, ignored (last applied
command rejected) in release.

## Graph mutation: command queue

```rust
pub enum GraphCommand {
    AddNode { id: NodeId, node: Box<dyn AudioNode> },
    RemoveNode { id: NodeId },
    Connect { edge: Edge },
    Disconnect { edge: Edge },
    SetParam { node: NodeId, key: ParamKey, value: ParamValue },
    Batch(Vec<GraphCommand>),       // applied atomically as a group
}
```

Commands flow host → audio via an SPSC ring buffer (`rtrb` or `ringbuf` —
choice deferred to implementation). The audio thread drains commands at the
top of every `process_block` and applies them in order. After applying, it
checks whether the topo order is dirty and recomputes if needed.

`Batch` exists for multi-step changes that must apply together (e.g., add a
new node, connect its output to an existing channel, disconnect the old
source — all between two audio blocks so the user hears no glitch). The
audio thread applies a Batch's contents as a single unit.

**Boxed nodes:** `Box<dyn AudioNode>` in a command transfers ownership from
host to audio thread. Allocation happens on the host side (RT-safe); the
audio thread takes the box without allocating. When `RemoveNode` is
applied, the audio thread cannot drop the box (drop may free, which is
non-RT-safe). Instead, removed nodes are moved to a `garbage` queue going
audio → host; the host drops them on its own time. (This requires a return
channel — but only for memory reclamation, not for control flow. Worth
having; uses `basedrop` or a hand-rolled SPSC return queue.)

## Event delivery

Events flow host → audio via a second SPSC ring buffer:

```rust
pub struct BlockEvent {
    pub time: SampleTime,            // absolute, in engine clock
    pub target: NodeId,
    pub message: Midi2Message,
}
```

Each block:

1. Audio thread drains all events with `time < block_end_sample` from the
   queue.
2. Partitions them by `target` NodeId.
3. For each node in topo order, passes its partition as an `EventBlock<'_>`
   in the `process()` call.

`EventBlock<'_>` is a borrowed slice. The partition itself is a scratch
buffer owned by the engine, sized at `prepare()` (e.g., 1024 events per
block max). Overflow is dropped with a warning flag the host can read.

## Mapping model events to engine events

The realization pass produces `TimedEvent`s targeting model-layer `TrackId`s.
The engine works in `NodeId`s. A translation layer maps `TrackId` →
`NodeId` for the instrument node assigned to that track.

This translation happens host-side, when events flow from `realize()` into
the event queue. The engine never sees `TrackId` directly; it deals only
in `NodeId`. The model layer never sees `NodeId`; it deals only in `TrackId`.

## Offline driver

```rust
impl Engine {
    /// Run the engine offline for `duration` of musical time. Returns
    /// interleaved stereo samples (left, right, left, right, ...).
    pub fn render_offline(
        &mut self,
        master_node: NodeId,
        duration: SampleTime,
        block_size: usize,
    ) -> Vec<f32>;
}
```

The offline driver is the engine's primary test surface. It loops:

1. Compute the next block boundary.
2. Drain commands + events scheduled for this block (already pushed by the host).
3. Run `process_block()` on the graph.
4. Read the master node's output buffer and append to the result.
5. Advance.

No audio device, no real-time deadline. Suitable for unit tests,
correctness checks, and bouncing to file (which is just `render_offline()`
+ WAV-write).

## cpal driver (iteration 3, sketched)

A separate module (or even a separate crate, if cpal's deps get heavy):

```rust
pub struct CpalDriver { /* cpal stream + engine handle */ }

impl CpalDriver {
    pub fn new(engine: Arc<Mutex<Engine>>) -> Result<Self, ...>;
    pub fn start(&mut self) -> Result<(), ...>;
    pub fn stop(&mut self) -> Result<(), ...>;
}
```

The cpal callback closes over a handle to the engine and calls
`engine.process_block(cpal_output_buffer, sample_rate, block_size)`. The
engine's `process_block` is the same one the offline driver uses — the
only difference is what writes the master output: cpal's buffer here, a
`Vec<f32>` there.

cpal is hidden behind a cargo feature (`cpal-driver`) so the engine crate
can be built and tested on systems without audio hardware.

## Crate layout

```
crates/rawdaw-engine/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── node.rs              // AudioNode trait, descriptors
│   ├── buffer.rs            // BufferRef, BufferMut, channel layout
│   ├── event.rs             // BlockEvent, EventBlock
│   ├── context.rs           // ProcessContext
│   ├── graph.rs             // Graph, NodeId, Edge, topology
│   ├── command.rs           // GraphCommand + apply
│   ├── engine.rs            // Engine + render_offline
│   └── nodes/
│       ├── mod.rs
│       ├── silence.rs       // SilenceNode: always emits zeros
│       └── impulse.rs       // ImpulseNode: emits 1.0 sample on NoteOn
└── tests/
    └── render.rs            // Build a graph offline, verify samples
```

Iteration 1 ships everything up to and including offline rendering of test
nodes. Iteration 2 adds a real synth (sine oscillator). Iteration 3 adds
the cpal driver.

## Test strategy

- **Unit tests per module:** topo sort correctness, command application,
  buffer disjointness assertions, event partitioning.
- **Integration tests in `tests/render.rs`:** build a tiny graph, push
  commands, push events, run `render_offline`, assert on output samples.
- **RT-safety lint:** wrap `process()` in `assert_no_alloc` in debug builds.
  Allocations inside `process()` produce a debug-assertion failure.

## Open questions

- **Choice of SPSC queue crate.** `rtrb` vs `ringbuf` vs hand-rolled. Decide
  when implementing the queues; the API is small so swapping is cheap.
- **Boxed-node return channel.** Memory reclamation for removed nodes needs
  a path back to the host. Hand-roll a simple SPSC, or use `basedrop`?
  Defer until `RemoveNode` is actually implemented.
- **Master node selection.** The engine needs to know which node is the
  output. v1: explicit `master_node: NodeId` parameter to `render_offline`.
  Later: a host-managed master/aux/return-bus structure.
- **Parameter format.** `ParamKey` and `ParamValue` are stubs in this doc.
  Each node implementation has its own param vocabulary. Defer the cross-
  cutting design until we have at least two node types.
- **Multi-output instrument routing.** Each output of a multi-out node
  becomes a separate input edge to downstream nodes. The graph supports
  this naturally; the question is just UI / convention. Punt for v1 —
  Iteration 2's synth is single-output.
