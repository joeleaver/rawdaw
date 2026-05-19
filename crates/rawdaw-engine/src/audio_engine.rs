//! The audio side of the engine: drives the graph through blocks, drains
//! the host → audio command and event queues, runs each node in topo
//! order, and writes the master output.
//!
//! `AudioEngine` is the type the cpal callback owns. It exposes
//! `process_block` (the real-time entry point) and `render_offline` (the
//! same loop minus the driver, used as the engine's primary test surface).
//!
//! All queue access is lock-free: commands/events come in over rtrb SPSC
//! ring buffers; the audio side never blocks, never allocates per block
//! (input scratch + event partition Vecs are reused across blocks), and
//! never frees memory (removed nodes go out through the garbage SPSC for
//! the host to drop).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use rtrb::{Consumer, Producer};

use rawdaw_model::{MusicalTime, SampleTime};

use crate::buffer::BufferMut;
use crate::command::{apply_command, GraphCommand};
use crate::context::ProcessContext;
use crate::event::{BlockEvent, BlockEventInBlock, EventBlock};
use crate::graph::{Graph, NodeId};
use crate::node::{AudioNode, PortAccess};
use crate::transport::{Transport, TransportHandle};

/// Engine-side maximum input ports per node. Used to size the input
/// scratch once at construction so it doesn't reallocate during
/// processing.
///
/// Realistic node graphs in rawdaw shouldn't exceed this for a long time
/// (a complex mixer node might want a dozen sidechains; we leave headroom).
const MAX_INPUT_PORTS_PER_NODE: usize = 16;

/// The audio side of the engine.
///
/// Owns the graph, the command/event consumer ends, and the garbage
/// producer end. Designed to live on the audio thread for the cpal
/// driver case; also usable from a single thread via `render_offline`.
pub struct AudioEngine {
    graph: Graph,
    command_rx: Consumer<GraphCommand>,
    event_rx: Consumer<BlockEvent>,
    /// Second SPSC consumer dedicated to MIDI input from external
    /// sources (`midir` callbacks, etc.). Drained in lockstep with
    /// `event_rx` at the start of every `process_block`; merged into
    /// the same event_partition so MIDI events compete fairly with
    /// host-pushed events at the same target / time.
    ///
    /// Stays separate at the queue level (not a Mutex on a single
    /// queue) so the MIDI input thread is an independent SPSC
    /// producer. See `MidiInputHandle` docs.
    midi_event_rx: Consumer<BlockEvent>,
    garbage_tx: Producer<Box<dyn AudioNode>>,

    /// Preallocated input scratch. `input_scratch[port]` is a flat planar
    /// buffer sized to `max_channels * max_block_size`. Reused every
    /// block; never reallocated during processing.
    input_scratch: Vec<Vec<f32>>,
    /// Per-port channel counts for `input_scratch`, used to construct
    /// `BufferRef` views. Mirrored from the currently-processing node.
    input_channel_counts: Vec<u8>,

    /// Per-block event partition: events targeting each node, in offset
    /// order. Cleared every block; keys stay (Vec capacity persists).
    event_partition: BTreeMap<NodeId, Vec<BlockEventInBlock>>,

    /// Per-block snapshot of the graph's topo order, used to walk nodes
    /// without holding a borrow on `self.graph`. Grows monotonically to
    /// the graph's node count, so once warm it never reallocates.
    topo_scratch: Vec<NodeId>,

    /// Shared sample clock — the absolute sample index at which the
    /// *next* `process_block` call will begin. Published with `Release`
    /// at the end of every `process_block` so a host-side observer can
    /// load with `Acquire` and read a consistent post-block transport
    /// position. Initialized to 0. Used by Phase E5's reactive playhead.
    ///
    /// Held as an `Arc` so the engine can clone the handle out to the
    /// host before splitting; the audio thread owns its own copy
    /// behind the same `Arc`.
    sample_clock: Arc<AtomicU64>,

    /// Shared transport state — Playing / Paused / Stopped. Loaded at
    /// the top of every `process_block` and used to gate event /
    /// node / output behavior. Cloned out to the host pre-split via
    /// [`Self::transport_handle`]; the host writes, the audio thread
    /// reads. See [`crate::transport`] for the state-machine
    /// semantics.
    transport: TransportHandle,
}

impl AudioEngine {
    pub(crate) fn new(
        sample_rate: u32,
        max_block_size: usize,
        command_rx: Consumer<GraphCommand>,
        event_rx: Consumer<BlockEvent>,
        midi_event_rx: Consumer<BlockEvent>,
        garbage_tx: Producer<Box<dyn AudioNode>>,
    ) -> Self {
        let max_channels = 2;
        let mut input_scratch = Vec::with_capacity(MAX_INPUT_PORTS_PER_NODE);
        for _ in 0..MAX_INPUT_PORTS_PER_NODE {
            input_scratch.push(vec![0.0; max_channels * max_block_size]);
        }
        Self {
            graph: Graph::new(sample_rate, max_block_size),
            command_rx,
            event_rx,
            midi_event_rx,
            garbage_tx,
            input_scratch,
            input_channel_counts: vec![0u8; MAX_INPUT_PORTS_PER_NODE],
            event_partition: BTreeMap::new(),
            topo_scratch: Vec::new(),
            sample_clock: Arc::new(AtomicU64::new(0)),
            transport: TransportHandle::new(),
        }
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Clone the shared transport handle. See
    /// [`crate::transport::TransportHandle`] for the read / write API.
    pub fn transport_handle(&self) -> TransportHandle {
        self.transport.clone()
    }

    /// Clone the shared sample-clock handle.
    ///
    /// The returned `Arc<AtomicU64>` is the same atomic the audio thread
    /// updates at the end of every `process_block`. Host callers can
    /// `load(Acquire)` it on any thread to read the transport position
    /// at the start of the next block. Initialized to 0 and reset to
    /// 0 only at engine construction; transport-state semantics
    /// (pause / stop with reset) land in Phase E6.
    pub fn sample_clock(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.sample_clock)
    }

    /// Process one block. Writes the master node's output into `output`.
    ///
    /// Must be RT-safe. The command and event queues are drained as
    /// `Consumer::pop()` (lock-free, no allocation). Node processing is
    /// single-threaded in topo order.
    ///
    /// Transport state gates per-block behavior — see [`Transport`] for
    /// the per-variant contract. The node graph runs in **every**
    /// transport state so live MIDI input (from the dedicated MIDI
    /// input queue) always produces sound. Transport only controls
    /// song-event consumption and sample-clock state.
    ///
    /// - **Playing**: song events drain, all nodes process, output is
    ///   live, sample clock advances.
    /// - **Paused**: song events stay queued (resume from where you
    ///   left off), all nodes still process so live MIDI is audible,
    ///   sample clock is frozen.
    /// - **Stopped**: song events drain (host re-arms before the next
    ///   Play), all nodes still process so live MIDI is audible, sample
    ///   clock is forced back to 0.
    ///
    /// **Live MIDI is unconditional.** The dedicated MIDI input queue
    /// (fed by [`MidiInputHandle`](crate::MidiInputHandle)) drains and
    /// partitions on every block in every transport state. Pressing
    /// a key on a MIDI keyboard always makes sound.
    ///
    /// Voices that are already playing continue to render until they
    /// release on a NoteOff. There's no panic / all-notes-off path in
    /// v1 — a NoteOn delivered while Playing that doesn't get a
    /// matching NoteOff before Stop will keep sounding in Stopped
    /// until its envelope releases naturally (sustain=1 patches
    /// hold forever). Future work: send synthetic AllNotesOff on
    /// Stop transitions.
    pub fn process_block(
        &mut self,
        master: NodeId,
        mut output: BufferMut<'_>,
        ctx: ProcessContext,
    ) {
        // 0. Drain commands first regardless of transport state. The host
        //    can edit the graph while paused / stopped (e.g. install an
        //    instrument before pressing play), and any pending RemoveNode
        //    needs to flush garbage through the queue promptly.
        while let Ok(cmd) = self.command_rx.pop() {
            apply_command(&mut self.graph, cmd, &mut self.garbage_tx);
        }
        if self.graph.topology_is_dirty() {
            self.graph.recompute_topology();
        }

        // 1. Transport bookkeeping. Stopped: drain pending *song* events
        //    (so the host's Stop → Play re-arm starts from an empty
        //    queue) and snap sample_clock back to 0. Paused: no-op here
        //    (events stay queued, clock frozen by skipping step 5). The
        //    MIDI input queue is *never* drained on transport change —
        //    live MIDI is independent of song state.
        let transport = self.transport.get();
        if matches!(transport, Transport::Stopped) {
            while self.event_rx.pop().is_ok() {}
            self.sample_clock.store(0, Ordering::Release);
        }

        // 2. Partition events for this block.
        //
        //   - Song events (main queue): drained only while Playing.
        //     Paused keeps them queued for resume; Stopped already
        //     drained them in step 1.
        //   - MIDI input events: drained unconditionally so live
        //     playing reaches the synth in every transport state.
        let drain_song_queue = matches!(transport, Transport::Playing);
        self.partition_events_for_block(&ctx, drain_song_queue);

        // 3. Process every node in topo order, *always*. Live MIDI
        //    input partitioned in step 2 reaches the synth here.
        //
        //    Snapshot the topo order into `topo_scratch` so we can
        //    iterate without holding a borrow on `self.graph` (each
        //    node needs `&mut self.graph` via `process_node`). The
        //    scratch is reused across blocks; only the first few
        //    blocks (until it's large enough) cause an allocation.
        self.topo_scratch.clear();
        self.topo_scratch.extend_from_slice(self.graph.topo_order());
        for i in 0..self.topo_scratch.len() {
            let node_id = self.topo_scratch[i];
            self.process_node(node_id, &ctx);
        }

        // 4. Copy master output to the host's output buffer. Always —
        //    the graph is responsible for producing silence when
        //    nothing is making sound, not the transport gate.
        Self::copy_master_to_output(&self.graph, master, &mut output, ctx.block_size);

        // 5. Publish the next-block sample position only while Playing.
        //    Paused freezes the clock; Stopped already snapped it to 0
        //    in step 1. `Release` pairs with the host's `Acquire` load
        //    — when the load sees this value, all of the output writes
        //    above are visible too. Allocation-free.
        if matches!(transport, Transport::Playing) {
            let next_block_start = ctx
                .absolute_time_samples
                .saturating_add(ctx.block_size as u64);
            self.sample_clock.store(next_block_start, Ordering::Release);
        }
    }

    /// Render the graph offline for a fixed duration, returning planar L/R
    /// sample buffers. The master node must produce a stereo output port
    /// at index 0.
    ///
    /// `block_size` is the working block size for each `process_block`
    /// call. The last block may be shorter if `duration_samples` isn't a
    /// multiple.
    pub fn render_offline(
        &mut self,
        master: NodeId,
        duration_samples: SampleTime,
        block_size: usize,
    ) -> RenderResult {
        assert!(
            block_size <= self.graph.max_block_size(),
            "block_size ({}) cannot exceed engine max_block_size ({})",
            block_size,
            self.graph.max_block_size()
        );

        // Offline render forces transport into Playing for the duration of
        // the call and restores the prior state on exit. This keeps the
        // offline path independent of whatever state a caller left the
        // transport in (default is `Stopped`, which would otherwise
        // silence every render).
        let prior_transport = self.transport.get();
        self.transport.set(Transport::Playing);

        let total = duration_samples.as_samples() as usize;
        let stride = self.graph.max_block_size();
        let mut left = Vec::with_capacity(total);
        let mut right = Vec::with_capacity(total);
        // Buffer sized for the engine's stride, not the per-call block_size.
        // The BufferMut handed to `process_block` uses the same stride, so
        // the slot↔output copy matches.
        let mut block_buf = vec![0.0_f32; 2 * stride];

        let mut produced: u64 = 0;
        while (produced as usize) < total {
            let remaining = total - produced as usize;
            let this_block = remaining.min(block_size);

            // Reset the active portion of both channels.
            for ch in 0..2 {
                let start = ch * stride;
                for s in block_buf[start..start + this_block].iter_mut() {
                    *s = 0.0;
                }
            }
            let output = BufferMut::new(&mut block_buf, 2, this_block, stride);

            let ctx = ProcessContext {
                sample_rate: self.graph.sample_rate(),
                block_size: this_block,
                absolute_time_samples: produced,
                musical_time: MusicalTime::ZERO, // tempo conversion is host's job for v1
                bpm: 120.0,
                playing: true,
            };

            self.process_block(master, output, ctx);

            // Append the block's active L/R samples.
            left.extend_from_slice(&block_buf[..this_block]);
            right.extend_from_slice(&block_buf[stride..stride + this_block]);

            produced += this_block as u64;
        }

        self.transport.set(prior_transport);

        RenderResult {
            left,
            right,
            sample_rate: self.graph.sample_rate(),
        }
    }

    // ---------- Internal helpers ----------

    fn partition_events_for_block(
        &mut self,
        ctx: &ProcessContext,
        drain_song_queue: bool,
    ) {
        // Clear existing partitions but keep their Vec capacity.
        for events in self.event_partition.values_mut() {
            events.clear();
        }

        let block_end = ctx.absolute_time_samples + ctx.block_size as u64;
        // Two independent queues feed events. Drain only those whose
        // `time` falls in this block's window. rtrb's `peek` lets us
        // look at the next event without consuming; when we see an
        // event past the window we stop and leave it queued for a
        // later block.
        //
        //   - **Main (song) queue.** Realized song MIDI + parameter
        //     changes from the host. Gated by `drain_song_queue` —
        //     only Playing consumes; Paused leaves them queued for
        //     resume; Stopped drained them already in step 1 of
        //     `process_block`.
        //   - **MIDI input queue.** Live MIDI from `midir`. Always
        //     drained — pressing a key on a MIDI keyboard makes
        //     sound regardless of transport state.
        //
        // Each queue is individually time-sorted (realize() guarantees
        // this for the main queue; live MIDI is monotonic by
        // construction). The merged per-node sequence is sorted by
        // `offset_in_block` below so consumers see one time-ordered
        // stream.
        if drain_song_queue {
            Self::drain_queue_into_partition(
                &mut self.event_rx,
                &mut self.event_partition,
                ctx,
                block_end,
            );
        }
        Self::drain_queue_into_partition(
            &mut self.midi_event_rx,
            &mut self.event_partition,
            ctx,
            block_end,
        );

        // Sort each per-node Vec by offset so the merge of two
        // time-sorted streams produces a single time-sorted output.
        // Most blocks see 0–1 MIDI events per node, so the sort is
        // cheap; we use Vec::sort_by_key which is in-place + stable
        // (preserves arrival order for events at the same offset,
        // matching what callers got pre-K1 from the single queue).
        for events in self.event_partition.values_mut() {
            events.sort_by_key(|ev| ev.offset_in_block);
        }
    }

    fn drain_queue_into_partition(
        rx: &mut Consumer<BlockEvent>,
        partition: &mut BTreeMap<NodeId, Vec<BlockEventInBlock>>,
        ctx: &ProcessContext,
        block_end: u64,
    ) {
        while let Ok(time) = rx.peek().map(|ev| ev.time.as_samples()) {
            if time >= block_end {
                break;
            }
            let ev = rx.pop().expect("just peeked successfully");
            let abs = ev.time.as_samples();
            let offset = abs.saturating_sub(ctx.absolute_time_samples) as u32;
            partition.entry(ev.target).or_default().push(BlockEventInBlock {
                offset_in_block: offset,
                message: ev.message,
            });
        }
    }

    fn process_node(&mut self, node_id: NodeId, ctx: &ProcessContext) {
        let n_inputs = match self.graph.slot(node_id) {
            Some(slot) => slot.input_channel_counts.len(),
            None => return,
        };
        if n_inputs > MAX_INPUT_PORTS_PER_NODE {
            panic!(
                "node has {} inputs; engine MAX_INPUT_PORTS_PER_NODE is {}",
                n_inputs, MAX_INPUT_PORTS_PER_NODE
            );
        }

        Self::copy_inputs_to_scratch(
            &self.graph,
            &mut self.input_scratch,
            &mut self.input_channel_counts,
            node_id,
            n_inputs,
            ctx,
        );

        let empty_events = Vec::new();
        let node_events = self
            .event_partition
            .get(&node_id)
            .unwrap_or(&empty_events);
        let events = EventBlock::new(node_events);

        let stride = self.graph.max_block_size();

        let Some(slot) = self.graph.slot_mut(node_id) else {
            return;
        };
        let mut ports = PortAccess::new(
            &self.input_scratch[..n_inputs],
            &self.input_channel_counts[..n_inputs],
            &mut slot.output_buffers,
            &slot.output_channel_counts,
            ctx.block_size,
            stride,
        );
        slot.node.process(&mut ports, &events, ctx);
    }

    fn copy_inputs_to_scratch(
        graph: &Graph,
        input_scratch: &mut [Vec<f32>],
        input_channel_counts: &mut [u8],
        node_id: NodeId,
        n_inputs: usize,
        ctx: &ProcessContext,
    ) {
        let stride = graph.max_block_size();
        let Some(slot) = graph.slot(node_id) else {
            return;
        };
        for in_idx in 0..n_inputs {
            let in_channels = slot.input_channel_counts[in_idx];
            input_channel_counts[in_idx] = in_channels;
            let scratch = &mut input_scratch[in_idx];
            for ch in 0..in_channels as usize {
                let start = ch * stride;
                let end = start + ctx.block_size;
                let scratch_end = end.min(scratch.len());
                for s in scratch[start..scratch_end].iter_mut() {
                    *s = 0.0;
                }
            }
            for edge in graph.edges() {
                if edge.to.node == node_id && edge.to.port as usize == in_idx {
                    if let Some(from_slot) = graph.slot(edge.from.node)
                        && let Some(from_buffer) =
                            from_slot.output_buffers.get(edge.from.port as usize)
                    {
                        for ch in 0..in_channels as usize {
                            let start = ch * stride;
                            let end = start + ctx.block_size;
                            let copy_end = end.min(from_buffer.len()).min(scratch.len());
                            scratch[start..copy_end]
                                .copy_from_slice(&from_buffer[start..copy_end]);
                        }
                    }
                    break;
                }
            }
        }
    }

    fn copy_master_to_output(
        graph: &Graph,
        master: NodeId,
        output: &mut BufferMut<'_>,
        block_size: usize,
    ) {
        let stride = graph.max_block_size();
        let Some(slot) = graph.slot(master) else {
            output.clear();
            return;
        };
        let Some(master_buffer) = slot.output_buffers.first() else {
            output.clear();
            return;
        };
        let master_channels = slot.output_channel_counts.first().copied().unwrap_or(0) as usize;
        let out_channels = output.channels();
        let n_channels = master_channels.min(out_channels);
        for ch in 0..n_channels {
            let src_start = ch * stride;
            let src_end = src_start + block_size;
            let src = &master_buffer[src_start..src_end.min(master_buffer.len())];
            let dst = output.channel_mut(ch);
            let n = block_size.min(src.len()).min(dst.len());
            dst[..n].copy_from_slice(&src[..n]);
        }
        for ch in n_channels..out_channels {
            output.channel_mut(ch).fill(0.0);
        }
    }
}

/// The result of an offline render: planar L/R sample buffers plus the
/// sample rate they were rendered at.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderResult {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub sample_rate: u32,
}

impl RenderResult {
    pub fn frames(&self) -> usize {
        self.left.len()
    }
}
