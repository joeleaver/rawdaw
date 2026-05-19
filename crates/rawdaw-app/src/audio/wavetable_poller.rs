//! Background poller that mirrors a [`WavetableSynthNode`]'s
//! patch-version atomic into a rinch `Signal<WavetablePatch>`.
//!
//! Same shape as [`PlayheadPoller`](super::poller::PlayheadPoller):
//! a `std::thread` watches an `Arc<AtomicU64>` at ~20 Hz and
//! `Signal::send`s the latest patch snapshot into the UI when the
//! atomic advances. The audio thread bumps the atomic + writes the
//! snapshot on every successful `ParamEvent` apply; the host
//! captures matching publishers when it constructs the synth node
//! (see [`WavetablePublishers`](rawdaw_synth_wavetable::WavetablePublishers)).
//!
//! Wrapped in an `Rc<WavetablePoller>` so the last `AudioResources`
//! clone going out of scope tears the thread down — mirror of the
//! PlayheadPoller drop contract.
//!
//! Polling at ~20 Hz (50 ms) gives external patch updates (future
//! MIDI Learn / automation) a worst-case 50 ms visual latency in
//! the editor — well under perceptible UI lag. Slider drags
//! produce their own immediate visual feedback via the slider
//! component's internal state; the poller is there for *external*
//! changes, not for echoing the user's own drags.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rinch::prelude::Signal;

use rawdaw_synth_wavetable::WavetablePatch;

/// Polling interval for wavetable patch snapshots. 20 Hz keeps
/// external-source latency under one frame at 30 fps with negligible
/// CPU — the loop body is two atomic loads + (maybe) a brief lock.
pub(super) const WAVETABLE_PATCH_POLL_INTERVAL_MS: u64 = 50;

/// Background thread that polls a shared `Arc<AtomicU64>`
/// patch-version atomic and publishes patch snapshots into a rinch
/// `Signal<WavetablePatch>` whenever the version advances.
pub(super) struct WavetablePoller {
    stop: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

impl WavetablePoller {
    /// Spawn the polling thread. `version` + `snapshot` are the
    /// audio thread's publishers; `signal` is the UI-facing reactive
    /// patch mirror.
    pub(super) fn spawn(
        version: Arc<AtomicU64>,
        snapshot: Arc<Mutex<WavetablePatch>>,
        signal: Signal<WavetablePatch>,
        interval_ms: u64,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        let join_handle = thread::Builder::new()
            .name("rawdaw-wavetable-poller".into())
            .spawn(move || {
                // The audio thread's first apply will bump version to
                // 1; the snapshot's boot value sits at the patch the
                // node was constructed with. Start at `u64::MAX` as a
                // "never seen" sentinel so a first version of 0 also
                // triggers a send — matches the PlayheadPoller idiom.
                let mut last_seen: u64 = u64::MAX;
                while !stop_for_thread.load(Ordering::Acquire) {
                    let now = version.load(Ordering::Acquire);
                    if now != last_seen {
                        last_seen = now;
                        // Brief lock — the patch is Copy, so this is
                        // a memcpy under the mutex. The audio thread
                        // might block for the duration if it happens
                        // to be applying a Param event right now; the
                        // critical section is microseconds.
                        if let Ok(guard) = snapshot.lock() {
                            let patch = *guard;
                            drop(guard);
                            signal.send(patch);
                        }
                    }
                    thread::sleep(Duration::from_millis(interval_ms));
                }
            })
            .expect("spawning rawdaw-wavetable-poller thread must succeed");
        Self {
            stop,
            join_handle: Some(join_handle),
        }
    }
}

impl Drop for WavetablePoller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.join_handle.take() {
            // Same drop contract as PlayheadPoller: worst case the
            // thread sleeps the poll interval before noticing. The
            // join keeps the thread name from leaking past
            // AudioResources's lifetime.
            let _ = handle.join();
        }
    }
}
