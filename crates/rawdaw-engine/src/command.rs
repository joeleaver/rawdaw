//! Graph mutation commands.
//!
//! Commands flow host → audio via a lock-free SPSC ring buffer. The
//! audio thread drains them at the top of every `process_block` and
//! applies them in order.
//!
//! Removed nodes flow audio → host via a second SPSC ring buffer so the
//! host drops them off the audio thread.

use rtrb::Producer;

use crate::graph::{Edge, Graph, NodeId};
use crate::node::AudioNode;

/// One mutation of the graph.
pub enum GraphCommand {
    /// Install a new node at the host-allocated `id`. The slot at that
    /// index must be empty.
    AddNode {
        id: NodeId,
        node: Box<dyn AudioNode>,
    },

    /// Remove the node at `id`. Any edges referencing it are also dropped.
    /// The removed node is moved to the engine's garbage queue for the
    /// host to drop on its own time (audio-thread drop would risk
    /// non-RT-safe deallocation).
    RemoveNode { id: NodeId },

    /// Add a directed audio edge from `edge.from` to `edge.to`.
    Connect { edge: Edge },

    /// Remove a previously-added edge.
    Disconnect { edge: Edge },

    /// Apply a group of commands as one atomic unit. The graph's topo
    /// order recomputes once after the whole batch, not after each
    /// inner command.
    Batch(Vec<GraphCommand>),
}

/// Apply a single command to the graph. Removed nodes are pushed into
/// the audio → host garbage queue so the host can drop them off the
/// audio thread.
///
/// Called from the audio thread; must be RT-safe in the steady state.
/// `AddNode` and `Batch(AddNode...)` move ownership of an already-
/// allocated `Box<dyn AudioNode>` and don't allocate themselves.
/// `Connect`/`Disconnect` may extend `edges` (amortized non-allocating
/// because `Vec` has reserved capacity for the working set).
///
/// If the garbage queue is full when a `RemoveNode` lands, the removed
/// node drops at the end of this call (which happens on the audio
/// thread — non-RT-safe). A `debug_assert` fires so debug builds
/// surface the bug; release builds limp along to avoid a callback
/// panic. The garbage queue is sized generously (see
/// `DEFAULT_GARBAGE_QUEUE_CAPACITY`) precisely so this stays
/// vanishingly rare.
pub(crate) fn apply_command(
    graph: &mut Graph,
    command: GraphCommand,
    garbage_tx: &mut Producer<Box<dyn AudioNode>>,
) {
    match command {
        GraphCommand::AddNode { id, node } => {
            graph.install_node(id, node);
        }
        GraphCommand::RemoveNode { id } => {
            if let Some(node) = graph.uninstall_node(id) {
                let push_result = garbage_tx.push(node);
                debug_assert!(
                    push_result.is_ok(),
                    "garbage queue full when applying RemoveNode; host must drain regularly",
                );
            }
        }
        GraphCommand::Connect { edge } => {
            graph.connect(edge);
        }
        GraphCommand::Disconnect { edge } => {
            graph.disconnect(edge);
        }
        GraphCommand::Batch(inner) => {
            for c in inner {
                apply_command(graph, c, garbage_tx);
            }
        }
    }
}
