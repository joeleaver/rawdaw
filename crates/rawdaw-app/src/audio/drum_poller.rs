//! Background poller that mirrors a [`DrumSynthNode`]'s patch-version
//! atomic into a rinch `Signal<DrumPatch>`.
//!
//! Mirror of [`WavetablePoller`](super::wavetable_poller::WavetablePoller)
//! for drum tracks. Same drop semantics; same 20 Hz interval.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rinch::prelude::Signal;

use rawdaw_synth_drum::DrumPatch;

/// Polling interval for drum patch snapshots. Matches the wavetable
/// poller — 20 Hz keeps external-source latency under one frame at
/// 30 fps with negligible CPU.
pub(super) const DRUM_PATCH_POLL_INTERVAL_MS: u64 = 50;

/// Background thread that polls a shared `Arc<AtomicU64>`
/// patch-version atomic and publishes patch snapshots into a rinch
/// `Signal<DrumPatch>` whenever the version advances.
pub(super) struct DrumPoller {
    stop: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

impl DrumPoller {
    /// Spawn the polling thread. `version` + `snapshot` are the
    /// audio thread's publishers; `signal` is the UI-facing reactive
    /// patch mirror.
    pub(super) fn spawn(
        version: Arc<AtomicU64>,
        snapshot: Arc<Mutex<DrumPatch>>,
        signal: Signal<DrumPatch>,
        interval_ms: u64,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        let join_handle = thread::Builder::new()
            .name("rawdaw-drum-poller".into())
            .spawn(move || {
                let mut last_seen: u64 = u64::MAX;
                while !stop_for_thread.load(Ordering::Acquire) {
                    let now = version.load(Ordering::Acquire);
                    if now != last_seen {
                        last_seen = now;
                        if let Ok(guard) = snapshot.lock() {
                            let patch = *guard;
                            drop(guard);
                            signal.send(patch);
                        }
                    }
                    thread::sleep(Duration::from_millis(interval_ms));
                }
            })
            .expect("spawning rawdaw-drum-poller thread must succeed");
        Self {
            stop,
            join_handle: Some(join_handle),
        }
    }
}

impl Drop for DrumPoller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}
