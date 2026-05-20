//! Edit → re-realize → audio re-arm pump (composition-writability C2).
//!
//! The single mutation entry point that every Tier-1 editing surface
//! eventually plugs into. UI handlers call
//! [`AudioResources::apply_project_edit`] with a closure that mutates a
//! cloned project; this module handles the rest:
//!
//! 1. Clone the live `Rc<Project>` and run the closure on the inner
//!    `Project`. Wrap the mutated value back in a fresh `Rc` so
//!    `Rc::clone`s held by readers (UI components, the audio side's
//!    own snapshot) stay pointing at the pre-edit value until the
//!    swap below.
//! 2. Re-realize the project's event stream
//!    ([`rawdaw_model::realize::realize`]) and translate to
//!    `BlockEvent`s through the audio side's routing table
//!    ([`rawdaw_engine::translate_events`]).
//! 3. Swap the audio side's `project`, `tempo_map`, and cached
//!    `realized_events` to the new values (interior mut via
//!    `Rc<RefCell<_>>`).
//! 4. Re-arm the engine's song queue. If the transport is Stopped
//!    the engine has already drained the queue, so nothing else is
//!    needed — the next `play()` will push the freshly cached
//!    events. If Playing / Paused, request a song-queue drain via
//!    [`EngineHandle::request_song_queue_drain`], wait for the audio
//!    thread to ack (bounded ~50ms poll), then push every cached
//!    event whose `time >= saved_sample_clock` so past events
//!    (already-played notes) don't re-fire at the current offset.
//!
//! Step 4's drain is independent of transport — the engine's
//! `song_drain_request` atomic was added in C2 specifically so the
//! edit pump can re-arm mid-playback without forcing a Stop→Play
//! recycle (which resets `sample_clock` to 0 and snaps the playhead
//! to bar 1). See `crates/rawdaw-engine/src/audio_engine.rs` for the
//! audio-thread side of the protocol.
//!
//! The returned `Rc<Project>` is the freshly installed snapshot;
//! [`AppState::apply_project_edit`](crate::state::AppState) calls
//! this and then `set`s its own `project` Signal to the same `Rc`,
//! keeping the UI and audio-side views in lockstep.

use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use rawdaw_engine::translate_events;
use rawdaw_model::project::Project;
use rawdaw_model::realize::realize;

use super::AudioResources;

/// Hard upper bound on how long the host will wait for the audio
/// thread to ack a `song_drain_request`. One `process_block` at
/// 44.1 kHz / 256 frames is ~5.8 ms; 50 ms covers an order of
/// magnitude of timing slop (very large block sizes, contended
/// audio thread, scheduling jitter). If the wait blows past this the
/// audio thread is in an unrecoverable state — surface the error
/// rather than spin forever.
#[cfg_attr(not(debug_assertions), allow(dead_code))]
const DRAIN_ACK_TIMEOUT: Duration = Duration::from_millis(50);

// Release builds don't reach the edit pump yet — the only call site
// today (the TopBar +1 BPM debug button) is cfg-gated to
// `debug_assertions`. C3 (project load) and C4 (real tempo / key /
// name controls) add release call sites and these allows go away.
#[cfg_attr(not(debug_assertions), allow(dead_code))]
impl AudioResources {
    /// Apply a structural edit to the live project and re-arm the
    /// audio engine against the re-realized event stream.
    ///
    /// The closure `f` runs on a clone of the current project; the
    /// mutated value becomes the new live snapshot. On success the
    /// freshly installed `Rc<Project>` is returned so the caller
    /// (typically [`AppState::apply_project_edit`](crate::state::AppState))
    /// can mirror the same `Rc` into its UI-facing signal.
    ///
    /// Returns `Err` when re-realization produces events for an
    /// unrouted track (programmer error — see
    /// [`rawdaw_engine::TranslateError`]), when the engine's event
    /// queue overflows mid-rearm (host-side capacity bug, not a
    /// race), or when the audio thread fails to ack the song-queue
    /// drain within [`DRAIN_ACK_TIMEOUT`] (the audio thread is
    /// blocked or stalled).
    ///
    /// On error the live project is **not** updated — the swap only
    /// happens after re-realization succeeds. Re-arm errors leave
    /// the new project installed but the engine in an inconsistent
    /// state; the host should surface the error and prompt a manual
    /// Stop → Play recycle.
    pub fn apply_project_edit<F>(&self, f: F) -> Result<Rc<Project>, String>
    where
        F: FnOnce(&mut Project),
    {
        // 1. Clone current project and run the closure on the inner
        //    value. Wrapping the mutated project in a fresh `Rc`
        //    means readers still holding the old `Rc` aren't
        //    disturbed by the in-place mutation.
        let current = self.project();
        let mut next = (*current).clone();
        f(&mut next);
        let new_project = Rc::new(next);

        // 2. Re-realize. `realize` is pure-functional, microseconds
        //    for round-1's ~hundreds of events in release builds.
        let realized = realize(&new_project, self.sample_rate);
        let new_events = translate_events(&realized, &self.routing)
            .map_err(|e| format!("translate failed during edit-pump rearm: {e}"))?;

        // 3. Swap host-side snapshots. Once these are written, every
        //    subsequent `project()` / `tempo_map()` / `realized_events`
        //    read sees the new values. UI components reading via
        //    `AppState.project` won't observe the change until the
        //    AppState delegator sets its signal.
        *self.project.borrow_mut() = Rc::clone(&new_project);
        *self.tempo_map.borrow_mut() = new_project.tempo_map.clone();
        *self.realized_events.borrow_mut() = Rc::new(new_events);

        // 4. Re-arm the engine.
        self.re_arm()?;

        Ok(new_project)
    }

    /// Drain the engine's song queue and re-push the cached
    /// realized events. Separate from [`Self::apply_project_edit`]
    /// so future call sites (C3's project-load path) can drive a
    /// re-arm without going through the edit closure.
    ///
    /// Transport semantics:
    ///
    /// - **Stopped**: the engine drains the song queue on every
    ///   `process_block` already (see
    ///   `audio_engine.rs::process_block` step 1). The cached
    ///   events were updated in `apply_project_edit`; the next
    ///   `play()` will push them via [`Self::rearm_events`]. No
    ///   work here.
    /// - **Playing / Paused**: request a song-queue drain through
    ///   the engine's `song_drain_request` atomic, wait for the
    ///   audio thread to ack (clear the flag), then push every
    ///   cached event whose `time` is at or after the saved
    ///   `sample_clock`. Past events (already-played notes that the
    ///   audio thread has already consumed) are filtered out so
    ///   they don't re-fire at offset 0 of the current block.
    pub fn re_arm(&self) -> Result<(), String> {
        use rawdaw_engine::Transport;
        let transport = self.transport.get();
        if matches!(transport, Transport::Stopped) {
            // Engine drains on Stop entry; cache is already fresh.
            // Next `play()` re-arms via the existing path.
            return Ok(());
        }

        // Snapshot the clock BEFORE requesting the drain — between
        // the request and the audio thread's ack the clock continues
        // to advance, and we want the cut-off to be "everything in
        // the past at the moment we decided to re-arm." Slightly
        // late events (between the saved clock and the audio
        // thread's block start when it processes the drain) will
        // still fire because their absolute `time` is checked
        // against `saved_clock`, not the audio thread's current
        // `sample_clock`.
        let saved_clock = self.sample_clock.load(Ordering::Acquire);

        // Request drain + wait for ack. The audio thread `swap`s the
        // flag false in one op, paired with this `Acquire` load.
        let handle_for_drain = self.handle.borrow();
        handle_for_drain.request_song_queue_drain();
        let start = Instant::now();
        while handle_for_drain.song_drain_pending() {
            if start.elapsed() > DRAIN_ACK_TIMEOUT {
                return Err(format!(
                    "audio thread did not ack song-queue drain within {}ms",
                    DRAIN_ACK_TIMEOUT.as_millis()
                ));
            }
            std::thread::yield_now();
        }
        drop(handle_for_drain);

        // Drain acked — safe to push the new event stream. Filter
        // out past events so they don't fire late at offset 0.
        let events = self.realized_events.borrow().clone();
        let mut handle = self.handle.borrow_mut();
        for ev in events.iter() {
            if ev.time.as_samples() < saved_clock {
                continue;
            }
            handle
                .push_event(ev.clone())
                .map_err(|e| format!("event queue overflowed while re-arming: {e:?}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rawdaw_engine::Transport;
    use rawdaw_model::fixtures::build_round1_project;
    use rawdaw_model::tempo::{BeatUnit, TempoMap};

    use crate::audio::FALLBACK_SAMPLE_RATE;

    #[test]
    fn apply_project_edit_swaps_project_and_tempo_snapshots() {
        let (project, _) = build_round1_project();
        let resources =
            AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);

        let new_rc = resources
            .apply_project_edit(|p| {
                p.tempo_map = TempoMap::constant(180.0, 4, BeatUnit::Quarter);
            })
            .expect("edit pump succeeds in Stopped transport");

        // The returned `Rc` is the live snapshot.
        assert!(Rc::ptr_eq(&new_rc, &resources.project()));
        // The tempo_map cache reflects the new BPM.
        assert_eq!(resources.tempo_map().bpm_events[0].bpm, 180.0);
    }

    #[test]
    fn apply_project_edit_in_stopped_skips_engine_drain() {
        // Stopped transport leaves the drain flag untouched — the
        // engine already drains on every block, and a Play after
        // edit hits the existing rearm_events path.
        let (project, _) = build_round1_project();
        let resources =
            AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);
        assert_eq!(resources.transport.get(), Transport::Stopped);

        resources
            .apply_project_edit(|p| {
                p.tempo_map = TempoMap::constant(140.0, 4, BeatUnit::Quarter);
            })
            .expect("Stopped edit succeeds");

        // The drain flag was never set — peek via a fresh probe.
        // Confirms re_arm took the Stopped fast path.
        assert!(!resources.handle.borrow().song_drain_pending());
    }

    #[test]
    fn apply_project_edit_reruns_realization() {
        // After an edit, `realized_events` should reflect the new
        // project — the round-1 fixture's event count is stable, so
        // an edit that only changes tempo doesn't change the count.
        // But the cached `Rc<Vec<_>>` is a fresh allocation; verify
        // it's not the same `Rc` as before.
        let (project, _) = build_round1_project();
        let resources =
            AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);

        let events_before = resources.realized_events.borrow().clone();

        resources
            .apply_project_edit(|p| {
                p.tempo_map = TempoMap::constant(180.0, 4, BeatUnit::Quarter);
            })
            .expect("edit succeeds");

        let events_after = resources.realized_events.borrow().clone();
        assert!(
            !Rc::ptr_eq(&events_before, &events_after),
            "edit pump must re-realize and swap the events Rc"
        );
        assert_eq!(events_before.len(), events_after.len());
    }

    #[test]
    fn apply_project_edit_rewrites_chord_loop_event_pitches() {
        // C2 done-when, pin (b): "apply an edit that changes a
        // chord-loop event's quality, verify the next rendered block
        // carries the new note pitches." Verified at the cached
        // event-stream level — every Pitched track NoteOn flows
        // through `realized_events` after translation, so a chord
        // quality flip changes the MIDI note numbers in the cache,
        // and the engine-side drain test
        // (`song_drain_request_clears_event_queue_while_playing`)
        // already pins that the audio thread consumes the freshly
        // pushed events without re-firing the old ones.
        use rawdaw_engine::BlockMessage;
        use rawdaw_model::chord::{ChordQuality, ChordSpec, ChordSuffix};
        use rawdaw_model::Midi2Message;

        let (project, _) = build_round1_project();
        let resources =
            AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);

        let collect_pitches = |evs: &[rawdaw_engine::BlockEvent]| -> Vec<u8> {
            evs.iter()
                .filter_map(|ev| match &ev.message {
                    BlockMessage::Midi(Midi2Message::NoteOn { note, .. }) => Some(note.get()),
                    _ => None,
                })
                .collect()
        };

        let before = collect_pitches(&resources.realized_events.borrow());
        assert!(
            !before.is_empty(),
            "round-1 should realize at least one NoteOn"
        );

        resources
            .apply_project_edit(|p| {
                // Force every functional chord in every loop to IV
                // (subdominant). Round-1's patterns play chord
                // tones (root + fifth + ...) which are stable across
                // Major↔Minor for the I and V degrees the verse
                // begins on, so a quality-only flip doesn't move
                // every realized NoteOn. Shifting the Roman degree
                // wholesale changes the chord's root pitch class,
                // which moves every chord-relative pattern note.
                use rawdaw_model::chord::RomanDegree;
                let new_suffix = ChordSuffix::new(ChordQuality::Major);
                for loop_ in p.chord_loops.values_mut() {
                    for ev in &mut loop_.events {
                        if let ChordSpec::Functional { roman, suffix, .. } = &mut ev.chord {
                            *roman = RomanDegree::IV;
                            *suffix = new_suffix.clone();
                        }
                    }
                }
            })
            .expect("chord-degree edit succeeds");

        let after = collect_pitches(&resources.realized_events.borrow());
        assert_eq!(
            before.len(),
            after.len(),
            "chord-quality flip doesn't change event count",
        );
        assert_ne!(
            before, after,
            "all-Minor flip must change at least one realized NoteOn pitch",
        );
    }

    #[test]
    fn apply_project_edit_does_not_mutate_old_snapshot() {
        // The clone-then-mutate strategy means callers still holding
        // the old `Rc<Project>` see the pre-edit value. This pins
        // that contract — without it, components rendering during a
        // mid-edit observation could see partial state.
        let (project, _) = build_round1_project();
        let resources =
            AudioResources::build_from_project_and_rate(&project, FALLBACK_SAMPLE_RATE);

        let before = resources.project();
        let original_bpm = before.tempo_map.bpm_events[0].bpm;

        resources
            .apply_project_edit(|p| {
                p.tempo_map = TempoMap::constant(180.0, 4, BeatUnit::Quarter);
            })
            .expect("edit succeeds");

        // The Rc we captured BEFORE the edit must still show the old
        // BPM — proves the mutation cloned-on-write.
        assert_eq!(before.tempo_map.bpm_events[0].bpm, original_bpm);
        // The fresh snapshot shows the new BPM.
        assert_eq!(resources.project().tempo_map.bpm_events[0].bpm, 180.0);
    }

}
