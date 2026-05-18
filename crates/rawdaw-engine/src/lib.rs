//! rawdaw audio engine.
//!
//! See `docs/design/engine.md` for the full design. Key points:
//!
//! - DAG of `AudioNode`s, processed in topological order each block.
//! - RT-safe `process_block()`: no allocation, locks, or syscalls on the
//!   audio thread.
//! - Two SPSC queues (host → audio) for graph commands and MIDI events.
//! - Offline-first: `Engine::render_offline()` is the primary test surface;
//!   the cpal driver (iteration 3) is a thin wrapper.
//!
//! The crate forbids unsafe code; the engine relies on safe Rust patterns
//! (index-based ownership, scratch buffers, flat planar layout) to satisfy
//! the borrow checker during single-threaded graph traversal.

#![forbid(unsafe_code)]

pub mod audio_engine;
pub mod buffer;
pub mod command;
pub mod context;
#[cfg(feature = "cpal-driver")]
pub mod cpal_driver;
pub mod engine;
pub mod event;
pub mod graph;
pub mod handle;
pub mod node;
pub mod nodes;
pub mod transport;
pub mod translate;

pub use audio_engine::{AudioEngine, RenderResult};
pub use buffer::{BufferMut, BufferRef, ChannelCount};
pub use command::GraphCommand;
pub use context::ProcessContext;
pub use engine::{
    Engine, QueueCapacities, DEFAULT_COMMAND_QUEUE_CAPACITY, DEFAULT_EVENT_QUEUE_CAPACITY,
    DEFAULT_GARBAGE_QUEUE_CAPACITY,
};
pub use event::{BlockEvent, BlockEventInBlock, EventBlock};
pub use graph::{Edge, Graph, NodeId, NodePort};
pub use handle::EngineHandle;
pub use node::{AudioNode, InputDescriptor, OutputDescriptor, PortAccess, PortInputs, PortOutputs};
pub use nodes::{ImpulseNode, MixerNode, SilenceNode, SineNode};
pub use rtrb::PushError;
pub use translate::{translate_events, TrackRouting, TranslateError};
pub use transport::{Transport, TransportHandle};
