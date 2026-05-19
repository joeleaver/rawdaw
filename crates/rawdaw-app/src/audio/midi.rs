//! MIDI-input wiring for [`AudioResources`].
//!
//! Split out from `audio/mod.rs` once that file reached the ~700-line
//! cap. Hosts the impl methods that bridge between rinch (UI thread,
//! signals + click handlers) and the audio-thread MIDI input queue
//! (via [`midir`]'s callback closure).
//!
//! Topology:
//!
//! - [`AudioResources::midi_target`] is a host-owned
//!   `Arc<AtomicU32>` shared with the bridge inside
//!   [`midir`]'s callback. K3 routing (track selection) writes a
//!   NodeId to this atomic from the host; the bridge reads on
//!   every incoming MIDI message and addresses the resulting
//!   `BlockEvent` at that NodeId.
//! - [`AudioResources::midi_input_handle`] holds the
//!   [`MidiInputHandle`] when no connection is open (e.g. after
//!   [`set_midi_device(None)`](AudioResources::set_midi_device) or
//!   after `build_from_project_and_rate` before `build` opens one).
//! - [`AudioResources::_midi_connection`] holds the active
//!   [`midir::MidiInputConnection`] — dropping it (e.g. on AudioResources
//!   drop or device-switch) closes the port and returns the bridge.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use rawdaw_engine::NodeId;
use rawdaw_model::patch::SynthAssignment;
use rawdaw_model::project::Project;

use crate::midi_input::{self, MidiInputBridge};

use super::AudioResources;

/// First Pitched track's NodeId, used as the initial MIDI input
/// routing target. Free-standing so
/// `build_from_project_and_rate` can call it before AudioResources
/// is fully constructed. `None` for all-drum projects.
pub(super) fn first_pitched_track_node_id(project: &Project) -> Option<NodeId> {
    for (i, track) in project.tracks.iter().enumerate() {
        if matches!(track.synth, SynthAssignment::Wavetable(_)) {
            return Some(NodeId::new((i + 1) as u32));
        }
    }
    None
}

impl AudioResources {
    /// Auto-pick the first non-loopback MIDI input device and open
    /// a connection routed to [`Self::midi_target`]'s current value
    /// (initialized to the first Pitched track's NodeId by
    /// `build_from_project_and_rate`).
    ///
    /// Silently no-ops when no device is connected
    /// (`auto_pick_input` returns `None`) or when midir reports an
    /// error (printed to stderr; the UI still works). K2 surfaces
    /// device selection in the UI for runtime switching.
    pub(super) fn open_default_midi_input(&mut self) {
        let target = NodeId::new(self.midi_target.load(Ordering::Acquire));
        let Some(handle) = self.midi_input_handle.borrow_mut().take() else {
            return;
        };
        let bridge = MidiInputBridge::new(
            handle,
            Arc::clone(&self.sample_clock),
            Arc::clone(&self.midi_target),
        );
        // Diagnostic: list every device midir can see, so a missing
        // keyboard is obvious from the launch log. The auto-pick
        // filters out "Midi Through" (see midi_input::auto_pick_input
        // doc); this listing is unfiltered so a user can spot whether
        // the device is even being enumerated.
        match midi_input::enumerate_inputs() {
            Ok(devices) if !devices.is_empty() => {
                eprintln!("audio: MIDI input devices found ({}):", devices.len());
                for (i, d) in devices.iter().enumerate() {
                    eprintln!("  [{i}] {}", d.name);
                }
            }
            Ok(_) => eprintln!("audio: no MIDI input devices found"),
            Err(e) => eprintln!("audio: enumerating MIDI input devices failed: {e}"),
        }
        match midi_input::auto_pick_input() {
            Ok(Some(device)) => {
                let device_name = device.name.clone();
                match midi_input::open(&device, bridge) {
                    Ok(connection) => {
                        eprintln!(
                            "audio: opened MIDI input '{device_name}' routed to NodeId({})",
                            target.0
                        );
                        self.current_midi_device.set(Some(device_name));
                        *self._midi_connection.borrow_mut() = Some(connection);
                    }
                    Err(e) => {
                        eprintln!("audio: opening MIDI input '{device_name}' failed: {e}");
                    }
                }
            }
            Ok(None) => {
                // No MIDI device connected (or only Midi Through was
                // present). Quietly continue without live input — the
                // round-1 song still plays.
                eprintln!("audio: no non-loopback MIDI input devices; live input disabled");
            }
            Err(e) => {
                eprintln!("audio: enumerating MIDI input devices failed: {e}");
            }
        }
    }

    /// Resolve a track index (into `project.tracks`) to the
    /// instrument NodeId installed for that track in
    /// `configure_graph`. Matches the formula
    /// `NodeId::new((i + 1) as u32)`. Returns `None` if the index
    /// is out of range.
    pub fn track_node_id(&self, track_idx: usize) -> Option<NodeId> {
        if track_idx >= self.project.tracks.len() {
            return None;
        }
        Some(NodeId::new((track_idx + 1) as u32))
    }

    /// Index into `project.tracks` of the first Pitched track —
    /// used as the boot-time MIDI routing target so live input
    /// has a sensible default before the user picks a track.
    pub fn first_pitched_track_index(&self) -> Option<usize> {
        self.project.tracks.iter().position(|t| {
            matches!(t.synth, SynthAssignment::Wavetable(_))
        })
    }

    /// Set the MIDI input routing target to a project track index.
    /// `None` is a no-op (sticky — the previous target stays so
    /// clearing the visual track selection doesn't unhook MIDI).
    /// Invalid indices are silently dropped.
    ///
    /// Called by the K3 UI Effect that observes
    /// [`AppState::midi_target_track`](crate::state::AppState).
    pub fn set_midi_target_track(&self, track_idx: Option<usize>) {
        let Some(idx) = track_idx else {
            return;
        };
        let Some(node_id) = self.track_node_id(idx) else {
            return;
        };
        self.midi_target.store(node_id.0, Ordering::Release);
    }

    /// Enumerate every currently-visible MIDI input device.
    /// Wrapper around [`midi_input::enumerate_inputs`] that exposes
    /// the list to UI components (K2's device picker).
    pub fn available_midi_inputs(&self) -> Vec<String> {
        midi_input::enumerate_inputs()
            .map(|devices| devices.into_iter().map(|d| d.name).collect())
            .unwrap_or_default()
    }

    /// Switch the active MIDI input device.
    ///
    /// `Some(name)` opens the named device (must match a name from
    /// [`Self::available_midi_inputs`]); `None` disconnects. Any
    /// currently-open connection is closed first, and the
    /// [`MidiInputHandle`](rawdaw_engine::MidiInputHandle) is
    /// recovered for the next open. Failures (device gone, midir
    /// error) print to stderr; the previous connection is left
    /// closed and `current_midi_device` updates to `None`.
    pub fn set_midi_device(&self, name: Option<&str>) {
        self.close_midi_connection();
        let Some(name) = name else {
            self.current_midi_device.set(None);
            return;
        };
        let Some(handle) = self.midi_input_handle.borrow_mut().take() else {
            // No handle to give to a fresh connection. Shouldn't
            // happen if close_midi_connection just returned it, but
            // be defensive.
            eprintln!("audio: set_midi_device('{name}') had no handle to open with");
            self.current_midi_device.set(None);
            return;
        };
        let bridge = MidiInputBridge::new(
            handle,
            Arc::clone(&self.sample_clock),
            Arc::clone(&self.midi_target),
        );
        match midi_input::find_input_by_name(name) {
            Ok(Some(device)) => match midi_input::open(&device, bridge) {
                Ok(connection) => {
                    let target = self.midi_target.load(Ordering::Acquire);
                    eprintln!(
                        "audio: opened MIDI input '{name}' routed to NodeId({target})"
                    );
                    self.current_midi_device.set(Some(name.to_string()));
                    *self._midi_connection.borrow_mut() = Some(connection);
                }
                Err(e) => {
                    eprintln!("audio: opening MIDI input '{name}' failed: {e}");
                    // The bridge's handle is gone (moved into the failed
                    // open). midir's connect returns the original
                    // MidiInput on error but not the user data. Treat
                    // the handle as lost; the UI will show disconnected.
                    self.current_midi_device.set(None);
                }
            },
            Ok(None) => {
                eprintln!("audio: MIDI device '{name}' not found");
                // Put the handle back so future set_midi_device calls
                // can use it.
                self.midi_input_handle
                    .borrow_mut()
                    .replace(bridge_into_handle(bridge));
                self.current_midi_device.set(None);
            }
            Err(e) => {
                eprintln!("audio: enumerating MIDI devices failed: {e}");
                self.midi_input_handle
                    .borrow_mut()
                    .replace(bridge_into_handle(bridge));
                self.current_midi_device.set(None);
            }
        }
    }

    /// Close the current midir connection (if any) and recover the
    /// [`MidiInputHandle`] so a subsequent open can reuse it.
    /// Idempotent — calling on an already-disconnected state is a
    /// no-op.
    fn close_midi_connection(&self) {
        let Some(connection) = self._midi_connection.borrow_mut().take() else {
            return;
        };
        // midir's `close()` returns (MidiInput, T) where T is our
        // bridge. Reclaim the handle from the bridge for re-open.
        let (_midi_input, bridge) = connection.close();
        let handle = bridge_into_handle(bridge);
        *self.midi_input_handle.borrow_mut() = Some(handle);
    }
}

/// Destructure a MidiInputBridge into its [`MidiInputHandle`]
/// (discarding `sample_clock` + `target` — both are `Arc` and the
/// host keeps its own clones).
fn bridge_into_handle(bridge: MidiInputBridge) -> rawdaw_engine::MidiInputHandle {
    let MidiInputBridge { handle, .. } = bridge;
    handle
}
