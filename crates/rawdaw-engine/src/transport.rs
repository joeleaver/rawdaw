//! Transport state — Playing / Paused / Stopped.
//!
//! The host mutates the transport state via an `Arc<AtomicU8>` shared
//! with the audio side. The audio thread loads it (`Acquire`) at the
//! top of every `process_block` and gates its behavior:
//!
//! - **Playing**: normal path. Commands drain, events partition into
//!   this block's window, nodes process, output is written, the
//!   shared sample clock advances.
//! - **Paused**: commands still drain (so the host can mutate the
//!   graph while paused — e.g. installing a new instrument node),
//!   but events stay queued for the eventual resume, nodes don't
//!   process, output is silent, and the sample clock isn't updated
//!   (the UI playhead freezes wherever it last was).
//! - **Stopped**: commands drain, the entire event queue is also
//!   drained (so the host can re-arm by re-pushing the realized
//!   events for the next play), nodes don't process, output is
//!   silent, and the sample clock is forced back to 0.
//!
//! The state-machine semantics intentionally live alongside the
//! audio thread, not in the cpal driver — `render_offline` and any
//! future offline-render path get the same behavior for free
//! (an offline render with `Transport::Stopped` produces silence
//! deterministically without the driver layer being involved).

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Audio transport state.
///
/// Stored on the audio thread as an `AtomicU8` so the host can flip it
/// from any thread (UI handler, keyboard shortcut, MIDI in the future)
/// without a lock. The packing is unstable — always go through
/// [`Transport::from_u8`] and [`Transport::as_u8`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Transport {
    /// Time advances, events fire, nodes produce output.
    Playing,
    /// Time frozen, events queued, output silent. Commands still
    /// apply so the host can edit the graph while paused.
    Paused,
    /// Time reset to 0, events drained, output silent. The host is
    /// expected to re-push the realized events before transitioning
    /// back to [`Transport::Playing`].
    Stopped,
}

impl Transport {
    /// Pack into a `u8` for atomic storage. `Stopped` is `0` so that
    /// `AtomicU8::default()` lands on the safest state — silent, clock
    /// at 0 — without an explicit init step.
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Stopped => 0,
            Self::Paused => 1,
            Self::Playing => 2,
        }
    }

    /// Unpack from atomic storage. Unrecognized values fall back to
    /// [`Transport::Stopped`].
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Paused,
            2 => Self::Playing,
            _ => Self::Stopped,
        }
    }
}

impl Default for Transport {
    /// Default state at engine construction: stopped. The host must
    /// explicitly transition to [`Transport::Playing`] for audio to
    /// flow.
    fn default() -> Self {
        Self::Stopped
    }
}

/// Shared transport handle. Cloning gives another reference to the
/// same atomic. The audio side and host side both hold a copy.
#[derive(Debug, Clone, Default)]
pub struct TransportHandle {
    state: Arc<AtomicU8>,
}

impl TransportHandle {
    /// New handle at [`Transport::Stopped`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Load the current state with `Acquire` ordering. Paired with
    /// every [`Self::set`] call's `Release` so any side effects the
    /// host published before flipping the state become visible.
    pub fn get(&self) -> Transport {
        Transport::from_u8(self.state.load(Ordering::Acquire))
    }

    /// Store a new state with `Release` ordering.
    pub fn set(&self, transport: Transport) {
        self.state.store(transport.as_u8(), Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_and_unpacks_all_variants() {
        for t in [Transport::Playing, Transport::Paused, Transport::Stopped] {
            assert_eq!(Transport::from_u8(t.as_u8()), t);
        }
    }

    #[test]
    fn unrecognized_byte_is_stopped() {
        assert_eq!(Transport::from_u8(42), Transport::Stopped);
    }

    #[test]
    fn default_is_stopped() {
        assert_eq!(Transport::default(), Transport::Stopped);
        assert_eq!(TransportHandle::new().get(), Transport::Stopped);
    }

    #[test]
    fn handle_shares_state_across_clones() {
        let a = TransportHandle::new();
        let b = a.clone();
        a.set(Transport::Playing);
        assert_eq!(b.get(), Transport::Playing);
        b.set(Transport::Paused);
        assert_eq!(a.get(), Transport::Paused);
    }
}
