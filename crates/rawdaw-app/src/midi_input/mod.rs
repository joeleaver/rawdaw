//! External MIDI input — K1 of `docs/midi-input-plan.md`.
//!
//! `midir` enumerates and opens an input port. Each incoming MIDI
//! message is parsed by the callback (running on midir's dedicated
//! thread), translated to a [`Midi2Message`], and pushed into the
//! engine's MIDI input queue via [`MidiInputHandle`]. The audio
//! thread drains that queue at the start of every `process_block`
//! and routes the event to the configured target node.
//!
//! ## Threading
//!
//! `midir` spawns its own thread for the input callback when
//! `MidiInput::connect` is called. The callback captures the
//! `MidiInputBridge` state by `&mut`, so we move the bridge into
//! midir via the user-data parameter and let midir's thread own it
//! for the lifetime of the connection. On `MidiInputConnection::close()`
//! midir returns the bridge back to us, so device switching (K2)
//! recovers the [`MidiInputHandle`] without re-creating the engine
//! queue.
//!
//! ## Scope (K1)
//!
//! - Notes only (status `0x9_` with velocity > 0 = NoteOn; status
//!   `0x8_` or `0x9_` with velocity 0 = NoteOff). Velocity widens to
//!   `U16Velocity::from_u7`.
//! - Auto-pick first available device on startup.
//! - Routing target hardcoded to the first Pitched track's NodeId.
//!   K3 makes this dynamic.
//! - No UI; K2 adds the device picker.
//! - No pitch bend, no sustain pedal, no CCs. K4 / K5 add those.
//!
//! ## Failure modes
//!
//! Every fallible step returns [`MidiInputError`]:
//! - No midir backend (rare on supported platforms) → `BackendInit`.
//! - Zero connected devices → `auto_pick_input` returns `Ok(None)`,
//!   not an error. The host runs without MIDI input silently.
//! - Device disappears mid-session → midir's callback stops firing;
//!   the connection stays open. Manual reconnect lands with K2.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use midir::{MidiInput, MidiInputConnection, MidiInputPort};

use rawdaw_engine::{MidiInputHandle, NodeId};
use rawdaw_model::{Midi2Message, MidiChannel, MidiNote, SampleTime, U16Velocity, U7};

const CLIENT_NAME: &str = "rawdaw";
const PORT_NAME: &str = "rawdaw-midi-in";

/// Errors surfaced when opening a MIDI input device.
#[derive(Debug)]
pub enum MidiInputError {
    /// midir failed to initialize. The `String` is midir's own error
    /// message — usually carries the backend's diagnostic (ALSA can't
    /// open the sequencer, etc.).
    BackendInit(String),
    /// midir's `connect` failed. Carries the underlying message.
    Connect(String),
}

impl std::fmt::Display for MidiInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendInit(s) => write!(f, "MIDI input backend init failed: {s}"),
            Self::Connect(s) => write!(f, "MIDI input connection failed: {s}"),
        }
    }
}

impl std::error::Error for MidiInputError {}

/// A discoverable MIDI input device. Pair of (midir's opaque port
/// handle, display name).
pub struct MidiInputDevice {
    pub name: String,
    pub port: MidiInputPort,
}

impl std::fmt::Debug for MidiInputDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MidiInputDevice")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// Enumerate every MIDI input device midir can see. Each call creates
/// a fresh `MidiInput` (midir's API doesn't expose a stable enumerator
/// type) and returns the snapshot. Hot-plugged devices that appear
/// after this call won't show up until the next enumeration.
pub fn enumerate_inputs() -> Result<Vec<MidiInputDevice>, MidiInputError> {
    let input = MidiInput::new(CLIENT_NAME).map_err(|e| MidiInputError::BackendInit(e.to_string()))?;
    let mut devices = Vec::new();
    for port in input.ports() {
        let name = input.port_name(&port).unwrap_or_else(|_| "(unnamed)".to_string());
        devices.push(MidiInputDevice { name, port });
    }
    Ok(devices)
}

/// Pick the first MIDI input device midir can see, if any. Returns
/// `Ok(None)` (not an error) when zero devices are connected — the
/// host runs without MIDI input.
///
/// Filters out the ALSA "Midi Through" virtual loopback device: it's
/// always present on Linux but never carries a real instrument's
/// notes (it's a software bridge for routing MIDI between
/// applications). Auto-picking it would mean a USB keyboard goes
/// unheard on a machine that has the loopback enumerated first.
/// K2 surfaces a full picker that includes Midi Through for users
/// who specifically want to route through it.
pub fn auto_pick_input() -> Result<Option<MidiInputDevice>, MidiInputError> {
    Ok(enumerate_inputs()?
        .into_iter()
        .find(|d| !is_virtual_loopback(&d.name)))
}

/// True for ALSA "Midi Through" / equivalents on other platforms.
/// Conservative — only filters the well-known loopback names. Other
/// virtual devices (PipeWire bridges, named loopback patchbays) pass
/// through so users running real MIDI through a router still get
/// auto-picked.
fn is_virtual_loopback(name: &str) -> bool {
    name.starts_with("Midi Through")
}

/// State captured by midir's callback. Owned by midir's thread for
/// the lifetime of the [`MidiInputConnection`]; recovered on close.
///
/// All fields are cheap to clone (Arc / NodeId / Producer) so a
/// device switch can move the bridge from one connection to the
/// next without re-allocating.
pub struct MidiInputBridge {
    pub handle: MidiInputHandle,
    /// Engine sample clock. Currently unused (K1.fix moved
    /// scheduling to `SampleTime::samples(0)` — see
    /// [`handle_midi_message`] for the rationale). Retained on the
    /// bridge so future expressive-control features (K4 pitch bend,
    /// K5 CC routing) can opt back into sample-accurate scheduling
    /// if they need it.
    #[allow(dead_code)] // reserved for K4/K5
    pub sample_clock: Arc<AtomicU64>,
    /// Routing target — which NodeId receives the MIDI. K1 sets this
    /// once at startup; K3 makes it dynamic by mutating the atomic
    /// from the host thread on track selection.
    pub target: Arc<AtomicU32>,
}

impl MidiInputBridge {
    pub fn new(
        handle: MidiInputHandle,
        sample_clock: Arc<AtomicU64>,
        target: NodeId,
    ) -> Self {
        Self {
            handle,
            sample_clock,
            target: Arc::new(AtomicU32::new(target.0)),
        }
    }

    /// Clone of the routing atomic. K3 will hold this on AppState
    /// and `.store()` whenever the user picks a new track.
    #[allow(dead_code)] // wired by K3
    pub fn target_handle(&self) -> Arc<AtomicU32> {
        Arc::clone(&self.target)
    }
}

/// Open a MIDI input connection. The returned
/// [`MidiInputConnection`] holds the port open for the lifetime of
/// the returned value; drop it to close.
pub fn open(
    device: &MidiInputDevice,
    bridge: MidiInputBridge,
) -> Result<MidiInputConnection<MidiInputBridge>, MidiInputError> {
    let input = MidiInput::new(CLIENT_NAME).map_err(|e| MidiInputError::BackendInit(e.to_string()))?;
    input
        .connect(
            &device.port,
            PORT_NAME,
            move |_timestamp_us, message, bridge| {
                handle_midi_message(message, bridge);
            },
            bridge,
        )
        .map_err(|e| MidiInputError::Connect(e.to_string()))
}

/// Parse one MIDI 1.0 message and forward to the engine queue.
///
/// Live MIDI events are pushed at **`SampleTime::samples(0)`**, not
/// at `sample_clock + 1`. Reasoning:
///
/// - A human key-press is "now-ish"; sub-block sample-accuracy
///   doesn't matter (one block at 256/48 kHz is ~5 ms; humans don't
///   perceive that).
/// - Scheduling at `sample_clock + 1` was racy under transport
///   changes — if the user hit Stop between midir's read of
///   `sample_clock` and the audio thread's reset of it, events
///   pushed at the old high value got stuck above `block_end` and
///   never fired.
/// - The engine's partition step computes
///   `offset_in_block = saturating_sub(time, ctx.absolute_time)`.
///   With `time = 0`, this always saturates to 0, so live MIDI
///   events fire at offset 0 of the next block they're partitioned
///   into. The block-window check `time < block_end` is also always
///   true since `block_end ≥ 1`, so events always drain on the next
///   block regardless of transport state.
fn handle_midi_message(message: &[u8], bridge: &mut MidiInputBridge) {
    let translated = translate(message);
    if midi_debug_enabled() {
        let raw: Vec<String> = message.iter().map(|b| format!("{b:02X}")).collect();
        match &translated {
            Some(msg) => eprintln!("midi raw=[{}] parsed={msg:?}", raw.join(" ")),
            None => eprintln!("midi raw=[{}] dropped", raw.join(" ")),
        }
    }
    let Some(translated) = translated else {
        return;
    };
    let target = NodeId::new(bridge.target.load(Ordering::Acquire));
    // The MIDI input queue is sized at DEFAULT_EVENT_QUEUE_CAPACITY
    // (16,384). Even a stuck-key trill can't realistically fill it;
    // a push failure here means the audio thread is wedged. Swallow
    // the error rather than panicking from midir's callback.
    let _ = bridge
        .handle
        .push_midi(SampleTime::samples(0), target, translated);
}

/// Enable per-message stderr tracing by setting `RAWDAW_MIDI_DEBUG=1`
/// in the environment. Useful for diagnosing controllers whose
/// running-status / sysex / aftertouch behavior trips up the
/// translator. Off by default to avoid spam.
fn midi_debug_enabled() -> bool {
    std::env::var_os("RAWDAW_MIDI_DEBUG").is_some()
}

/// Pure MIDI 1.0 → [`Midi2Message`] translation. v1 covers Note On
/// and Note Off only; other statuses return `None` and are silently
/// dropped at K1. K4 / K5 extend the match arms.
pub fn translate(message: &[u8]) -> Option<Midi2Message> {
    if message.len() < 3 {
        return None;
    }
    let status = message[0] & 0xF0;
    let channel = MidiChannel::new(message[0] & 0x0F)?;
    let data1 = message[1];
    let data2 = message[2];
    let note = MidiNote::new(data1)?;
    let velocity_u7 = U7::new(data2)?;
    match status {
        0x90 if data2 > 0 => Some(Midi2Message::NoteOn {
            channel,
            note,
            velocity: U16Velocity::from_u7(velocity_u7),
        }),
        // Note On with velocity 0 is the canonical "Note Off" alias —
        // MIDI 1.0 hardware often uses it to avoid a status-byte
        // switch in running-status mode.
        0x90 => Some(Midi2Message::NoteOff {
            channel,
            note,
            velocity: U16Velocity::from_u7(velocity_u7),
        }),
        0x80 => Some(Midi2Message::NoteOff {
            channel,
            note,
            velocity: U16Velocity::from_u7(velocity_u7),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_parses_note_on() {
        // 0x90 = NoteOn on channel 0, note 60 (middle C), velocity 100.
        let msg = [0x90, 60, 100];
        let parsed = translate(&msg).expect("parse note on");
        match parsed {
            Midi2Message::NoteOn {
                channel,
                note,
                velocity,
            } => {
                assert_eq!(channel.get(), 0);
                assert_eq!(note.get(), 60);
                // U7::100 widens via from_u7's MIDI 2.0 scaling —
                // exact value isn't load-bearing; non-zero is.
                assert!(velocity.get() > 0);
            }
            _ => panic!("expected NoteOn, got {parsed:?}"),
        }
    }

    #[test]
    fn translate_parses_note_off() {
        // 0x80 = NoteOff on channel 0, note 60, velocity 64.
        let msg = [0x80, 60, 64];
        let parsed = translate(&msg).expect("parse note off");
        assert!(matches!(parsed, Midi2Message::NoteOff { .. }));
    }

    #[test]
    fn note_on_with_velocity_zero_aliases_to_note_off() {
        // MIDI 1.0 running-status convention — sequencers send
        // 0x90 + velocity 0 instead of 0x80 to keep the status byte
        // sticky. We must treat it as NoteOff.
        let msg = [0x90, 60, 0];
        let parsed = translate(&msg).expect("parse note on with velocity 0");
        assert!(matches!(parsed, Midi2Message::NoteOff { .. }));
    }

    #[test]
    fn translate_returns_none_for_short_message() {
        // System realtime messages (clock, start, stop) are 1 byte;
        // we ignore them.
        assert!(translate(&[0xF8]).is_none());
        assert!(translate(&[0xFA]).is_none());
    }

    #[test]
    fn translate_returns_none_for_control_change_v1() {
        // 0xB_ is Control Change. K5 will route these to the mod
        // matrix; v1 drops them silently.
        let msg = [0xB0, 7, 100];
        assert!(translate(&msg).is_none());
    }

    #[test]
    fn translate_returns_none_for_pitch_bend_v1() {
        // 0xE_ is Pitch Bend. K4 will handle these; v1 drops.
        let msg = [0xE0, 0x00, 0x40];
        assert!(translate(&msg).is_none());
    }

    #[test]
    fn translate_rejects_out_of_range_note() {
        // MIDI note 200 is invalid (range is 0..=127). MidiNote::new
        // returns None, so the message gets dropped.
        let msg = [0x90, 200, 100];
        assert!(translate(&msg).is_none());
    }
}
