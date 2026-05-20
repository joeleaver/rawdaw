//! Per-track synth parameter pushes + preset application.
//!
//! Extracted from `audio/mod.rs` once that file approached the
//! workspace 700-line cap (C2). All four methods address the audio
//! thread via [`crate::audio::AudioResources::handle`] and the
//! per-track editor handle tables — pure host-side glue, no audio
//! processing.
//!
//! Two slider drag paths land here:
//!
//! - [`AudioResources::push_wavetable_param`] /
//!   [`AudioResources::push_drum_param`] (U5/U7 single-field
//!   updates).
//! - [`AudioResources::apply_wavetable_preset`] /
//!   [`AudioResources::apply_drum_preset`] (U8 whole-patch swaps).
//!
//! Both use the engine's **host live event queue**
//! ([`rawdaw_engine::EngineHandle::push_param`]) so the audio
//! thread drains them in every transport state — sliders work in
//! Stopped / Paused / Playing alike. Same `SampleTime::samples(0)`
//! scheduling convention as live MIDI (K0).

use super::AudioResources;

impl AudioResources {
    /// Push a wavetable `ParamEvent` into the host live-event queue.
    /// `track_idx` selects the target synth via
    /// [`Self::wavetable_handles`]; returns `Err` when the index
    /// has no handle (e.g., it's a drum track).
    ///
    /// Scheduled at [`rawdaw_model::SampleTime::samples`]`(0)` so the
    /// event lands at offset 0 of the next block regardless of
    /// transport state — same convention live MIDI uses
    /// (`docs/midi-input-plan.md` K0). Reading `sample_clock` here
    /// would re-introduce the Stop-reset race that K1.fix already
    /// solved for MIDI input.
    pub fn push_wavetable_param(
        &self,
        track_idx: usize,
        param: rawdaw_synth_wavetable::WavetableParam,
        value: f32,
    ) -> Result<(), String> {
        let handle = self
            .wavetable_handles
            .get(&track_idx)
            .ok_or_else(|| format!("no wavetable handle for track {track_idx}"))?;
        let path = param.encode();
        let time = rawdaw_model::SampleTime::samples(0);
        self.handle()
            .push_param(time, handle.node_id, path, value)
            .map_err(|e| format!("event queue overflow on push_wavetable_param: {e:?}"))
    }

    /// Drum equivalent of [`Self::push_wavetable_param`]. Same
    /// scheduling contract and same `Err` shape when the index
    /// doesn't resolve to a Drum track.
    pub fn push_drum_param(
        &self,
        track_idx: usize,
        param: rawdaw_synth_drum::DrumParam,
        value: f32,
    ) -> Result<(), String> {
        let handle = self
            .drum_handles
            .get(&track_idx)
            .ok_or_else(|| format!("no drum handle for track {track_idx}"))?;
        let path = param.encode();
        let time = rawdaw_model::SampleTime::samples(0);
        self.handle()
            .push_param(time, handle.node_id, path, value)
            .map_err(|e| format!("event queue overflow on push_drum_param: {e:?}"))
    }

    /// Apply a wavetable preset to a Pitched track. Flattens the
    /// patch into one `BlockMessage::Param` per field (~72 events)
    /// and pushes them all at the same `SampleTime::samples(0)`
    /// timestamp so the audio thread applies them inside one block
    /// — patches change atomically from the perspective of any
    /// subsequent NoteOn.
    ///
    /// Also writes the new patch into the host-side
    /// `handle.patch_signal` directly so editor sliders re-sync
    /// immediately even when the engine is `Transport::Stopped`
    /// (which silently drains pending Param events). The audio
    /// thread will overwrite the host signal via its
    /// `WavetablePoller` on the next playing block — the host
    /// write is just a same-value shortcut for the visual case.
    ///
    /// Returns `Err` when the track index has no wavetable handle
    /// (e.g., it's a Drum track) or when the event queue overflows
    /// mid-push.
    pub fn apply_wavetable_preset(
        &self,
        track_idx: usize,
        patch_data: rawdaw_model::patch::wavetable::WavetablePatchData,
    ) -> Result<(), String> {
        let handle = self
            .wavetable_handles
            .get(&track_idx)
            .ok_or_else(|| format!("no wavetable handle for track {track_idx}"))?;
        let patch: rawdaw_synth_wavetable::WavetablePatch = patch_data.into();
        // Update the host-side signal first so the editor sliders
        // re-sync on the next reactive tick. Cheap — Signal::set
        // is one downcast + notify.
        handle.patch_signal.set(patch);
        let events = rawdaw_synth_wavetable::wavetable_patch_to_param_events(&patch);
        // All 72 events scheduled at samples(0) so they apply in the
        // next block regardless of transport state. The synth applies
        // them in order; the last write wins per field, which is the
        // semantics callers expect from "load this preset".
        let time = rawdaw_model::SampleTime::samples(0);
        let mut handle_mut = self.handle();
        for (param, value) in events {
            handle_mut
                .push_param(time, handle.node_id, param.encode(), value)
                .map_err(|e| format!("event queue overflow on apply_wavetable_preset: {e:?}"))?;
        }
        Ok(())
    }

    /// Drum equivalent of [`Self::apply_wavetable_preset`] — pushes
    /// 29 events for a full drum patch + writes the host-side
    /// signal for immediate slider re-sync.
    pub fn apply_drum_preset(
        &self,
        track_idx: usize,
        patch_data: rawdaw_model::patch::drum::DrumPatchData,
    ) -> Result<(), String> {
        let handle = self
            .drum_handles
            .get(&track_idx)
            .ok_or_else(|| format!("no drum handle for track {track_idx}"))?;
        let patch: rawdaw_synth_drum::DrumPatch = patch_data.into();
        handle.patch_signal.set(patch);
        let events = rawdaw_synth_drum::drum_patch_to_param_events(&patch);
        // Same scheduling contract as the wavetable preset path:
        // all events at samples(0) so the audio thread applies them
        // in the next block regardless of transport state.
        let time = rawdaw_model::SampleTime::samples(0);
        let mut handle_mut = self.handle();
        for (param, value) in events {
            handle_mut
                .push_param(time, handle.node_id, param.encode(), value)
                .map_err(|e| format!("event queue overflow on apply_drum_preset: {e:?}"))?;
        }
        Ok(())
    }
}
