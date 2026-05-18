//! Background poller that mirrors the engine's `Arc<AtomicU64>` sample
//! clock into a rinch `Signal<u64>` for reactive UI consumption.
//!
//! Wrapped in an `Rc<PlayheadPoller>` inside `AudioResources` so the
//! last clone dropping out of scope stops the thread cleanly.
//!
//! This is the explicit anti-pattern called out in the engine-wiring
//! plan: until rinch grows a native audio-thread → signal bridge, the
//! UI has to poll. Sleeping `PLAYHEAD_POLL_INTERVAL_MS` between reads
//! caps wakeups at ~60 Hz and only emits a `Signal::send` when the
//! atomic actually changed since the last read.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rinch::prelude::Signal;

/// Background thread that polls a shared sample-clock atomic and
/// publishes changes into a rinch `Signal<u64>`.
///
/// Constructed via [`Self::spawn`]; the returned handle owns the
/// thread. Dropping it sets a stop flag (with `Release`) and joins
/// the thread — worst case the loop sleeps up to its `interval_ms`
/// before noticing.
pub(super) struct PlayheadPoller {
    stop: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

impl PlayheadPoller {
    /// Spawn the polling thread. `clock` is shared with the audio
    /// thread (the engine's sample clock); `signal` is the UI-facing
    /// reactive playhead position.
    pub(super) fn spawn(
        clock: Arc<AtomicU64>,
        signal: Signal<u64>,
        interval_ms: u64,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        let join_handle = thread::Builder::new()
            .name("rawdaw-playhead-poller".into())
            .spawn(move || {
                // u64::MAX is a "never seen" sentinel so the first
                // successful read of any value — including 0 — emits a
                // Signal::send. The signal itself starts at 0, so this
                // does nothing on the steady state until the audio
                // thread publishes a new clock value.
                let mut last_seen: u64 = u64::MAX;
                while !stop_for_thread.load(Ordering::Acquire) {
                    let now = clock.load(Ordering::Acquire);
                    if now != last_seen {
                        last_seen = now;
                        // `send` routes to the main thread via rinch's
                        // registered dispatcher; required because this
                        // thread is not the rinch main thread.
                        signal.send(now);
                    }
                    thread::sleep(Duration::from_millis(interval_ms));
                }
            })
            .expect("spawning rawdaw-playhead-poller thread must succeed");
        Self {
            stop,
            join_handle: Some(join_handle),
        }
    }
}

impl Drop for PlayheadPoller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.join_handle.take() {
            // Worst case the thread sleeps the poll interval before
            // noticing — acceptable for app teardown. We do join (not
            // detach) so the thread name doesn't leak beyond the
            // AudioResources lifetime.
            let _ = handle.join();
        }
    }
}
