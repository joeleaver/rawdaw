//! The audio graph: nodes, edges, and topological ordering.
//!
//! NodeIds are dense `u32` indices into the graph's node table. They are
//! allocated by the host *before* a node is added; the engine just installs
//! the node at the requested index. This keeps the audio thread free of
//! any "return value" channel.
//!
//! Edges are directed from one node's output port to another node's input
//! port. Cycles are illegal and will trip a debug assertion when the topo
//! order is recomputed.

use crate::node::AudioNode;

/// A dense, stable identifier for a node in the graph.
///
/// Host-side allocation: the host owns a monotonically increasing counter
/// and assigns IDs before pushing `AddNode` commands. The host is also
/// responsible for not reusing an ID until the engine has applied the
/// corresponding `RemoveNode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

impl NodeId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A port reference: a specific input or output index on a specific node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodePort {
    pub node: NodeId,
    pub port: u8,
}

impl NodePort {
    pub const fn new(node: NodeId, port: u8) -> Self {
        Self { node, port }
    }
}

/// A directed audio edge: `from.node`'s `from.port` output feeds `to.node`'s
/// `to.port` input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Edge {
    pub from: NodePort,
    pub to: NodePort,
}

/// A slot in the graph that holds one node, its preallocated output buffers,
/// and the cached channel counts. Buffers are sized at the time the node
/// is installed (via `AddNode`).
///
/// Output buffers are flat planar (`channels * max_block_size` per port) —
/// see `buffer.rs` for the layout. Channel counts are cached as `u8` slices
/// so the engine's per-block code can borrow them without recomputing from
/// the descriptors each call.
///
/// We deliberately do *not* store the full `InputDescriptor`/`OutputDescriptor`
/// slices; the engine only needs channel counts. Names and other metadata
/// are looked up via the node itself when needed (rare, non-RT paths).
pub(crate) struct NodeSlot {
    pub node: Box<dyn AudioNode>,
    /// Per-port flat planar buffer; length = `channels * max_block_size`.
    pub output_buffers: Vec<Vec<f32>>,
    /// Channel counts per port, cached for fast per-block borrow.
    pub input_channel_counts: Vec<u8>,
    pub output_channel_counts: Vec<u8>,
}

impl NodeSlot {
    pub(crate) fn new(mut node: Box<dyn AudioNode>, sample_rate: u32, max_block_size: usize) -> Self {
        node.prepare(sample_rate, max_block_size);

        let input_channel_counts: Vec<u8> = node
            .input_descriptors()
            .iter()
            .map(|d| d.channels.count() as u8)
            .collect();
        let output_channel_counts: Vec<u8> = node
            .output_descriptors()
            .iter()
            .map(|d| d.channels.count() as u8)
            .collect();

        let output_buffers = node
            .output_descriptors()
            .iter()
            .map(|d| vec![0.0_f32; d.channels.count() * max_block_size])
            .collect();

        Self {
            node,
            output_buffers,
            input_channel_counts,
            output_channel_counts,
        }
    }
}

/// The audio graph. Owns all nodes and edges; computes topological order
/// on demand. Lives on the audio thread; mutated only via commands.
pub struct Graph {
    /// Sparse vector of slots; `None` means "no node at this index"
    /// (either never installed, or removed). Indexed by `NodeId.0 as usize`.
    nodes: Vec<Option<NodeSlot>>,
    edges: Vec<Edge>,
    /// Topo order of installed nodes. Recomputed lazily after mutations
    /// (see `topology_dirty`). Empty until `recompute_topology` runs.
    topo_order: Vec<NodeId>,
    topology_dirty: bool,
    sample_rate: u32,
    max_block_size: usize,
}

impl Graph {
    pub fn new(sample_rate: u32, max_block_size: usize) -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            topo_order: Vec::new(),
            topology_dirty: false,
            sample_rate,
            max_block_size,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn max_block_size(&self) -> usize {
        self.max_block_size
    }

    /// Install a node at the given `NodeId`. The slot must currently be
    /// empty (no node installed at that ID). Sizes output buffers from
    /// the node's `output_descriptors()` and the graph's `max_block_size`.
    pub(crate) fn install_node(&mut self, id: NodeId, node: Box<dyn AudioNode>) {
        let idx = id.0 as usize;
        if idx >= self.nodes.len() {
            self.nodes.resize_with(idx + 1, || None);
        }
        debug_assert!(
            self.nodes[idx].is_none(),
            "NodeId {idx} already in use; host allocated a duplicate ID"
        );
        self.nodes[idx] = Some(NodeSlot::new(node, self.sample_rate, self.max_block_size));
        self.topology_dirty = true;
    }

    /// Remove a node. Returns the boxed node so the caller (the engine's
    /// command-application path) can route it to the host-side garbage
    /// queue rather than dropping on the audio thread.
    #[must_use]
    pub(crate) fn uninstall_node(&mut self, id: NodeId) -> Option<Box<dyn AudioNode>> {
        let idx = id.0 as usize;
        let slot = self.nodes.get_mut(idx)?.take()?;
        // Drop any edges referencing this node.
        self.edges
            .retain(|e| e.from.node != id && e.to.node != id);
        self.topology_dirty = true;
        Some(slot.node)
    }

    pub(crate) fn connect(&mut self, edge: Edge) {
        debug_assert!(
            self.has_node(edge.from.node) && self.has_node(edge.to.node),
            "edge references missing node"
        );
        self.edges.push(edge);
        self.topology_dirty = true;
    }

    pub(crate) fn disconnect(&mut self, edge: Edge) {
        self.edges.retain(|e| *e != edge);
        self.topology_dirty = true;
    }

    pub fn has_node(&self, id: NodeId) -> bool {
        self.nodes
            .get(id.0 as usize)
            .map(|slot| slot.is_some())
            .unwrap_or(false)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.iter().filter(|s| s.is_some()).count()
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn topo_order(&self) -> &[NodeId] {
        &self.topo_order
    }

    pub fn topology_is_dirty(&self) -> bool {
        self.topology_dirty
    }

    /// Recompute the topological order. Kahn's algorithm over the current
    /// edges. Panics in debug if the graph contains a cycle.
    ///
    /// This runs once per batch of mutations (i.e., once per `process_block`
    /// at most, not per node), so it's not in the per-sample hot path —
    /// but it is on the audio thread, so it must not allocate beyond
    /// `topo_order`'s already-reserved capacity.
    pub(crate) fn recompute_topology(&mut self) {
        self.topo_order.clear();
        // Reserve capacity proportional to installed nodes. This may allocate
        // on first run / on growth; subsequent runs reuse the capacity.
        self.topo_order.reserve(self.node_count());

        // in_degree[i] = number of incoming edges to node at index i, where
        // node is installed. Uninstalled slots are marked None.
        let mut in_degree: Vec<Option<u32>> = self
            .nodes
            .iter()
            .map(|slot| slot.as_ref().map(|_| 0))
            .collect();
        for edge in &self.edges {
            let idx = edge.to.node.0 as usize;
            if let Some(Some(d)) = in_degree.get_mut(idx) {
                *d += 1;
            }
        }

        // Initial frontier: nodes with in-degree 0.
        let mut frontier: Vec<NodeId> = in_degree
            .iter()
            .enumerate()
            .filter_map(|(i, d)| match d {
                Some(0) => Some(NodeId(i as u32)),
                _ => None,
            })
            .collect();

        while let Some(id) = frontier.pop() {
            self.topo_order.push(id);
            for edge in &self.edges {
                if edge.from.node == id {
                    let to_idx = edge.to.node.0 as usize;
                    if let Some(Some(d)) = in_degree.get_mut(to_idx) {
                        *d -= 1;
                        if *d == 0 {
                            frontier.push(edge.to.node);
                        }
                    }
                }
            }
        }

        debug_assert_eq!(
            self.topo_order.len(),
            self.node_count(),
            "graph has a cycle: {} nodes ordered out of {}",
            self.topo_order.len(),
            self.node_count(),
        );
        self.topology_dirty = false;
    }

    pub(crate) fn slot(&self, id: NodeId) -> Option<&NodeSlot> {
        self.nodes.get(id.0 as usize)?.as_ref()
    }

    pub(crate) fn slot_mut(&mut self, id: NodeId) -> Option<&mut NodeSlot> {
        self.nodes.get_mut(id.0 as usize)?.as_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::ChannelCount;

    struct TestNode;
    impl AudioNode for TestNode {
        fn process(
            &mut self,
            _: &mut crate::node::PortAccess<'_>,
            _: &crate::event::EventBlock<'_>,
            _: &crate::context::ProcessContext,
        ) {
        }
        fn output_descriptors(&self) -> &[crate::node::OutputDescriptor] {
            const D: &[crate::node::OutputDescriptor] = &[crate::node::OutputDescriptor {
                name: "out",
                channels: ChannelCount::Mono,
            }];
            D
        }
        fn prepare(&mut self, _: u32, _: usize) {}
    }

    fn add(graph: &mut Graph, id: u32) -> NodeId {
        let n = NodeId(id);
        graph.install_node(n, Box::new(TestNode));
        n
    }

    #[test]
    fn install_and_uninstall_track_node_count() {
        let mut g = Graph::new(48_000, 256);
        let a = add(&mut g, 0);
        let b = add(&mut g, 1);
        assert_eq!(g.node_count(), 2);
        assert!(g.has_node(a) && g.has_node(b));
        assert!(g.uninstall_node(a).is_some());
        assert_eq!(g.node_count(), 1);
        assert!(!g.has_node(a));
    }

    #[test]
    fn topo_sort_orders_linear_chain() {
        let mut g = Graph::new(48_000, 256);
        let a = add(&mut g, 0);
        let b = add(&mut g, 1);
        let c = add(&mut g, 2);
        g.connect(Edge { from: NodePort::new(a, 0), to: NodePort::new(b, 0) });
        g.connect(Edge { from: NodePort::new(b, 0), to: NodePort::new(c, 0) });
        g.recompute_topology();
        let order = g.topo_order();
        // a precedes b precedes c.
        let pos = |id: NodeId| order.iter().position(|n| *n == id).unwrap();
        assert!(pos(a) < pos(b));
        assert!(pos(b) < pos(c));
    }

    #[test]
    fn topo_sort_orders_diamond() {
        // a -> b -> d
        //  \-> c -/
        let mut g = Graph::new(48_000, 256);
        let a = add(&mut g, 0);
        let b = add(&mut g, 1);
        let c = add(&mut g, 2);
        let d = add(&mut g, 3);
        g.connect(Edge { from: NodePort::new(a, 0), to: NodePort::new(b, 0) });
        g.connect(Edge { from: NodePort::new(a, 0), to: NodePort::new(c, 0) });
        g.connect(Edge { from: NodePort::new(b, 0), to: NodePort::new(d, 0) });
        g.connect(Edge { from: NodePort::new(c, 0), to: NodePort::new(d, 0) });
        g.recompute_topology();
        let order = g.topo_order();
        let pos = |id: NodeId| order.iter().position(|n| *n == id).unwrap();
        assert!(pos(a) < pos(b));
        assert!(pos(a) < pos(c));
        assert!(pos(b) < pos(d));
        assert!(pos(c) < pos(d));
    }

    #[test]
    fn disconnect_removes_edge() {
        let mut g = Graph::new(48_000, 256);
        let a = add(&mut g, 0);
        let b = add(&mut g, 1);
        let edge = Edge { from: NodePort::new(a, 0), to: NodePort::new(b, 0) };
        g.connect(edge);
        assert_eq!(g.edges().len(), 1);
        g.disconnect(edge);
        assert_eq!(g.edges().len(), 0);
    }

    #[test]
    fn uninstall_drops_incident_edges() {
        let mut g = Graph::new(48_000, 256);
        let a = add(&mut g, 0);
        let b = add(&mut g, 1);
        g.connect(Edge { from: NodePort::new(a, 0), to: NodePort::new(b, 0) });
        let _ = g.uninstall_node(b);
        assert_eq!(g.edges().len(), 0);
    }

    #[test]
    #[should_panic(expected = "cycle")]
    fn topo_sort_panics_on_cycle() {
        let mut g = Graph::new(48_000, 256);
        let a = add(&mut g, 0);
        let b = add(&mut g, 1);
        g.connect(Edge { from: NodePort::new(a, 0), to: NodePort::new(b, 0) });
        g.connect(Edge { from: NodePort::new(b, 0), to: NodePort::new(a, 0) });
        g.recompute_topology();
    }
}
