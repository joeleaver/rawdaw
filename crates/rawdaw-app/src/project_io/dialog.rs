//! Native file picker, off the UI thread.
//!
//! [`rfd::FileDialog`] blocks the calling thread until the user
//! picks a file (or cancels). The rinch main thread cannot block
//! without freezing the entire app, so each pick spawns a
//! short-lived `std::thread` that runs the dialog and `send`s the
//! result through a [`rinch::prelude::Signal`] (rinch's cross-thread
//! dispatcher routes the update back to the UI thread).
//!
//! Same pattern as E5's PlayheadPoller — see
//! `crates/rawdaw-app/src/audio/poller.rs` for the precedent.
//!
//! ## Usage
//!
//! Hold a `Signal<Option<DialogOutcome>>` on the UI state. When the
//! user clicks "Open" / "Save As", call [`pick_open_path`] /
//! [`pick_save_path`] passing the signal. The spawned thread fires
//! `signal.send(Some(outcome))` when the dialog closes. A reactive
//! Effect on the signal reads the outcome, dispatches the save /
//! load, then clears the signal back to `None` so the same handler
//! is ready for the next click.
//!
//! Note that `Signal::send` from a background thread requires the
//! rinch runtime's cross-thread dispatcher to be registered. Production
//! callers go through `app::main_window` which is inside the runtime;
//! tests calling these functions outside the runtime would panic on
//! send. That's why this module has no unit tests — the rfd calls
//! also need a desktop session to run, which CI doesn't have.

use std::path::PathBuf;

use rinch::prelude::Signal;

use super::bundle::FILE_EXTENSION;

/// Filter title shown in the dialog's format dropdown. Concise on
/// purpose — the user already knows they want a project file.
const FILTER_TITLE: &str = "rawdaw project";

/// Outcome of a file-dialog interaction.
///
/// Carried through a `Signal<Option<DialogOutcome>>` so the UI side
/// can branch on Picked vs Cancelled and then reset the signal to
/// `None` for the next pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogOutcome {
    /// User picked a path. The path is whatever the OS dialog
    /// returned; rfd handles canonicalization.
    Picked(PathBuf),
    /// User dismissed the dialog without picking.
    Cancelled,
}

/// Spawn a "Save As" dialog. Returns immediately; the spawned
/// `std::thread` runs the blocking rfd call and `send`s a
/// [`DialogOutcome`] to `out` when it closes.
///
/// `default_filename` seeds the dialog's filename input. Pass `None`
/// to let rfd pick the platform default.
pub fn pick_save_path(default_filename: Option<&str>, out: Signal<Option<DialogOutcome>>) {
    let default = default_filename.map(|s| s.to_string());
    std::thread::spawn(move || {
        let mut dlg = rfd::FileDialog::new()
            .add_filter(FILTER_TITLE, &[FILE_EXTENSION])
            .set_title("Save Project As");
        if let Some(name) = default.as_deref() {
            dlg = dlg.set_file_name(name);
        }
        let outcome = match dlg.save_file() {
            Some(path) => DialogOutcome::Picked(path),
            None => DialogOutcome::Cancelled,
        };
        // `send` (vs `set`) is the cross-thread variant — required
        // because we're on a background thread.
        out.send(Some(outcome));
    });
}

/// Spawn an "Open" dialog. Symmetric to [`pick_save_path`].
pub fn pick_open_path(out: Signal<Option<DialogOutcome>>) {
    std::thread::spawn(move || {
        let dlg = rfd::FileDialog::new()
            .add_filter(FILTER_TITLE, &[FILE_EXTENSION])
            .set_title("Open Project");
        let outcome = match dlg.pick_file() {
            Some(path) => DialogOutcome::Picked(path),
            None => DialogOutcome::Cancelled,
        };
        out.send(Some(outcome));
    });
}
